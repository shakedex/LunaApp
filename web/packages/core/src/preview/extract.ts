// Embedded-preview extraction for the RAW formats that ship a grabbable JPEG
// instead of a decodable frame (FINDINGS.md "Verified box offsets" +
// "RED KOMODO .r3d"): Canon Cinema RAW Light's top-level `uuid[PRVW]` box,
// ProRes RAW's tail `moov/udta`, and RED's `.rtn` sidecar. All three read a
// small, bounded slice (never the multi-gigabyte `mdat`) and hand the result
// through `findValidJpegs`/`jpegDimensions` so a candidate is only trusted
// once it parses to sane dimensions (FINDINGS.md's false-positive-SOI caveat).

import type { BlobLike } from '../scan/handles'
import { type BoxHeader, findChildBox, walkTopLevelBoxes } from './boxes'
import { findValidJpegs, type JpegCandidate, jpegDimensions } from './jpeg'

export interface EmbeddedPreview {
  jpeg: Uint8Array
  width: number
  height: number
}

// eaf42b5e-1c98-4b88-b9fb-b7dc406e4d16 (FINDINGS.md, Canon .crm top-level uuid)
export const CRM_PREVIEW_UUID: Uint8Array = Uint8Array.from([
  0xea, 0xf4, 0x2b, 0x5e, 0x1c, 0x98, 0x4b, 0x88, 0xb9, 0xfb, 0xb7, 0xdc, 0x40, 0x6e, 0x4d, 0x16,
])

// Never materialize more than this much of a box/child-box into memory, even
// though these boxes are normally far smaller (the verified crm PRVW box is
// ~441KB; ProRes RAW's udta ~1.2MB) — a defensive ceiling against malformed
// or hostile size fields.
const SLICE_CAP_BYTES = 16 * 1024 * 1024

// Inner `PRVW` header length inside the crm uuid box's payload (FINDINGS.md:
// "uuid 16 + inner PRVW header 40" = the 56-byte figure quoted there).
const PRVW_HEADER_SIZE = 40

/**
 * Canon Cinema RAW Light (.crm): walk top-level boxes for the uuid box whose
 * 16-byte id is CRM_PREVIEW_UUID, slice it (capped), and prefer the JPEG that
 * sits at the well-known primary offset; fall back to the largest valid JPEG
 * anywhere in the slice if that offset doesn't hold one.
 *
 * Arithmetic reconciliation with FINDINGS.md's "box start + 56" figure:
 * FINDINGS.md's box-offsets.mjs reports the uuid box's *id* offset as "box
 * start" (i.e. it excludes the leading 8-byte size+type prefix), so its "+56"
 * = 16 (uuid id) + 40 (PRVW header), measured from the id, not from the file
 * offset of the box's size+type field.
 *
 * Our `BoxHeader.start` (from boxes.ts) is the absolute offset of the box's
 * size+type prefix, and `BoxHeader.headerSize` for a uuid box is 24 (8-byte
 * base header + 16-byte id) and already includes those id bytes — per Task
 * 2's bridge note, "payload starts uniformly at start + headerSize". So here
 * the primary JPEG offset relative to `box.start` is:
 *   box.headerSize (24) + PRVW_HEADER_SIZE (40) = 64
 * not 56 — the two figures describe the exact same absolute file offset,
 * they just measure from different reference points (id-start vs.
 * size+type-start, an 8-byte difference). Verified against FINDINGS.md's
 * concrete numbers: uuid id offset 94912 + 56 = 94968 (the JPEG); box.start
 * (94912 - 8 = 94904) + 64 = 94968 — same offset.
 */
