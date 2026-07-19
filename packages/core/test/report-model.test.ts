import { describe, expect, test } from 'bun:test'
import { buildReportModel, cardCountFrom } from '../src/report/model'
import type { BlobLike, FileHandleLike } from '../src/scan/handles'
import type { ClipRef, OtherFileRef } from '../src/scan/model'

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
const other = (relativePath: string, sizeBytes: number): OtherFileRef => ({
  fileName: relativePath.split('/').pop() ?? relativePath,
  relativePath,
  extension: `.${relativePath.split('.').pop() ?? ''}`,
  sizeBytes,
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
  test('merges metadata + thumbnails, groups reels, sums stats over ALL files', () => {
    const clips = [ref('A001/one.mov', 100), ref('A001/two.mov', 50), ref('B002/three.mxf', 25)]
    const otherFiles = [other('A001/sound.wav', 200), other('A001/grade.cube', 4)]
    const model = buildReportModel({
      clips,
      otherFiles,
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
      otherFileCount: 2,
      otherFileSizeBytes: 204,
      totalDurationSeconds: 15, // missing duration contributes 0, never fabricated
      // Byte-for-byte honesty: clips (175) + other files (204).
      totalSizeBytes: 379,
    })
    expect(model.reels.map((r) => r.name)).toEqual(['A001', 'B002', 'CUSTOM'])
    const custom = model.reels.find((r) => r.name === 'CUSTOM')
    expect(custom?.clips[0]?.thumbnails.length).toBe(1)
    const three = model.reels.find((r) => r.name === 'B002')?.clips[0]
    expect(three?.metadata).toEqual({}) // failed metadata → empty, present
    expect(three?.thumbnails).toEqual([])
    expect(model.cover.projectTitle).toBe('Test')
    // Other files are LISTED inside their top-folder reel — name, path, size.
    // The report is a 1:1 inventory of the card; a bare count is not enough.
    const a001 = model.reels.find((r) => r.name === 'A001')
    expect(a001?.otherFiles.map((f) => f.relativePath)).toEqual([
      'A001/grade.cube',
      'A001/sound.wav',
    ])
    expect(a001?.otherFiles[1]).toEqual({
      fileName: 'sound.wav',
      relativePath: 'A001/sound.wav',
      extension: '.wav',
      sizeBytes: 200,
    })
    expect(model.reels.find((r) => r.name === 'CUSTOM')?.otherFiles).toEqual([])
    expect(a001?.stats).toEqual({
      clipCount: 1,
      otherFileCount: 2,
      otherFileSizeBytes: 204,
      totalSizeBytes: 254,
      totalDurationSeconds: 5,
    })
    const customStats = model.reels.find((r) => r.name === 'CUSTOM')?.stats
    expect(customStats).toEqual({
      clipCount: 1,
      otherFileCount: 0,
      otherFileSizeBytes: 0,
      totalSizeBytes: 100,
      totalDurationSeconds: 10,
    })
  })

  test('other files in a folder with no clips still form a reel — bytes are never dropped', () => {
    const model = buildReportModel({
      clips: [ref('A001/one.mov', 100)],
      otherFiles: [other('SOUND/day1.wav', 500)],
      metadataById: {},
      thumbsById: {},
      cover: {},
    })
    expect(model.reels.map((r) => r.name)).toEqual(['A001', 'SOUND'])
    const sound = model.reels.find((r) => r.name === 'SOUND')
    expect(sound?.clips).toEqual([])
    expect(sound?.otherFiles.map((f) => f.fileName)).toEqual(['day1.wav'])
    expect(sound?.stats).toEqual({
      clipCount: 0,
      otherFileCount: 1,
      otherFileSizeBytes: 500,
      totalSizeBytes: 500,
      totalDurationSeconds: 0,
    })
    // The clipless folder is still a card, and its bytes are in the total.
    expect(model.stats.cardCount).toBe(2)
    expect(model.stats.totalSizeBytes).toBe(600)
  })

  test('root-level other files group as Ungrouped', () => {
    const model = buildReportModel({
      clips: [],
      otherFiles: [other('manifest.xml', 12)],
      metadataById: {},
      thumbsById: {},
      cover: {},
    })
    expect(model.reels.map((r) => r.name)).toEqual(['Ungrouped'])
    expect(model.reels[0]?.otherFiles.map((f) => f.fileName)).toEqual(['manifest.xml'])
    expect(model.stats.totalSizeBytes).toBe(12)
    expect(model.stats.cardCount).toBe(1)
  })

  test('empty input produces an empty, zeroed model', () => {
    const model = buildReportModel({
      clips: [],
      otherFiles: [],
      metadataById: {},
      thumbsById: {},
      cover: {},
    })
    expect(model.stats).toEqual({
      cardCount: 0,
      clipCount: 0,
      otherFileCount: 0,
      otherFileSizeBytes: 0,
      totalDurationSeconds: 0,
      totalSizeBytes: 0,
    })
    expect(model.reels).toEqual([])
  })

  test('sourceRoot flows through; absent input defaults to empty string', () => {
    const base = { clips: [], otherFiles: [], metadataById: {}, thumbsById: {}, cover: {} }
    expect(buildReportModel({ ...base, sourceRoot: 'CARD_A' }).sourceRoot).toBe('CARD_A')
    expect(buildReportModel(base).sourceRoot).toBe('')
  })
})
