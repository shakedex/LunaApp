import { describe, expect, test } from 'bun:test'
import { detectReels, UNGROUPED_REEL } from '../src/reels/detect'

const clip = (relativePath: string, reelName?: string) => ({ relativePath, reelName })

describe('detectReels', () => {
  test('groups by embedded reelName first, regardless of folders', () => {
    const reels = detectReels([
      clip('X/one.mov', 'A001R2B'),
      clip('Y/two.mov', 'A001R2B'),
      clip('X/three.mov', 'B001R1A'),
    ])
    expect(reels.map((r) => r.name)).toEqual(['A001R2B', 'B001R1A'])
    expect(reels[0]?.clips.map((c) => c.relativePath)).toEqual(['X/one.mov', 'Y/two.mov'])
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
