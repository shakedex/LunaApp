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
  return walkMarkers(bytes, 2, { stopAtSof: true })?.dims ?? null
}

interface WalkResult {
  /** Index one past the terminating FF D9, i.e. the parsed span's exclusive end. */
  end: number
  dims: { width: number; height: number } | null
}

/**
 * Walks JPEG segment markers starting at `start` (just past the SOI this
 * candidate begins with). Two modes share this walk:
 *  - `stopAtSof: true` (jpegDimensions): return as soon as a SOF segment's
 *    dims are known, without requiring a well-formed EOI/entropy stream.
 *  - `stopAtSof: false` (parseJpegAt): keep walking through SOS's
 *    entropy-coded data (byte-stuffed `FF 00` and `FF D0-D7` restarts don't
 *    end it) until a true `FF D9`, and only then return — this is what makes
 *    the parsed span end where the JPEG actually ends, not at the first
 *    incidental `FF D9` byte pair.
 */
function walkMarkers(
  bytes: Uint8Array,
  start: number,
  opts: { stopAtSof: boolean },
): WalkResult | null {
  let i = start
  let dims: { width: number; height: number } | null = null
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
    if (marker === 0xd9) {
      // EOI — the candidate ends here (whether or not we ever saw SOS).
      return { end: i + 2, dims }
    }
    if (marker === 0xd8) {
      // A second SOI outside entropy-coded data means this isn't a single
      // well-formed JPEG stream from the SOI we started at — most often a
      // decoy `FF D8 FF` hit whose "walk" would otherwise wander into and
      // swallow a real, independent JPEG that happens to follow it. Treat
      // as malformed rather than silently absorbing whatever comes after.
      return null
    }
    // Markers with no length-prefixed payload: RSTn, TEM.
    if ((marker >= 0xd0 && marker <= 0xd7) || marker === 0x01) {
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
      dims = { width, height }
      if (opts.stopAtSof) return { end: i + 2 + segmentLength, dims }
    }

    i += 2 + segmentLength

    if (marker === 0xda) {
      // SOS — the rest is entropy-coded scan data, not more segments. Skip
      // over it (respecting FF 00 stuffing and FF D0-D7 restart markers)
      // until the true FF D9 EOI, since scan data can itself contain byte
      // pairs that look like markers.
      const scanEnd = skipEntropyData(bytes, i)
      if (scanEnd === -1) return null
      i = scanEnd
      if (opts.stopAtSof) continue
      // Caller wants the full span: the next FF D9 loop iteration finds it.
    }
  }
  return null
}

/**
 * Advances past entropy-coded scan data starting at `i`, stopping right
 * before the true `FF D9` EOI marker. Returns -1 if no true EOI is found
 * before the stream ends (malformed/truncated).
 */
function skipEntropyData(bytes: Uint8Array, i: number): number {
  while (i + 1 < bytes.length) {
    if (bytes[i] !== 0xff) {
      i++
      continue
    }
    const next = bytes[i + 1]
    if (next === 0x00 || (next >= 0xd0 && next <= 0xd7)) {
      // Byte-stuffed FF or a restart marker: part of the entropy stream.
      i += 2
      continue
    }
    if (next === 0xd9) return i
    // Any other marker byte inside scan data — treat as not-yet-EOI and
    // keep scanning (some encoders emit padding fill bytes here too).
    i++
  }
  return -1
}

export interface JpegCandidate {
  offset: number
  length: number
  width: number
  height: number
}

/**
 * Parses a single JPEG candidate starting at an `FF D8` SOI found at
 * `offset`. Unlike a naive "find the next FF D9" scan, this walks the
 * stream's own segment structure — including entropy-coded scan data after
 * SOS, where `FF 00` byte-stuffing and `FF D0-D7` restart markers must be
 * skipped rather than mistaken for the end — so the returned span is the
 * true extent of *this* JPEG, not wherever the first incidental `FF D9`
 * byte pair happens to occur (which may belong to an unrelated decoy SOI
 * that precedes a real image, or a byte pair inside compressed essence).
 *
 * Returns null if there's no SOI at offset, the walk hits truncated/
 * malformed data, or no SOF segment was found to report dimensions from.
 */
function parseJpegAt(
  bytes: Uint8Array,
  offset: number,
): { length: number; width: number; height: number } | null {
  if (bytes[offset] !== 0xff || bytes[offset + 1] !== 0xd8) return null
  const result = walkMarkers(bytes, offset + 2, { stopAtSof: false })
  if (!result || !result.dims) return null
  return { length: result.end - offset, width: result.dims.width, height: result.dims.height }
}

/**
 * Scans raw bytes for `FF D8 FF …` candidates (e.g. a box payload that
 * might embed a preview JPEG), parsing each independently from its own SOI
 * via parseJpegAt. A candidate is only returned once the walk succeeds AND
 * both dimensions are sane — this is what rejects the false-positive SOI
 * hits FINDINGS.md warns about, without conflating a decoy SOI's span with
 * a real JPEG that happens to follow it.
 */
export function findValidJpegs(bytes: Uint8Array): JpegCandidate[] {
  const candidates: JpegCandidate[] = []
  let i = 0
  while (i + 3 <= bytes.length) {
    if (bytes[i] === 0xff && bytes[i + 1] === 0xd8 && bytes[i + 2] === 0xff) {
      const parsed = parseJpegAt(bytes, i)
      if (parsed && isSaneDimension(parsed.width) && isSaneDimension(parsed.height)) {
        candidates.push({
          offset: i,
          length: parsed.length,
          width: parsed.width,
          height: parsed.height,
        })
        i += parsed.length
        continue
      }
      // Decoy: step past just this SOI byte so a nested real SOI starting
      // one or more bytes later is still found.
      i++
      continue
    }
    i++
  }
  return candidates
}

function isSaneDimension(n: number): boolean {
  return n >= MIN_SANE_DIMENSION && n <= MAX_SANE_DIMENSION
}
