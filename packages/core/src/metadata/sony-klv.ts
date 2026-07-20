// Sony MXF camera-model scan (FINDINGS.md 2026-07-20 X-OCN probes).
//
// Sony records the RDD-18 CameraAttributes item ("MPC-3626 0010119
// Version3.00") once per frame inside the MXF essence container, but
// mediainfo.js does not surface that item — so the generic mapper can only say
// camera: "Sony". This module walks the file's top-level KLV packets
// (16-byte UL key + BER length + value), reads ONLY values small enough to be
// metadata (the multi-gigabyte picture essence is jumped by its declared
// length, same never-read-the-body discipline as preview/boxes.ts), and scans
// them for the model code.
//
// Two in-file sources cover the corpus: the RDD-18 CameraAttributes string
// (Venice/BURANO AXS recordings) and XDCAM-style embedded XML with a
// modelName attribute (FX6 XAVC recordings — sits in the header metadata, no
// sidecar involved). Corpus-verified codes: MPC-2610 = BURANO,
// MPC-3610 = VENICE, MPC-3626 = VENICE 2 (8K), ILME-FX6V = FX6. Unknown codes
// surface honestly as "Sony <code>" rather than being guessed at.

import type { BlobLike } from '../scan/handles'

const KLV_KEY_SIZE = 16
// SMPTE UL designator every MXF KLV key starts with.
const UL_PREFIX = [0x06, 0x0e, 0x2b, 0x34]
// BER long-form length: at most 8 length bytes (values beyond 2^53 are
// rejected anyway), so a full KLV header fits in 16 + 1 + 8 bytes.
const MAX_KLV_HEADER_SIZE = KLV_KEY_SIZE + 1 + 8
// Values at or under this size are read and scanned; anything bigger is
// essence (picture/oversized payloads) and is jumped, never materialized.
const VALUE_READ_CAP = 1024 * 1024
// The model string appears in frame 1's metadata element — for the largest
// corpus clip (VENICE 2 8.6K) that is ~19.5 MB in. 256 MB leaves headroom for
// bigger frames without letting a pathological file drag the scan on forever.
const MAX_SCAN_OFFSET = 256 * 1024 * 1024
const MAX_PACKETS = 4096

const SONY_MODEL_BY_CODE: Record<string, string> = {
  'MPC-2610': 'BURANO',
  'MPC-3610': 'VENICE',
  'MPC-3626': 'VENICE 2',
  'ILME-FX6V': 'FX6',
}

// CameraAttributes-style device code ("MPC-3626 0010119 Version3.00" —
// Venice/BURANO AXS recordings).
const MODEL_CODE_PATTERN = /\bMPC-\d{4}\b/
// XDCAM-style embedded XML ('modelName="ILME-FX6V"' — FX6 XAVC recordings).
// Tight charset so binary noise can't fake a match.
const MODEL_NAME_ATTR_PATTERN = /\bmodelName="([A-Za-z0-9-]{2,32})"/

const LATIN1 = new TextDecoder('latin1')

/**
 * Maps a scanned payload (any text containing a Sony device code — a
 * CameraAttributes string or an embedded-XML modelName attribute) to a display
 * name: corpus-verified codes get their model name, unknown codes stay honest
 * ("Sony MPC-xxxx"), no code means undefined.
 */
export function sonyCameraDisplayName(text: string): string | undefined {
  const code = text.match(MODEL_CODE_PATTERN)?.[0] ?? text.match(MODEL_NAME_ATTR_PATTERN)?.[1]
  if (!code) return undefined
  return SONY_MODEL_BY_CODE[code] ?? `Sony ${code}`
}

interface KlvHeader {
  valueStart: number
  valueLength: number
}

/**
 * Reads one KLV header at `offset`: 16-byte UL key + BER length. Returns null
 * for anything that is not a plausible KLV packet — never throws.
 */
async function readKlvHeaderAt(blob: BlobLike, offset: number): Promise<KlvHeader | null> {
  if (offset < 0 || offset + KLV_KEY_SIZE + 1 > blob.size) return null
  const end = Math.min(offset + MAX_KLV_HEADER_SIZE, blob.size)
  const bytes = new Uint8Array(await blob.slice(offset, end).arrayBuffer())
  if (bytes.length < KLV_KEY_SIZE + 1) return null

  for (let i = 0; i < UL_PREFIX.length; i++) {
    if (bytes[i] !== UL_PREFIX[i]) return null
  }

  const first = bytes[KLV_KEY_SIZE]
  if (first < 0x80) {
    return { valueStart: offset + KLV_KEY_SIZE + 1, valueLength: first }
  }
  const tailCount = first & 0x7f
  // 0x80 alone is BER "indefinite length" — not valid in MXF.
  if (tailCount === 0 || tailCount > 8 || KLV_KEY_SIZE + 1 + tailCount > bytes.length) return null
  let length = 0
  for (let i = 0; i < tailCount; i++) {
    length = length * 256 + bytes[KLV_KEY_SIZE + 1 + i]
    if (!Number.isSafeInteger(length)) return null
  }
  return { valueStart: offset + KLV_KEY_SIZE + 1 + tailCount, valueLength: length }
}

/**
 * Walks top-level KLV packets from the start of a (presumed Sony) MXF, scanning
 * small values for the camera model code. Bounded by packet count, byte
 * offset, and per-value read cap; returns undefined the moment the stream
 * stops looking like KLV.
 */
export async function extractSonyMxfCameraModel(blob: BlobLike): Promise<string | undefined> {
  let offset = 0
  for (let packet = 0; packet < MAX_PACKETS; packet++) {
    if (offset + KLV_KEY_SIZE + 1 > blob.size || offset > MAX_SCAN_OFFSET) return undefined
    const header = await readKlvHeaderAt(blob, offset)
    if (!header) return undefined
    if (header.valueLength > 0 && header.valueLength <= VALUE_READ_CAP) {
      const valueEnd = Math.min(header.valueStart + header.valueLength, blob.size)
      if (valueEnd > header.valueStart) {
        const value = new Uint8Array(await blob.slice(header.valueStart, valueEnd).arrayBuffer())
        const model = sonyCameraDisplayName(LATIN1.decode(value))
        if (model) return model
      }
    }
    offset = header.valueStart + header.valueLength
  }
  return undefined
}
