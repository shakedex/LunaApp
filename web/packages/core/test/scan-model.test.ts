import { describe, expect, test } from 'bun:test'
import type { FileHandleLike } from '../src/scan/handles'
import { isJunkName } from '../src/scan/junk'
import { buildScanSummary, type ClipRef, type RawNotice } from '../src/scan/model'

const fakeFile: FileHandleLike = { kind: 'file', name: 'x', getFile: async () => ({ size: 0 }) }
const clip = (relativePath: string, extension: string, sizeBytes: number): ClipRef => ({
  id: relativePath,
  fileName: relativePath.split('/').pop() ?? relativePath,
  relativePath,
  extension,
  sizeBytes,
  file: fakeFile,
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
  test('counts, sums clip bytes, groups by extension', () => {
    const clips = [
      clip('a/1.mov', '.mov', 100),
      clip('a/2.mov', '.mov', 50),
      clip('b/3.mxf', '.mxf', 25),
    ]
    const raw: RawNotice[] = [clip('c/4.r3d', '.r3d', 999)]
    const s = buildScanSummary(clips, raw)
    expect(s.clipCount).toBe(3)
    expect(s.rawCount).toBe(1)
    expect(s.totalClipSizeBytes).toBe(175)
    expect(s.byExtension).toEqual({ '.mov': 2, '.mxf': 1 })
  })
  test('empty scan', () => {
    expect(buildScanSummary([], [])).toEqual({
      clipCount: 0,
      rawCount: 0,
      totalClipSizeBytes: 0,
      byExtension: {},
    })
  })
})
