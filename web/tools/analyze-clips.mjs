#!/usr/bin/env bun
/**
 * analyze-clips.mjs — metadata diagnostic tool (Luna Web, since 2026-07-18).
 *
 * Purpose: get a full glance at the raw metadata payloads mediainfo.js reads
 * from the camera file types we hit on set (Sony MXF / X-OCN, ARRI ProRes,
 * DJI ProRes, Blackmagic BRAW, ...), plus whatever sidecars (Sony NRT XML,
 * Avid ALE) ride alongside. The findings tell us how to build out Luna Web's
 * metadata parser in more detail. It is NOT shipped in the app bundle — it's a
 * dev/debug tool we keep around to inspect new clips and extend camera/model
 * support over time. See FINDINGS.md for what we've learned so far.
 *
 * It drives the SAME mediainfo.js the app worker uses, and the SAME
 * mapMediaInfoToClipMetadata mapper from @luna-web/core, so "what we read"
 * and "what we currently keep" sit side by side.
 *
 * Run (from web/):
 *   bun tools/analyze-clips.mjs                       # default corpora
 *   bun tools/analyze-clips.mjs "D:/path/to/CAMERA"   # a folder (walked)
 *   bun tools/analyze-clips.mjs file1.mxf file2.mov   # explicit clips
 *   bun tools/analyze-clips.mjs --out tools/out ...    # output dir override
 *
 * Outputs (git-ignored, tools/out/ by default):
 *   <label>.json  — full mediainfo payload + current mapping + sidecars, per clip
 *   _schema.json  — machine-readable union of tag keys per track type
 *   _schema.md    — human-readable schema/coverage tables
 */

import { mkdir, open, readdir, readFile, stat, writeFile } from 'node:fs/promises'
import { createRequire } from 'node:module'
import { basename, dirname, extname, join, resolve, sep } from 'node:path'
import { fileURLToPath } from 'node:url'

import {
  fileExtensionOf,
  SUPPORTED_MEDIA_EXTENSIONS,
  UNSUPPORTED_RAW_EXTENSIONS,
} from '../packages/core/src/media/extensions.ts'
import { mapMediaInfoToClipMetadata } from '../packages/core/src/metadata/mediainfo.ts'

const HERE = dirname(fileURLToPath(import.meta.url)) // web/tools
const WEB = resolve(HERE, '..')

// Resolve mediainfo.js exactly as the app would (from app's dependency graph).
const requireFromApp = createRequire(join(WEB, 'app', 'package.json'))
const mediaInfoFactoryMod = requireFromApp('mediainfo.js')
const mediaInfoFactory = mediaInfoFactoryMod.default ?? mediaInfoFactoryMod
const wasmPath = requireFromApp.resolve('mediainfo.js/MediaInfoModule.wasm')

const DEFAULT_ROOTS = ['D:/LUNA_TEST/TEST_PROJECT_LUNA/CAMERA', 'E:/Coding/LunaApp/docs']
const MEDIA_EXTS = new Set([...SUPPORTED_MEDIA_EXTENSIONS, ...UNSUPPORTED_RAW_EXTENSIONS])
// Camera-raw extensions Luna core doesn't list yet, but the diagnostic should
// still probe on a directory walk (does mediainfo read them at all?). Presence
// here means "worth inspecting", NOT "supported" — keep it in the tool, not core.
// .crm = Canon Cinema RAW Light, .rmf = Canon original RAW, .cine = Phantom,
// .dng/.cdng = CinemaDNG. Explicit file args bypass filtering entirely.
const EXTRA_PROBE_EXTS = new Set(['.crm', '.rmf', '.cine', '.dng', '.cdng'])
const PROBE_EXTS = new Set([...MEDIA_EXTS, ...EXTRA_PROBE_EXTS])
const SIDECAR_TEXT_EXTS = new Set(['.xml', '.ale', '.xmp', '.cdl', '.ccc', '.txt'])
const SIDECAR_BINARY_EXTS = new Set(['.bin'])
const SKIP_DIRS = new Set(['node_modules', '.git', 'out'])
const MAX_SIDECAR_TEXT_BYTES = 512 * 1024

