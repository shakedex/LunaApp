import { describe, expect, test } from 'bun:test'
import { buildReportModel, cardCountFrom } from '../src/report/model'
import type { BlobLike, FileHandleLike } from '../src/scan/handles'
import type { ClipRef } from '../src/scan/model'

const fakeBlob: BlobLike = {
  size: 0,
  slice: () => fakeBlob,
  arrayBuffer: async () => new ArrayBuffer(0),
}
const fakeFile: FileHandleLike = { kind: 'file', name: 'x', getFile: async () => fakeBlob }
const ref = (relativePath: string, sizeBytes: number): ClipRef => ({
  id: relativePath,
  fileName: relativePath.split('/').pop() ?? relativePath,
  relativePath,
  extension: `.${relativePath.split('.').pop() ?? ''}`,
  sizeBytes,
  file: fakeFile,
})

describe('cardCountFrom', () => {
  test('distinct top-level folders', () => {
    expect(cardCountFrom(['A001/a.mov', 'A001/b.mov', 'B002/c.mov'])).toBe(2)
  })
  test('media at root only counts as one card', () => {
    expect(cardCountFrom(['a.mov', 'b.mov'])).toBe(1)
  })
  test('mixed root + folders counts folders only (spec §8.8)', () => {
    expect(cardCountFrom(['a.mov', 'A001/b.mov'])).toBe(1)
  })
  test('no clips, no cards', () => {
    expect(cardCountFrom([])).toBe(0)
  })
})

describe('buildReportModel', () => {
  test('merges metadata + thumbnails, groups reels, sums stats', () => {
    const clips = [ref('A001/one.mov', 100), ref('A001/two.mov', 50), ref('B002/three.mxf', 25)]
    const model = buildReportModel({
      clips,
      raw: [],
      metadataById: {
        'A001/one.mov': { durationSeconds: 10, reelName: 'CUSTOM' },
        'A001/two.mov': { durationSeconds: 5 },
      },
      thumbsById: {
        'A001/one.mov': [{ positionRatio: 0.1, timestampSeconds: 1, outcome: 'Success' }],
      },
      cover: { projectTitle: 'Test' },
    })
    expect(model.stats).toEqual({
      cardCount: 2,
      clipCount: 3,
      rawCount: 0,
      totalDurationSeconds: 15, // missing duration contributes 0, never fabricated
      totalSizeBytes: 175,
    })
    expect(model.reels.map((r) => r.name)).toEqual(['A001', 'B002', 'CUSTOM'])
    const custom = model.reels.find((r) => r.name === 'CUSTOM')
    expect(custom?.clips[0]?.thumbnails.length).toBe(1)
    const three = model.reels.find((r) => r.name === 'B002')?.clips[0]
    expect(three?.metadata).toEqual({}) // failed metadata → empty, present
    expect(three?.thumbnails).toEqual([])
    expect(model.cover.projectTitle).toBe('Test')
    const a001 = model.reels.find((r) => r.name === 'A001')
    expect(a001?.stats).toEqual({ clipCount: 1, totalSizeBytes: 50, totalDurationSeconds: 5 })
    const customStats = model.reels.find((r) => r.name === 'CUSTOM')?.stats
    expect(customStats).toEqual({ clipCount: 1, totalSizeBytes: 100, totalDurationSeconds: 10 })
  })

  test('empty input produces an empty, zeroed model', () => {
    const model = buildReportModel({
      clips: [],
      raw: [],
      metadataById: {},
      thumbsById: {},
      cover: {},
    })
    expect(model.stats).toEqual({
      cardCount: 0,
      clipCount: 0,
      rawCount: 0,
      totalDurationSeconds: 0,
      totalSizeBytes: 0,
    })
    expect(model.reels).toEqual([])
  })
})
