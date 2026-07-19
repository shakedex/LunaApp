import { describe, expect, test } from 'bun:test'
import {
  CRM_PREVIEW_UUID,
  extractCrmPreview,
  extractMovTailPreview,
  extractRtnJpeg,
} from '../src/preview/extract'
import type { BlobLike } from '../src/scan/handles'

// ---- byte-building helpers (mirrors test/boxes.test.ts and test/jpeg.test.ts) ----

function u32(n: number): number[] {
  return [n >>> 24, (n >>> 16) & 0xff, (n >>> 8) & 0xff, n & 0xff]
}
function u64(n: bigint): number[] {
  const bytes: number[] = []
  for (let shift = 56n; shift >= 0n; shift -= 8n) bytes.push(Number((n >> shift) & 0xffn))
  return bytes
}
function box(type: string, ...payload: number[][]): number[] {
  const body = payload.flat()
  return [...u32(8 + body.length), ...[...type].map((c) => c.charCodeAt(0)), ...body]
}
function uuidBox(id: Uint8Array, ...payload: number[][]): number[] {
  const body = payload.flat()
  const idArr = Array.from(id)
  return [
    ...u32(8 + idArr.length + body.length),
    ...[...'uuid'].map((c) => c.charCodeAt(0)),
    ...idArr,
    ...body,
  ]
}
// A box header declaring size===1 (64-bit largesize) with NO body materialized
// after it, so the walker must jump by declared size, never read the body.
function largesizeHeaderOnly(type: string, largeSize: bigint): number[] {
  return [...u32(1), ...[...type].map((c) => c.charCodeAt(0)), ...u64(largeSize)]
}
function fakeBlobOf(bytes: number[]): BlobLike {
  const arr = Uint8Array.from(bytes)
  function make(offset: number, length: number): BlobLike {
    return {
      size: length,
      slice(start = 0, end = length) {
        const from = offset + Math.max(0, start)
        const to = offset + Math.min(end, length)
        return make(from, Math.max(0, to - from))
      },
      async arrayBuffer() {
        return arr.slice(offset, offset + length).buffer
      },
    }
  }
  return make(0, arr.length)
}

function minimalJpeg(width: number, height: number): number[] {
  return [
    0xff,
    0xd8, // SOI
    0xff,
    0xc0,
    0x00,
    0x11,
    0x08, // SOF0, len 17, precision 8
    (height >> 8) & 0xff,
    height & 0xff,
    (width >> 8) & 0xff,
    width & 0xff,
    0x03,
    0x01,
    0x11,
    0x00,
    0x02,
    0x11,
    0x00,
    0x03,
    0x11,
    0x00, // 3 components
    0xff,
    0xd9, // EOI
  ]
}

const WRONG_UUID = Uint8Array.from(Array.from({ length: 16 }, () => 0x00))

describe('extractCrmPreview', () => {
  test('finds the primary JPEG at box start + headerSize(24) + PRVW header(40)', async () => {
    const ftyp = box('ftyp', [0x69, 0x73, 0x6f, 0x6d])
    const filler = box('free', [1, 2, 3, 4])
    const prvwHeaderFiller = Array(40).fill(0xab)
    const jpeg = minimalJpeg(2048, 1080)
    const crmBox = uuidBox(CRM_PREVIEW_UUID, prvwHeaderFiller, jpeg)
    const blob = fakeBlobOf([...ftyp, ...filler, ...crmBox])

    const result = await extractCrmPreview(blob)

    expect(result).not.toBeNull()
    expect(result?.width).toBe(2048)
    expect(result?.height).toBe(1080)
    expect(Array.from(result?.jpeg ?? [])).toEqual(jpeg)
  })

  test('returns null when no top-level uuid box matches CRM_PREVIEW_UUID', async () => {
    const ftyp = box('ftyp', [0x69, 0x73, 0x6f, 0x6d])
    const prvwHeaderFiller = Array(40).fill(0xab)
    const jpeg = minimalJpeg(2048, 1080)
    const wrongBox = uuidBox(WRONG_UUID, prvwHeaderFiller, jpeg)
    const blob = fakeBlobOf([...ftyp, ...wrongBox])

    expect(await extractCrmPreview(blob)).toBeNull()
  })

  test('falls back to the largest valid JPEG in the slice when the primary offset misses', async () => {
    // Deliberately shift the JPEG so it does NOT sit at headerSize+40 — the
    // primary lookup misses, but findValidJpegs still finds it in the slice.
    const ftyp = box('ftyp', [0x69, 0x73, 0x6f, 0x6d])
    const shiftedFiller = Array(48).fill(0xcd) // 8 bytes more than the real PRVW header
    const jpeg = minimalJpeg(1024, 768)
    const crmBox = uuidBox(CRM_PREVIEW_UUID, shiftedFiller, jpeg)
    const blob = fakeBlobOf([...ftyp, ...crmBox])

    const result = await extractCrmPreview(blob)

    expect(result?.width).toBe(1024)
    expect(result?.height).toBe(768)
  })
})

