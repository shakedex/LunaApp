import { describe, expect, test } from 'bun:test'
import { findValidJpegs, jpegDimensions } from '../src/preview/jpeg'

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

// A fake SOI hit with a SOF segment whose dims are nonsense (FINDINGS.md's
// real-world example from scanning compressed-RAW essence: 3900×56032).
function insaneJpeg(): number[] {
  const width = 3900
  const height = 56032
  return [
    0xff,
    0xd8,
    0xff,
    0xc0,
    0x00,
    0x11,
    0x08,
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
    0x00,
    0xff,
    0xd9,
  ]
}

describe('jpegDimensions', () => {
  test('reads width/height from a SOF0 marker', () => {
    const bytes = Uint8Array.from(minimalJpeg(2048, 1080))
    expect(jpegDimensions(bytes)).toEqual({ width: 2048, height: 1080 })
  })

  test('finds SOF among preceding APP/other segments', () => {
    const app0 = [0xff, 0xe0, 0x00, 0x04, 0xab, 0xcd] // APP0, len 4, 2 payload bytes
    const bytes = Uint8Array.from([0xff, 0xd8, ...app0, ...minimalJpeg(640, 480).slice(2)])
    expect(jpegDimensions(bytes)).toEqual({ width: 640, height: 480 })
  })

  test('returns null when there is no SOI', () => {
    const bytes = Uint8Array.from([0x00, 0x01, 0x02, 0x03])
    expect(jpegDimensions(bytes)).toBeNull()
  })

  test('returns null on truncated JPEG (cut mid-SOF-segment)', () => {
    const full = minimalJpeg(2048, 1080)
    const truncated = Uint8Array.from(full.slice(0, 8)) // stops before height/width bytes
    expect(jpegDimensions(truncated)).toBeNull()
  })

  test('returns null on garbage bytes', () => {
    const bytes = Uint8Array.from([0xff, 0xd8, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66])
    expect(jpegDimensions(bytes)).toBeNull()
  })

  test('a second SOI before SOF makes the stream malformed', () => {
    const real = minimalJpeg(320, 240)
    expect(jpegDimensions(new Uint8Array([0xff, 0xd8, ...real]))).toBeNull()
  })
})

describe('findValidJpegs', () => {
  test('rejects a fake SOI hit with insane SOF dims while accepting a valid neighbor', () => {
    const fake = insaneJpeg()
    const real = minimalJpeg(1920, 1080)
    const bytes = Uint8Array.from([0x00, 0x00, ...fake, 0x00, 0x00, 0x00, ...real])

    const candidates = findValidJpegs(bytes)

    expect(candidates).toHaveLength(1)
    expect(candidates[0]).toMatchObject({ width: 1920, height: 1080 })
    // sanity: the rejected candidate really was the 3900x56032 one, not silently dropped for another reason
    expect(jpegDimensions(Uint8Array.from(fake))).toEqual({ width: 3900, height: 56032 })
  })

  test('returns the offset and length of the accepted candidate', () => {
    const real = minimalJpeg(2048, 1080)
    const prefix = [0xde, 0xad, 0xbe, 0xef]
    const bytes = Uint8Array.from([...prefix, ...real])

    const candidates = findValidJpegs(bytes)

    expect(candidates).toHaveLength(1)
    expect(candidates[0]?.offset).toBe(prefix.length)
    expect(candidates[0]?.length).toBe(real.length)
  })

  test('returns no candidates when nothing sane is present', () => {
    const bytes = Uint8Array.from([...insaneJpeg(), 0x00, 0x00])
    expect(findValidJpegs(bytes)).toEqual([])
  })

  test('a non-self-terminating decoy SOI before a real JPEG does not hide it', () => {
    const real = minimalJpeg(1920, 1080)
    const bytes = new Uint8Array([0xff, 0xd8, 0xff, 1, 2, 3, 4, 5, 6, 7, 8, ...real])
    const found = findValidJpegs(bytes)
    expect(found).toHaveLength(1)
    expect(found[0]).toMatchObject({ offset: 11, length: real.length, width: 1920, height: 1080 })
  })

  test('candidate spans start at the REAL SOI, not a preceding decoy', () => {
    const real = minimalJpeg(640, 480)
    const bytes = new Uint8Array([0xff, 0xd8, 0xff, 0x01, 0x02, ...real])
    const found = findValidJpegs(bytes)
    expect(found).toHaveLength(1)
    expect(found[0]?.offset).toBe(5)
    // the reported span must itself re-parse as a valid JPEG:
    const span = bytes.subarray(found[0].offset, found[0].offset + found[0].length)
    expect(jpegDimensions(span)).toEqual({ width: 640, height: 480 })
  })
})