// ---------------------------------------------------------------------------
// arg parsing
// ---------------------------------------------------------------------------
function parseArgs(argv) {
  const inputs = []
  let outDir = join(HERE, 'out')
  for (let i = 0; i < argv.length; i++) {
    const a = argv[i]
    if (a === '--out') outDir = resolve(argv[++i])
    else if (a === '--help' || a === '-h') {
      console.log('Usage: bun tools/analyze-clips.mjs [--out <dir>] [paths...]')
      process.exit(0)
    } else inputs.push(a)
  }
  return { inputs: inputs.length ? inputs : DEFAULT_ROOTS, outDir }
}

// ---------------------------------------------------------------------------
// discovery: expand inputs (files or dirs) into a clip list
// ---------------------------------------------------------------------------
async function walk(dir, acc) {
  let entries
  try {
    entries = await readdir(dir, { withFileTypes: true })
  } catch {
    return
  }
  for (const e of entries) {
    if (e.isDirectory()) {
      if (SKIP_DIRS.has(e.name)) continue
      await walk(join(dir, e.name), acc)
    } else if (e.isFile()) {
      acc.push(join(dir, e.name))
    }
  }
}

async function discoverClips(inputs) {
  const files = []
  for (const input of inputs) {
    const abs = resolve(input)
    let s
    try {
      s = await stat(abs)
    } catch {
      console.warn(`! skip (not found): ${input}`)
      continue
    }
    if (s.isDirectory()) {
      const walked = []
      await walk(abs, walked)
      for (const f of walked) if (PROBE_EXTS.has(fileExtensionOf(f))) files.push(f)
    } else {
      files.push(abs) // explicit file: probe regardless of extension
    }
  }
  return files
}

// Sidecars = sibling files in the clip's folder that are metadata carriers.
// Matched loosely: any text/binary sidecar in the same directory (per-reel
// files like *_AVID.ale and *.bin are shared, so we surface them all and
// flag whether the name stem matches this clip).
async function findSidecars(clipPath) {
  const dir = dirname(clipPath)
  const stem = basename(clipPath, extname(clipPath)).toLowerCase()
  let entries
  try {
    entries = await readdir(dir, { withFileTypes: true })
  } catch {
    return []
  }
  const out = []
  for (const e of entries) {
    if (!e.isFile()) continue
    const ext = fileExtensionOf(e.name)
    if (!SIDECAR_TEXT_EXTS.has(ext) && !SIDECAR_BINARY_EXTS.has(ext)) continue
    const p = join(dir, e.name)
    const st = await stat(p)
    const nameStem = basename(e.name, extname(e.name)).toLowerCase()
    // Sony writes "<clip>M01.xml"; match exact stem or "<clip>m01".
    const matchesClip = nameStem === stem || nameStem === `${stem}m01`
    if (SIDECAR_BINARY_EXTS.has(ext)) {
      out.push({ name: e.name, ext, sizeBytes: st.size, matchesClip, note: 'binary — not dumped' })
      continue
    }
    let text = null
    let truncated = false
    if (st.size <= MAX_SIDECAR_TEXT_BYTES) {
      text = await readFile(p, 'utf8')
    } else {
      const fh = await open(p, 'r')
      const buf = Buffer.alloc(MAX_SIDECAR_TEXT_BYTES)
      const { bytesRead } = await fh.read(buf, 0, MAX_SIDECAR_TEXT_BYTES, 0)
      await fh.close()
      text = buf.subarray(0, bytesRead).toString('utf8')
      truncated = true
    }
    out.push({ name: e.name, ext, sizeBytes: st.size, matchesClip, truncated, text })
  }
  return out
}

