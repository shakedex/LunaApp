import { describe, expect, test } from 'bun:test'
import type { DirectoryHandleLike, FileHandleLike, FileSystemEntryLike } from '../src/scan/handles'
import { type ScanProgress, scanFolder } from '../src/scan/walker'

function file(name: string, size: number): FileHandleLike {
  return { kind: 'file', name, getFile: async () => ({ size }) }
}
function dir(name: string, ...children: FileSystemEntryLike[]): DirectoryHandleLike {
  return {
    kind: 'directory',
    name,
    async *entries() {
      for (const c of children) yield [c.name, c] as const
    },
  }
}

const card = dir(
  'CARD01',
  dir('A001', file('A001C001.mov', 100), file('A001C002.MP4', 50), file('notes.txt', 1)),
  dir('.hidden', file('secret.mov', 10)),
  dir('System Volume Information', file('x.mov', 10)),
  file('shot.R3D', 999),
  file('.DS_Store', 1),
)

describe('scanFolder', () => {
  test('finds nested clips with relative paths and sizes', async () => {
    const { clips } = await scanFolder(card)
    expect(clips.map((c) => c.relativePath)).toEqual(['A001/A001C001.mov', 'A001/A001C002.MP4'])
    expect(clips[0]).toMatchObject({
      id: 'A001/A001C001.mov',
      fileName: 'A001C001.mov',
      extension: '.mov',
      sizeBytes: 100,
    })
    expect(clips[1]?.extension).toBe('.mp4')
  })

  test('routes known RAW to raw notices, skips junk and non-media', async () => {
    const { clips, raw } = await scanFolder(card)
    expect(raw.map((r) => r.relativePath)).toEqual(['shot.R3D'])
    expect(raw[0]?.extension).toBe('.r3d')
    const all = [...clips, ...raw].map((e) => e.relativePath).join()
    expect(all).not.toContain('secret')
    expect(all).not.toContain('x.mov')
    expect(all).not.toContain('notes.txt')
  })

  test('reports progress and counts only non-junk files', async () => {
    const seen: ScanProgress[] = []
    await scanFolder(card, (p) => seen.push({ ...p }))
    expect(seen.length).toBeGreaterThan(0)
    const last = seen[seen.length - 1]
    expect(last?.filesSeen).toBe(4) // 2 clips + notes.txt + shot.R3D (.DS_Store junk-skipped)
    expect(last?.clipsFound).toBe(2)
  })
})
