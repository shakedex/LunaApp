import { describe, expect, test } from 'bun:test'
import type {
  BlobLike,
  DirectoryHandleLike,
  FileHandleLike,
  FileSystemEntryLike,
} from '../src/scan/handles'
import { type ScanProgress, scanFolder } from '../src/scan/walker'

function fakeBlob(size: number): BlobLike {
  return { size, slice: () => fakeBlob(0), arrayBuffer: async () => new ArrayBuffer(0) }
}
function file(name: string, size: number): FileHandleLike {
  return { kind: 'file', name, getFile: async () => fakeBlob(size) }
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

// shot.R3D is deliberately reclassified: .r3d moved from UNSUPPORTED_RAW_EXTENSIONS
// into SUPPORTED_MEDIA_EXTENSIONS (FINDINGS.md, Plan 08), so the walker now
// emits it as a clip. still.ari keeps a real raw-notice case covered — .ari
// is the only extension left in UNSUPPORTED_RAW_EXTENSIONS.
const card = dir(
  'CARD01',
  dir('A001', file('A001C001.mov', 100), file('A001C002.MP4', 50), file('notes.txt', 1)),
  dir('.hidden', file('secret.mov', 10)),
  dir('System Volume Information', file('x.mov', 10)),
  file('shot.R3D', 999),
  file('still.ari', 5),
  file('.DS_Store', 1),
)

describe('scanFolder', () => {
  test('finds nested clips with relative paths and sizes, including reclassified RAW', async () => {
    const { clips } = await scanFolder(card)
    expect(clips.map((c) => c.relativePath)).toEqual([
      'A001/A001C001.mov',
      'A001/A001C002.MP4',
      'shot.R3D',
    ])
    expect(clips[0]).toMatchObject({
      id: 'A001/A001C001.mov',
      fileName: 'A001C001.mov',
      extension: '.mov',
      sizeBytes: 100,
    })
    expect(clips[1]?.extension).toBe('.mp4')
    expect(clips[2]?.extension).toBe('.r3d') // reclassified RAW now flows as a clip
  })

  test('routes .ari to raw notices, skips junk and non-media', async () => {
    const { clips, raw } = await scanFolder(card)
    expect(raw.map((r) => r.relativePath)).toEqual(['still.ari'])
    expect(raw[0]?.extension).toBe('.ari')
    // .r3d is a clip now, not a raw notice.
    expect(clips.map((c) => c.relativePath)).toContain('shot.R3D')
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
    // 3 clips (2 mov/mp4 + reclassified shot.R3D) + notes.txt + still.ari raw notice
    // (.DS_Store junk-skipped).
    expect(last?.filesSeen).toBe(5)
    expect(last?.clipsFound).toBe(3)
  })
})
