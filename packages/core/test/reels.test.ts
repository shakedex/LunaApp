import { describe, expect, test } from 'vite-plus/test'
import { detectReels, UNGROUPED_REEL, wrapperPrefixDepth } from '../src/reels/detect'

const clip = (relativePath: string, reelName?: string) => ({ relativePath, reelName })

describe('detectReels', () => {
  test('groups by embedded reelName when no folder looks like a reel', () => {
    const reels = detectReels([
      clip('X/one.mov', 'A001R2B'),
      clip('Y/two.mov', 'A001R2B'),
      clip('X/three.mov', 'B001R1A'),
    ])
    expect(reels.map((r) => r.name)).toEqual(['A001R2B', 'B001R1A'])
    expect(reels[0]?.clips.map((c) => c.relativePath)).toEqual(['X/one.mov', 'Y/two.mov'])
  })

  test('a reel-like folder beats the embedded reelName', () => {
    const reels = detectReels([
      clip('A001/PRIVATE/M4ROOT/CLIP/c1.mxf', '0001AB'),
      clip('A001/PRIVATE/M4ROOT/CLIP/c2.mxf', '0001AB'),
      clip('B002/PRIVATE/M4ROOT/CLIP/c3.mxf', '0001AC'),
    ])
    expect(reels.map((r) => r.name)).toEqual(['A001', 'B002'])
  })

  test('the deepest reel-like folder wins, at any depth', () => {
    const reels = detectReels([
      clip('CAMERA/A_CAM/A001/x.mov'),
      clip('CAMERA/A_CAM/A002/y.mov'),
      clip('CAMERA/S_cam/s004/z.mxf'),
      clip('CAMERA/RONIN-4D/w.mov'),
    ])
    expect(reels.map((r) => r.name)).toEqual(['A001', 'A002', 'CAMERA', 's004'])
  })

  test('prefixDepth strips wrapper folders before the first-folder fallback', () => {
    const reels = detectReels(
      [clip('CAMERA/BURANO/x.mxf'), clip('CAMERA/T_CAM/y.mxf'), clip('CAMERA/T_CAM/z.mxf')],
      { prefixDepth: 1 },
    )
    expect(reels.map((r) => r.name)).toEqual(['BURANO', 'T_CAM'])
  })

  test('falls back to the top-level folder when reelName is absent', () => {
    const reels = detectReels([clip('A001/c2.mov'), clip('A001/c1.mov'), clip('B002/c3.mov')])
    expect(reels.map((r) => r.name)).toEqual(['A001', 'B002'])
    expect(reels[0]?.clips.map((c) => c.relativePath)).toEqual(['A001/c1.mov', 'A001/c2.mov'])
  })

  test('root-level clips without reelName land in Ungrouped', () => {
    const reels = detectReels([clip('loose.mov'), clip('A001/x.mov')])
    expect(reels.map((r) => r.name)).toEqual(['A001', UNGROUPED_REEL])
  })

  test('whitespace-only reelName is treated as absent', () => {
    const reels = detectReels([clip('A001/x.mov', '   ')])
    expect(reels.map((r) => r.name)).toEqual(['A001'])
  })

  test('numeric-aware reel ordering', () => {
    const reels = detectReels([
      clip('REEL_10/a.mov'),
      clip('REEL_2/b.mov'),
      clip('A002/c.mov'),
      clip('A001/d.mov'),
    ])
    expect(reels.map((r) => r.name)).toEqual(['A001', 'A002', 'REEL_2', 'REEL_10'])
  })

  test('empty input yields no reels', () => {
    expect(detectReels([])).toEqual([])
  })
})

describe('wrapperPrefixDepth', () => {
  test('a single non-reel top folder shared by every path is a wrapper', () => {
    expect(wrapperPrefixDepth(['CAMERA/A_CAM/A001/x.mov', 'CAMERA/BURANO/y.mxf'])).toBe(1)
  })

  test('nested wrappers strip level by level', () => {
    expect(
      wrapperPrefixDepth(['PROJ/CAMERA/A_CAM/A001/x.mov', 'PROJ/CAMERA/B_CAM/B005/y.mxf']),
    ).toBe(2)
  })

  test('multiple top-level folders mean no wrapper', () => {
    expect(wrapperPrefixDepth(['A001/x.mov', 'B002/y.mov'])).toBe(0)
  })

  test('a reel-like top folder is never treated as a wrapper', () => {
    expect(wrapperPrefixDepth(['A001/x.mov', 'A001/y.mov'])).toBe(0)
  })

  test('never strips a level that would leave a file with no folder', () => {
    expect(wrapperPrefixDepth(['CAMERA/x.mov', 'CAMERA/T_CAM/y.mov'])).toBe(0)
  })

  test('root-level files mean no wrapper', () => {
    expect(wrapperPrefixDepth(['x.mov', 'CAMERA/y.mov'])).toBe(0)
  })

  test('empty input has no wrapper', () => {
    expect(wrapperPrefixDepth([])).toBe(0)
  })
})
