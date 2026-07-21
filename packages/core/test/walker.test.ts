import { describe, expect, test } from 'vite-plus/test'
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

// "A file is a file" (2026-07-19 backlog): every camera-original — including
// .ari — is a clip, and every other non-junk file is surfaced as an
// OtherFileRef so no byte on the card goes unaccounted.
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
  test('finds nested clips with relative paths and sizes, including RAW formats', async () => {
    const { clips } = await scanFolder(card)
    expect(clips.map((c) => c.relativePath)).toEqual([
      'A001/A001C001.mov',
      'A001/A001C002.MP4',
      'shot.R3D',
      'still.ari',
    ])
    expect(clips[0]).toMatchObject({
      id: 'A001/A001C001.mov',
      fileName: 'A001C001.mov',
      extension: '.mov',
      sizeBytes: 100,
    })
    expect(clips[1]?.extension).toBe('.mp4')
    expect(clips[2]?.extension).toBe('.r3d')
    expect(clips[3]?.extension).toBe('.ari') // ARRIRAW stills are clips too
  })

  test('surfaces non-media files as other files, skips junk', async () => {
    const { clips, otherFiles } = await scanFolder(card)
    expect(otherFiles.map((f) => f.relativePath)).toEqual(['A001/notes.txt'])
    expect(otherFiles[0]).toMatchObject({
      fileName: 'notes.txt',
      relativePath: 'A001/notes.txt',
      extension: '.txt',
      sizeBytes: 1,
    })
    const all = [...clips, ...otherFiles].map((e) => e.relativePath).join()
    expect(all).not.toContain('secret')
    expect(all).not.toContain('x.mov')
    expect(all).not.toContain('.DS_Store')
  })

  test('reports progress and counts only non-junk files', async () => {
    const seen: ScanProgress[] = []
    await scanFolder(card, (p) => seen.push({ ...p }))
    expect(seen.length).toBeGreaterThan(0)
    const last = seen[seen.length - 1]
    // 4 clips (mov + mp4 + shot.R3D + still.ari) + notes.txt other file
    // (.DS_Store junk-skipped).
    expect(last?.filesSeen).toBe(5)
    expect(last?.clipsFound).toBe(4)
  })
})

describe('scanFolder .rtn sidecar association', () => {
  test('associates a same-directory .rtn with its .r3d clip by basename', async () => {
    const root = dir('ROOT', dir('A001', file('A001C001.R3D', 100), file('A001C001.rtn', 10)))
    const { clips } = await scanFolder(root)
    const clip = clips.find((c) => c.relativePath === 'A001/A001C001.R3D')
    expect(clip?.previewSidecar).toBeDefined()
    expect(clip?.previewSidecar?.name).toBe('A001C001.rtn')
  })

  test('matches basenames case-insensitively', async () => {
    const root = dir('ROOT', dir('A001', file('a001c001.r3d', 100), file('A001C001.RTN', 10)))
    const { clips } = await scanFolder(root)
    const clip = clips.find((c) => c.relativePath === 'A001/a001c001.r3d')
    expect(clip?.previewSidecar?.name).toBe('A001C001.RTN')
  })

  test('does not associate an .rtn from a different directory', async () => {
    const root = dir(
      'ROOT',
      dir('A001', file('A001C001.R3D', 100)),
      dir('A002', file('A001C001.rtn', 10)),
    )
    const { clips } = await scanFolder(root)
    const clip = clips.find((c) => c.relativePath === 'A001/A001C001.R3D')
    expect(clip?.previewSidecar).toBeUndefined()
  })

  test('does not associate an .rtn with a different basename in the same directory', async () => {
    const root = dir('ROOT', dir('A001', file('A001C001.R3D', 100), file('A001C002.rtn', 10)))
    const { clips } = await scanFolder(root)
    const clip = clips.find((c) => c.relativePath === 'A001/A001C001.R3D')
    expect(clip?.previewSidecar).toBeUndefined()
  })

  test('.rtn is never a clip, but IS counted as an other file (bytes on card)', async () => {
    const root = dir('ROOT', dir('A001', file('A001C001.R3D', 100), file('A001C001.rtn', 10)))
    const seen: ScanProgress[] = []
    const { clips, otherFiles } = await scanFolder(root, (p) => seen.push({ ...p }))
    expect(clips.map((c) => c.relativePath)).toEqual(['A001/A001C001.R3D'])
    // The sidecar's bytes were delivered on the card — they must be accounted
    // for, even though its JPEG doubles as the clip's thumbnail source.
    expect(otherFiles.map((f) => f.relativePath)).toEqual(['A001/A001C001.rtn'])
    expect(otherFiles[0]?.sizeBytes).toBe(10)
    const last = seen[seen.length - 1]
    expect(last?.filesSeen).toBe(2)
    expect(last?.clipsFound).toBe(1)
  })
})
