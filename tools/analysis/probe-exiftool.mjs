#!/usr/bin/env bun
/**
 * probe-exiftool.mjs — companion to analyze-clips.mjs (Luna Web).
 *
 * mediainfo.js reads container/video basics but nothing camera-specific for
 * some formats (notably Canon Cinema RAW Light `.crm`, and it can't decode
 * their frames). exiftool reads vendor maker-note metadata AND can pull the
 * embedded preview/thumbnail JPEG out of RAW containers WITHOUT debayering.
 * This probes whether that path gives Luna a usable thumbnail + camera fields.
 *
 * Not shipped in the app — a dev/debug tool, like analyze-clips.mjs.
 *
 * Run (from the repo root):
 *   bun tools/analysis/probe-exiftool.mjs                    # default: the S001 .crm
 *   bun tools/analysis/probe-exiftool.mjs path/to/clip.crm   # explicit files
 *   bun tools/analysis/probe-exiftool.mjs --out tools/analysis/out/exiftool ...
 *
 * Outputs (git-ignored, tools/analysis/out/exiftool/ by default):
 *   <label>.exiftool.json   — every tag exiftool read
 *   <label>.<Tag>.jpg       — each embedded image tag it could extract
 */

import { mkdir, stat, writeFile } from 'node:fs/promises'
import { basename, join, resolve, sep } from 'node:path'
import { fileURLToPath } from 'node:url'
import { exiftool } from 'exiftool-vendored'

const HERE = fileURLToPath(new URL('.', import.meta.url)) // tools/analysis/
const DEFAULT_FILES = [
  'D:/LUNA_TEST/TEST_PROJECT_LUNA/CAMERA/S_cam/S001/S_0001C038X241003_1807575U_CANONRAW.CRM',
]

// Camera fields we care about for ClipMetadata, and the exiftool tags that carry
// them (exiftool normalizes across vendors, so this is largely format-agnostic).
const CAMERA_TAGS = {
  make: ['Make'],
  camera: ['Model', 'CanonModelID'],
  iso: ['ISO', 'CameraISO', 'BaseISO', 'RecommendedExposureIndex'],
  whiteBalance: ['WhiteBalance', 'ColorTemperature', 'WB_RGGBLevelsAsShot'],
  shutter: ['ShutterSpeed', 'ExposureTime', 'ShutterSpeedValue'],
  aperture: ['Aperture', 'FNumber', 'ApertureValue'],
  focalLength: ['FocalLength'],
  lens: ['LensModel', 'LensType', 'LensID', 'LensInfo'],
  gamma: ['CanonLogVersion', 'Gamma', 'PictureStyle'],
  fps: ['VideoFrameRate', 'FrameRate'],
  reel: ['ReelName', 'CameraRollName'],
}

// Embedded-image tags worth trying to extract as a thumbnail source.
const IMAGE_TAGS = [
  'JpgFromRaw',
  'PreviewImage',
  'OtherImage',
  'ThumbnailImage',
  'PreviewTIFF',
  'ThumbnailTIFF',
]

function parseArgs(argv) {
  const files = []
  let outDir = join(HERE, 'out', 'exiftool')
  for (let i = 0; i < argv.length; i++) {
    const a = argv[i]
    if (a === '--out') outDir = resolve(argv[++i])
    else files.push(a)
  }
  return { files: files.length ? files : DEFAULT_FILES, outDir }
}

function labelFor(filePath) {
  return resolve(filePath).split(sep).slice(-2).join('/')
}
function safeName(label) {
  return label.replace(/[^\w.-]+/g, '_')
}

// exiftool-vendored returns binary tags as objects carrying a byte count rather
// than the bytes themselves. Detect those so we know which images are present.
function isBinaryField(v) {
  return typeof v === 'object' && v !== null && typeof v.bytes === 'number'
}

async function main() {
  const { files, outDir } = parseArgs(process.argv.slice(2))
  await mkdir(outDir, { recursive: true })
  console.log(`exiftool ${await exiftool.version()} → probing ${files.length} file(s)\n`)

  try {
    for (const file of files) {
      const label = labelFor(file)
      console.log(`══════ ${label}`)
      let tags
      try {
        tags = await exiftool.read(file)
      } catch (err) {
        console.log(`  read ERROR: ${err?.message ?? err}\n`)
        continue
      }

      await writeFile(
        join(outDir, `${safeName(label)}.exiftool.json`),
        JSON.stringify(tags, null, 2),
      )
      console.log(`  ${Object.keys(tags).length} tags`)

      // Camera fields
      console.log('  — camera fields —')
      for (const [field, candidates] of Object.entries(CAMERA_TAGS)) {
        const hit = candidates.find((t) => tags[t] != null && !isBinaryField(tags[t]))
        if (hit) console.log(`    ${field.padEnd(13)} ${hit} = ${JSON.stringify(tags[hit])}`)
      }

      // Embedded images present, then try to extract each.
      const present = IMAGE_TAGS.filter((t) => t in tags)
      const alsoBinary = Object.keys(tags).filter(
        (k) => isBinaryField(tags[k]) && !IMAGE_TAGS.includes(k),
      )
      console.log(
        `  — embedded images — present tags: ${present.join(', ') || 'none of the usual'}`,
      )
      if (alsoBinary.length) console.log(`    other binary tags: ${alsoBinary.join(', ')}`)

      for (const tag of present) {
        const dest = join(outDir, `${safeName(label)}.${tag}.jpg`)
        try {
          await exiftool.extractBinaryTag(tag, file, dest)
          const st = await stat(dest)
          console.log(
            `    ✓ extracted ${tag} → ${basename(dest)} (${(st.size / 1024).toFixed(0)} KB)`,
          )
        } catch (err) {
          console.log(`    ✗ ${tag}: ${err?.message ?? err}`)
        }
      }
      console.log('')
    }
  } finally {
    await exiftool.end()
  }
  console.log(`Done → ${outDir}`)
}

main().catch(async (err) => {
  console.error(err)
  await exiftool.end().catch(() => {})
  process.exit(1)
})