// ---------------------------------------------------------------------------
// mediainfo: analyze a file by streaming chunks (mirrors the app worker)
// ---------------------------------------------------------------------------
async function analyzeFile(mediainfo, filePath) {
  const fh = await open(filePath, 'r')
  try {
    const { size } = await fh.stat()
    const readChunk = async (chunkSize, offset) => {
      const buf = Buffer.alloc(chunkSize)
      const { bytesRead } = await fh.read(buf, 0, chunkSize, offset)
      return new Uint8Array(buf.subarray(0, bytesRead))
    }
    return await mediainfo.analyzeData(size, readChunk)
  } finally {
    await fh.close()
  }
}

// A short, disambiguating label from the last 2-3 path segments.
function labelFor(filePath) {
  const parts = resolve(filePath).split(sep)
  return parts.slice(-3).join('/')
}

function safeName(label) {
  return label.replace(/[^\w.-]+/g, '_')
}

// ---------------------------------------------------------------------------
// schema aggregation
// ---------------------------------------------------------------------------
function accumulateSchema(schema, label, mediainfoResult) {
  const tracks = mediainfoResult?.media?.track ?? []
  for (const track of tracks) {
    const type = String(track['@type'] ?? 'Unknown')
    if (!schema[type]) schema[type] = {}
    const bucket = schema[type]
    for (const key of Object.keys(track)) {
      if (key === '@type') continue
      if (!bucket[key]) bucket[key] = new Set()
      bucket[key].add(label)
    }
  }
}

function schemaToJson(schema) {
  const out = {}
  for (const [type, keys] of Object.entries(schema)) {
    out[type] = {}
    for (const [key, labels] of Object.entries(keys)) {
      out[type][key] = [...labels].sort()
    }
  }
  return out
}

// The camera block we WANT to fill in ClipMetadata, and mediainfo tag names
// worth watching as candidate sources (purely to focus the human eye).
const CAMERA_FIELD_HINTS = {
  camera: ['Encoded_Library_Name', 'Comment', 'Make', 'Model', 'com.arri', 'CameraModel'],
  iso: ['ExposureIndex', 'ISO', 'com.arri.camera.ExposureIndex'],
  whiteBalance: ['WhiteBalance', 'ColorTemperature', 'com.arri.camera.WhiteBalance'],
  lens: ['Lens', 'LensModel', 'com.arri.lens'],
  focalLength: ['FocalLength'],
  aperture: ['Aperture', 'IrisFNumber', 'FNumber'],
  shutter: ['ShutterSpeed', 'ShutterAngle', 'ExposureTime'],
  gamma: ['Gamma', 'transfer_characteristics', 'CaptureGammaEquation', 'colour_transfer'],
}