describe('extractMovTailPreview', () => {
  test('walks past a 64-bit largesize mdat to moov/udta and picks the LARGER of two JPEGs', async () => {
    const ftyp = box('ftyp', [0x69, 0x73, 0x6f, 0x6d])
    // Declares its size via the 64-bit largesize field (size===1), exercising
    // the same header-expansion path FINDINGS.md's real 5.6GB DJI mdat uses —
    // but sized to exactly the header's own length so this fixture's moov
    // (placed immediately after, as bytes) sits exactly where the walker's
    // jump lands, without materializing gigabytes of filler.
    const mdatHeader = largesizeHeaderOnly('mdat', 16n)
    const small = minimalJpeg(448, 240)
    const filler = [0, 0, 0, 0]
    const large = minimalJpeg(1920, 1012)
    const udta = box('udta', small, filler, large)
    const moov = box('moov', udta)
    const blob = fakeBlobOf([...ftyp, ...mdatHeader, ...moov])

    const result = await extractMovTailPreview(blob)

    expect(result?.width).toBe(1920)
    expect(result?.height).toBe(1012)
  })

  test('returns null when there is no moov box', async () => {
    const ftyp = box('ftyp', [0x69, 0x73, 0x6f, 0x6d])
    const blob = fakeBlobOf([...ftyp])
    expect(await extractMovTailPreview(blob)).toBeNull()
  })

  test('returns null when moov has no udta child', async () => {
    const ftyp = box('ftyp', [0x69, 0x73, 0x6f, 0x6d])
    const trak = box('trak', [1, 2, 3, 4])
    const moov = box('moov', trak)
    const blob = fakeBlobOf([...ftyp, ...moov])
    expect(await extractMovTailPreview(blob)).toBeNull()
  })
})

describe('extractRtnJpeg', () => {
  function rtnFrame(jpeg: number[]): number[] {
    const header = [...'REDTHUMBNAIL'].map((c) => c.charCodeAt(0)) // 12 ascii
    const reserved = [0, 0] // 2 bytes
    const length = jpeg.length
    const lengthLE = [
      length & 0xff,
      (length >> 8) & 0xff,
      (length >> 16) & 0xff,
      (length >> 24) & 0xff,
    ]
    return [...header, ...reserved, ...lengthLE, ...jpeg]
  }

  test('parses the exact 18-byte header framing', () => {
    const jpeg = minimalJpeg(720, 405)
    const bytes = Uint8Array.from(rtnFrame(jpeg))

    const result = extractRtnJpeg(bytes)

    expect(result?.width).toBe(720)
    expect(result?.height).toBe(405)
    expect(Array.from(result?.jpeg ?? [])).toEqual(jpeg)
  })

  test('falls back to findValidJpegs when the header is garbage', () => {
    const jpeg = minimalJpeg(320, 240)
    const bytes = Uint8Array.from([0xde, 0xad, 0xbe, 0xef, 0x00, 0x00, ...jpeg])

    const result = extractRtnJpeg(bytes)

    expect(result?.width).toBe(320)
    expect(result?.height).toBe(240)
  })

  test('returns null when neither the header nor a fallback scan finds anything', () => {
    const bytes = Uint8Array.from([0xde, 0xad, 0xbe, 0xef, 0x00, 0x00])
    expect(extractRtnJpeg(bytes)).toBeNull()
  })
})
