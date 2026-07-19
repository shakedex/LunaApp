import { describe, expect, test } from 'bun:test'
import type { BlobLike, FileHandleLike } from '../src/scan/handles'
import { isJunkName } from '../src/scan/junk'
import { buildScanSummary, type ClipRef, type OtherFileRef } from '../src/scan/model'

const fakeBlob: BlobLike = {
  size: 0,
  slice: () => fakeBlob,
  arrayBuffer: async () => new ArrayBuffer(0),
}
const fakeFile: FileHandleLike = { kind: 'file', name: 'x', getFile: async () => fakeBlob }
const clip = (relativePath: string, extension: string, sizeBytes: number): ClipRef => ({
  id: relativePath,
  fileName: relativePath.split('/').pop() ?? relativePath,
  relativePath,
  extension,
  sizeBytes,
  file: fakeFile,
})
const other = (relativePath: string, extension: string, sizeBytes: number): OtherFileRef => ({
  fileName: relativePath.split('/').pop() ?? relativePath,
  relativePath,
  extension,
  sizeBytes,
})

describe('isJunkName', () => {
  test('dot-prefixed and known junk dirs are junk', () => {
    expect(isJunkName('.hidden')).toBe(true)
    expect(isJunkName('.DS_Store')).toBe(true)
    expect(isJunkName('System Volume Information')).toBe(true)
    expect(isJunkName('$RECYCLE.BIN')).toBe(true)
    expect(isJunkName('__MACOSX')).toBe(true)
  })
  test('normal names are not junk', () => {
    expect(isJunkName('A001')).toBe(false)
    expect(isJunkName('CARD01')).toBe(false)
  })
})

describe('buildScanSummary', () => {
  test('totals cover EVERY surfaced file — clips and other files alike', () => {
    const clips = [
      clip('a/1.mov', '.mov', 100),
      clip('a/2.mov', '.mov', 50),
      clip('b/3.ari', '.ari', 25),
    ]
    const otherFiles = [other('a/sound.wav', '.wav', 200), other('b/lut.cube', '.cube', 4)]
    const s = buildScanSummary(clips, otherFiles)
    expect(s.clipCount).toBe(3)
    expect(s.otherFileCount).toBe(2)
    expect(s.otherFileSizeBytes).toBe(204)
    // The report's whole point is byte-for-byte comparability: the total is
    // the sum of ALL file sizes, never a subset.
    expect(s.totalSizeBytes).toBe(379)
    expect(s.byExtension).toEqual({ '.mov': 2, '.ari': 1 })
  })
  test('empty scan', () => {
    expect(buildScanSummary([], [])).toEqual({
      clipCount: 0,
      otherFileCount: 0,
      otherFileSizeBytes: 0,
      totalSizeBytes: 0,
      byExtension: {},
    })
  })
})
