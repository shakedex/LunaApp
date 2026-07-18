// JPEG marker walking, used to pull pixel dimensions out of an embedded
// preview/thumbnail JPEG (FINDINGS.md "Verified box offsets") and to validate
// scan-found `FF D8 FF … FF D9` candidates before trusting them. ⚠️ FINDINGS.md
// caveat: scanning compressed-RAW essence for the SOI byte pattern produces
// false hits (nonsense SOF dims like 3900×56032) — a candidate is only real
// once its SOF segment parses to sane dimensions.

// SOF0–SOF15 (0xc0–0xcf) except the non-SOF markers that share the range:
// DHT (0xc4), JPG reserved (0xc8), DAC (0xcc).
const NON_SOF_MARKERS_IN_RANGE = new Set([0xc4, 0xc8, 0xcc])

const MIN_SANE_DIMENSION = 1
const MAX_SANE_DIMENSION = 30000

/**
 * Walks JPEG markers from SOI looking for a SOF0-15 segment and returns its
 * pixel dimensions. Returns null when there's no SOI, no SOF segment, or the
 * byte stream is truncated/malformed partway through the walk.
 */
export function jpegDimensions(bytes: Uint8Array): { width: number; height: number } | null {
  if (bytes.length < 4 || bytes[0] !== 0xff || bytes[1] !== 0xd8) return null

  let i = 2
  while (i + 1 < bytes.length) {
    if (bytes[i] !== 0xff) {
      i++
      continue
    }
    const marker = bytes[i + 1]
    if (marker === 0xff) {
      // Fill byte before the real marker code — re-examine at i+1.
      i++
      continue
    }
    // Markers with no length-prefixed payload: SOI/EOI, RSTn, TEM.
    if (
      marker === 0xd8 ||
      marker === 0xd9 ||
      (marker >= 0xd0 && marker <= 0xd7) ||
      marker === 0x01
    ) {
      i += 2
      continue
    }

    if (i + 3 >= bytes.length) return null
    const segmentLength = (bytes[i + 2] << 8) | bytes[i + 3]
    if (segmentLength < 2) return null

    const isSof = marker >= 0xc0 && marker <= 0xcf && !NON_SOF_MARKERS_IN_RANGE.has(marker)
    if (isSof) {
      // Segment layout: marker(2) length(2) precision(1) height(2) width(2) ...
      if (i + 8 >= bytes.length) return null
      const height = (bytes[i + 5] << 8) | bytes[i + 6]
      const width = (bytes[i + 7] << 8) | bytes[i + 8]
      return { width, height }
    }

    i += 2 + segmentLength
  }
  return null
}

export interface JpegCandidate {
  offset: number
  length: number
  width: number
  height: number
}

/**
 * Scans raw bytes for `FF D8 FF … FF D9` candidate spans (e.g. a box payload
 * that might embed a preview JPEG). A candidate is only returned once
 * jpegDimensions succeeds on it AND both dimensions are sane — this is what
 * rejects the false-positive SOI hits FINDINGS.md warns about.
 */
export function findValidJpegs(bytes: Uint8Array): JpegCandidate[] {
  const candidates: JpegCandidate[] = []
  let i = 0
  while (i + 3 <= bytes.length) {
    if (bytes[i] === 0xff && bytes[i + 1] === 0xd8 && bytes[i + 2] === 0xff) {
      const eoi = findEoi(bytes, i + 2)
      if (eoi === -1) {
        i++
        continue
      }
      const length = eoi + 2 - i
      const dims = jpegDimensions(bytes.subarray(i, i + length))
      if (dims && isSaneDimension(dims.width) && isSaneDimension(dims.height)) {
        candidates.push({ offset: i, length, width: dims.width, height: dims.height })
      }
      i = eoi + 2
      continue
    }
    i++
  }
  return candidates
}

function findEoi(bytes: Uint8Array, from: number): number {
  for (let j = from; j + 1 < bytes.length; j++) {
    if (bytes[j] === 0xff && bytes[j + 1] === 0xd9) return j
  }
  return -1
}

function isSaneDimension(n: number): boolean {
  return n >= MIN_SANE_DIMENSION && n <= MAX_SANE_DIMENSION
}
