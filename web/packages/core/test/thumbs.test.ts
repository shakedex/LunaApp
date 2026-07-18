import { describe, expect, test } from 'bun:test'
import { THUMBNAIL_POSITIONS, thumbnailTimestamps } from '../src/thumbs/model'
import { decodePathFor } from '../src/thumbs/router'

describe('thumbnailTimestamps', () => {
  test('maps 10/50/90% of duration', () => {
    expect(THUMBNAIL_POSITIONS).toEqual([0.1, 0.5, 0.9])
    expect(thumbnailTimestamps(100)).toEqual([10, 50, 90])
  })
  test('degenerate durations collapse to a single frame at 0', () => {
    expect(thumbnailTimestamps(0)).toEqual([0])
    expect(thumbnailTimestamps(-5)).toEqual([0])
    expect(thumbnailTimestamps(Number.NaN)).toEqual([0])
  })
  test('timestamps never reach the duration itself', () => {
    for (const t of thumbnailTimestamps(1)) expect(t).toBeLessThan(1)
  })
})

describe('decodePathFor', () => {
  test('fast-path containers go to mediabunny', () => {
    for (const ext of ['.mp4', '.m4v', '.mov', '.mkv', '.webm', '.3gp']) {
      expect(decodePathFor(ext)).toBe('mediabunny')
    }
  })
  test('MXF and legacy go to ffmpeg', () => {
    for (const ext of ['.mxf', '.avi', '.mts', '.m2ts', '.wmv', '.flv']) {
      expect(decodePathFor(ext)).toBe('ffmpeg')
    }
  })
  test('RAW and unknown are never decoded', () => {
    for (const ext of ['.r3d', '.braw', '.ari', '.txt', '']) {
      expect(decodePathFor(ext)).toBe('none')
    }
  })
})
