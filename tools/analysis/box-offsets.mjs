#!/usr/bin/env bun
/**
 * box-offsets.mjs — ISO-BMFF / QuickTime box walker (Luna Web diagnostic).
 *
 * Verifies exactly where an embedded preview/thumbnail JPEG lives inside a
 * container, so the browser box-reader can be implemented against concrete
 * box paths + byte offsets. Seeks through the file (reads box headers and small
 * leaf payloads only) — never loads mdat or the multi-GB essence.
 *
 * Run (from the repo root):
 *   bun tools/analysis/box-offsets.mjs path/to/clip.crm path/to/clip.mov
 *
 * Prints the box tree (offset+size, mdat skipped) and every JPEG found with its
 * absolute file offset, length, and pixel dimensions.
 */

import { open, stat } from 'node:fs/promises'

const CONTAINERS = new Set([
  'moov',
  'trak',
  'mdia',
  'minf',
  'stbl',
  'udta',
  'ilst',
  'covr',
  'edts',
  'dinf',
  'gmhd',
])
const SOI = Buffer.from([0xff, 0xd8, 0xff])
const EOI = Buffer.from([0xff, 0xd9])
const MAX_LEAF_SCAN = 12 * 1024 * 1024 // previews are ≤ ~1-2 MB; cap generously

async function readAt(fh, pos, len) {
  const buf = Buffer.alloc(len)
  const { bytesRead } = await fh.read(buf, 0, len, pos)
  return buf.subarray(0, bytesRead)
}

function uuidStr(b) {
  const h = b.toString('hex')
  return `${h.slice(0, 8)}-${h.slice(8, 12)}-${h.slice(12, 16)}-${h.slice(16, 20)}-${h.slice(20, 32)}`
}

function isPrintableType(t) {
  return /^[\x20-\x7e]{4}$/.test(t)
}

// Find SOF marker after a JPEG SOI and return {w,h}.
function jpegDims(buf, start) {
  let i = start + 2
  while (i + 9 < buf.length) {
    if (buf[i] !== 0xff) {
      i++
      continue
    }
    const marker = buf[i + 1]
    if (
      marker === 0xd8 ||
      marker === 0xd9 ||
      (marker >= 0xd0 && marker <= 0xd7) ||
      marker === 0x01
    ) {
      i += 2
      continue
    }
    const len = buf.readUInt16BE(i + 2)
    const isSOF =
      (marker >= 0xc0 && marker <= 0xc3) ||
      (marker >= 0xc5 && marker <= 0xc7) ||
      (marker >= 0xc9 && marker <= 0xcb) ||
      (marker >= 0xcd && marker <= 0xcf)
    if (isSOF) return { h: buf.readUInt16BE(i + 5), w: buf.readUInt16BE(i + 7) }
    i += 2 + len
  }
  return null
}

async function walk(fh, start, end, path, out, depth = 0) {
  let pos = start
  while (pos + 8 <= end) {
    const hdr = await readAt(fh, pos, 16)
    if (hdr.length < 8) break
    let size = hdr.readUInt32BE(0)
    const type = hdr.toString('latin1', 4, 8)
    let headerSize = 8
    if (size === 1) {
      size = Number(hdr.readBigUInt64BE(8))
      headerSize = 16
    } else if (size === 0) {
      size = end - pos
    }
    if (!isPrintableType(type) || size < headerSize || pos + size > end) break

    let payloadStart = pos + headerSize
    let label = type
    if (type === 'uuid') {
      const u = await readAt(fh, payloadStart, 16)
      label = `uuid[${uuidStr(u)}]`
      payloadStart += 16
    }
    const nodePath = [...path, label]
    out.tree.push(`${'  '.repeat(depth)}${label}  @${pos}  size=${size}`)

    if (type === 'mdat' || type === 'free' || type === 'skip' || type === 'wide') {
      // skip payload
    } else if (CONTAINERS.has(type) || type === 'uuid' || type === 'meta') {
      let childStart = payloadStart
      if (type === 'meta') {
        // ISO 'meta' is a FullBox (4-byte version/flags); QuickTime 'meta' isn't.
        const peek = await readAt(fh, payloadStart, 8)
        const isoFirst = peek.toString('latin1', 4, 8)
        if (peek.readUInt32BE(0) === 0 && isPrintableType(isoFirst)) childStart = payloadStart + 4
      }
      await walk(fh, childStart, pos + size, nodePath, out, depth + 1)
    } else if (size <= MAX_LEAF_SCAN) {
      const payLen = pos + size - payloadStart
      const buf = await readAt(fh, payloadStart, Math.min(payLen, MAX_LEAF_SCAN))
      const soi = buf.indexOf(SOI)
      if (soi >= 0) {
        const eoi = buf.indexOf(EOI, soi + 2)
        const jpegOffset = payloadStart + soi
        const jpegLen = eoi >= 0 ? eoi + 2 - soi : payLen - soi
        out.jpegs.push({
          path: nodePath.join('/'),
          boxOffset: pos,
          boxSize: size,
          jpegOffset,
          jpegLen,
          dims: jpegDims(buf, soi),
        })
      }
    }
    pos += size
  }
}

async function main() {
  const files = process.argv.slice(2)
  if (!files.length) {
    console.error('Usage: bun tools/box-offsets.mjs <file> [file...]')
    process.exit(1)
  }
  for (const file of files) {
    const { size } = await stat(file)
    const fh = await open(file, 'r')
    const out = { tree: [], jpegs: [] }
    try {
      await walk(fh, 0, size, [], out)
    } finally {
      await fh.close()
    }
    console.log(`\n══════════════════════════════════════════════════════════════`)
    console.log(`${file}\n  fileSize=${size}`)
    console.log('\n— box tree (mdat payload skipped) —')
    console.log(out.tree.join('\n'))
    console.log('\n— embedded JPEGs —')
    if (!out.jpegs.length) console.log('  none found')
    for (const j of out.jpegs) {
      const d = j.dims ? `${j.dims.w}×${j.dims.h}` : '??'
      console.log(
        `  ${d.padEnd(11)} ${(j.jpegLen / 1024).toFixed(0).padStart(5)} KB  @offset ${j.jpegOffset}  len ${j.jpegLen}\n      path: ${j.path}  (box @${j.boxOffset} size ${j.boxSize})`,
      )
    }
  }
}

main().catch((e) => {
  console.error(e)
  process.exit(1)
})