function renderSchemaMarkdown(schema, clips) {
  const lines = []
  lines.push('# mediainfo.js payload schema — Luna Web corpus glance')
  lines.push('')
  lines.push(`_Generated ${new Date().toISOString()} · ${clips.length} clips analyzed._`)
  lines.push('')

  lines.push('## Clips')
  lines.push('')
  lines.push('| Clip | Ext | Container | Video codec | Duration | mediainfo tags | Sidecars |')
  lines.push('|---|---|---|---|---|---|---|')
  for (const c of clips) {
    const g = c.tracks.find((t) => t['@type'] === 'General') ?? {}
    const v = c.tracks.find((t) => t['@type'] === 'Video') ?? {}
    const tagCount = c.tracks.reduce((n, t) => n + Object.keys(t).length - 1, 0)
    const sidecars = c.sidecars.map((s) => s.ext).join(' ') || '—'
    lines.push(
      `| ${c.label} | ${c.ext} | ${g.Format ?? '—'} | ${v.Format ?? '—'} | ${g.Duration ?? v.Duration ?? '—'} | ${tagCount} | ${sidecars} |`,
    )
  }
  lines.push('')

  const types = Object.keys(schema).sort()
  for (const type of types) {
    const keys = Object.keys(schema[type]).sort()
    lines.push(`## ${type} track — ${keys.length} distinct tags`)
    lines.push('')
    lines.push('| Tag | Seen in |')
    lines.push('|---|---|')
    for (const key of keys) {
      const labels = [...schema[type][key]].sort()
      lines.push(`| \`${key}\` | ${labels.length}/${clips.length} |`)
    }
    lines.push('')
  }

  lines.push('## Camera-block coverage (what we still want to fill)')
  lines.push('')
  lines.push('For each ClipMetadata camera field, candidate mediainfo tags observed in the corpus:')
  lines.push('')
  lines.push('| ClipMetadata field | Candidate tags present in corpus |')
  lines.push('|---|---|')
  const allTags = new Set()
  for (const type of types) for (const k of Object.keys(schema[type])) allTags.add(k)
  for (const [field, hints] of Object.entries(CAMERA_FIELD_HINTS)) {
    const hits = [...allTags].filter((tag) =>
      hints.some((h) => tag.toLowerCase().includes(h.toLowerCase())),
    )
    lines.push(
      `| \`${field}\` | ${hits.length ? hits.map((h) => `\`${h}\``).join(', ') : '— none via mediainfo (needs sidecar/vendor)'} |`,
    )
  }
  lines.push('')
  return lines.join('\n')
}

// ---------------------------------------------------------------------------
// main
// ---------------------------------------------------------------------------
async function main() {
  const { inputs, outDir } = parseArgs(process.argv.slice(2))
  const clipPaths = await discoverClips(inputs)
  if (!clipPaths.length) {
    console.error('No media files found in:', inputs.join(', '))
    process.exit(1)
  }
  await mkdir(outDir, { recursive: true })

  console.log(`Analyzing ${clipPaths.length} clip(s) → ${outDir}\n`)
  const mediainfo = await mediaInfoFactory({
    format: 'object',
    locateFile: (p) => (p.endsWith('.wasm') ? wasmPath : p),
  })

  const schema = {}
  const clips = []
  try {
    for (const filePath of clipPaths) {
      const label = labelFor(filePath)
      const ext = fileExtensionOf(filePath)
      process.stdout.write(`• ${label} … `)
      const record = { label, path: filePath, ext }
      try {
        const raw = await analyzeFile(mediainfo, filePath)
        const tracks = raw?.media?.track ?? []
        record.tracks = tracks
        record.mappedClipMetadata = mapMediaInfoToClipMetadata(raw)
        record.mediainfo = raw
        accumulateSchema(schema, label, raw)
      } catch (err) {
        record.tracks = []
        record.error = String(err?.message ?? err)
        console.log(`ERROR: ${record.error}`)
      }
      record.sidecars = await findSidecars(filePath)
      if (!record.error) {
        const tagCount = record.tracks.reduce((n, t) => n + Object.keys(t).length - 1, 0)
        const sc = record.sidecars.length ? ` (+${record.sidecars.length} sidecar)` : ''
        console.log(`${record.tracks.length} tracks, ${tagCount} tags${sc}`)
      }
      clips.push(record)
      await writeFile(
        join(outDir, `${safeName(label)}.json`),
        JSON.stringify(
          {
            file: filePath,
            ext,
            knownToCore: MEDIA_EXTS.has(ext),
            mappedClipMetadata: record.mappedClipMetadata,
            error: record.error,
            sidecars: record.sidecars,
            mediainfo: record.mediainfo,
          },
          null,
          2,
        ),
      )
    }
  } finally {
    mediainfo.close?.()
  }

  await writeFile(join(outDir, '_schema.json'), JSON.stringify(schemaToJson(schema), null, 2))
  await writeFile(join(outDir, '_schema.md'), renderSchemaMarkdown(schema, clips))

  console.log(`\nDone. Wrote ${clips.length} clip dumps + _schema.json + _schema.md to ${outDir}`)
}

main().catch((err) => {
  console.error(err)
  process.exit(1)
})