export async function extractCrmPreview(blob: BlobLike): Promise<EmbeddedPreview | null> {
  let match: BoxHeader | null = null
  await walkTopLevelBoxes(blob, (box) => {
    if (box.type === 'uuid' && box.uuid && bytesEqual(box.uuid, CRM_PREVIEW_UUID)) {
      match = box
      return 'stop'
    }
    return 'continue'
  })
  if (!match) return null

  const found: BoxHeader = match
  const len = Math.min(found.size, SLICE_CAP_BYTES, blob.size - found.start)
  const bytes = new Uint8Array(await blob.slice(found.start, found.start + len).arrayBuffer())

  const candidates = findValidJpegs(bytes)
  if (candidates.length === 0) return null

  const primaryOffset = found.headerSize + PRVW_HEADER_SIZE
  const primary = candidates.find((c) => c.offset === primaryOffset)
  const chosen = primary ?? pickLargest(candidates)
  return previewFromCandidate(bytes, chosen)
}

/**
 * ProRes RAW (and similar): walk top-level boxes (a plain top-level walk
 * already jumps a 64-bit largesize `mdat` by its declared size — no special
 * handling needed) to `moov`, find its `udta` child, slice it (capped), and
 * take the LARGEST valid JPEG found (FINDINGS.md: udta holds a 448×240 AND a
 * 1920×1012 preview — the bigger one wins).
 */
export async function extractMovTailPreview(blob: BlobLike): Promise<EmbeddedPreview | null> {
  let moov: BoxHeader | null = null
  await walkTopLevelBoxes(blob, (box) => {
    if (box.type === 'moov') {
      moov = box
      return 'stop'
    }
    return 'continue'
  })
  if (!moov) return null

  const udta = await findChildBox(blob, moov, 'udta')
  if (!udta) return null

  const len = Math.min(udta.size, SLICE_CAP_BYTES, blob.size - udta.start)
  const bytes = new Uint8Array(await blob.slice(udta.start, udta.start + len).arrayBuffer())

  const candidates = findValidJpegs(bytes)
  if (candidates.length === 0) return null

  return previewFromCandidate(bytes, pickLargest(candidates))
}

const RTN_MAGIC = 'REDTHUMBNAIL'
const RTN_HEADER_SIZE = 18 // 12 ascii + 2 reserved + 4-byte u32LE length

/**
 * RED `.rtn` sidecar: `"REDTHUMBNAIL"` (12 ascii bytes) + 2 reserved bytes +
 * a u32LE JPEG length at offset 14, JPEG bytes starting at offset 18
 * (FINDINGS.md). Validates the framed JPEG via jpegDimensions; falls back to
 * the largest findValidJpegs hit over the whole buffer if the header doesn't
 * match or the framed span doesn't parse.
 */
export function extractRtnJpeg(bytes: Uint8Array): EmbeddedPreview | null {
  if (bytes.length >= RTN_HEADER_SIZE && matchesRtnMagic(bytes)) {
    const length = bytes[14] | (bytes[15] << 8) | (bytes[16] << 16) | (bytes[17] << 24)
    const jpegEnd = Math.min(RTN_HEADER_SIZE + length, bytes.length)
    const jpeg = bytes.slice(RTN_HEADER_SIZE, jpegEnd)
    const dims = jpegDimensions(jpeg)
    if (dims) return { jpeg, width: dims.width, height: dims.height }
  }

  const candidates = findValidJpegs(bytes)
  if (candidates.length === 0) return null
  return previewFromCandidate(bytes, pickLargest(candidates))
}

function matchesRtnMagic(bytes: Uint8Array): boolean {
  for (let i = 0; i < RTN_MAGIC.length; i++) {
    if (bytes[i] !== RTN_MAGIC.charCodeAt(i)) return false
  }
  return true
}

function pickLargest(candidates: JpegCandidate[]): JpegCandidate {
  return candidates.reduce((best, c) => (c.width * c.height > best.width * best.height ? c : best))
}

function previewFromCandidate(bytes: Uint8Array, candidate: JpegCandidate): EmbeddedPreview {
  return {
    jpeg: bytes.slice(candidate.offset, candidate.offset + candidate.length),
    width: candidate.width,
    height: candidate.height,
  }
}

function bytesEqual(a: Uint8Array, b: Uint8Array): boolean {
  if (a.length !== b.length) return false
  for (let i = 0; i < a.length; i++) if (a[i] !== b[i]) return false
  return true
}
