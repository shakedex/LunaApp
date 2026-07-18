import {
  fileExtensionOf,
  isKnownRawExtension,
  isSupportedMediaExtension,
} from '../media/extensions'
import type { DirectoryHandleLike, FileHandleLike } from './handles'
import { isJunkName } from './junk'
import type { ClipRef, RawNotice } from './model'

export interface ScanProgress {
  filesSeen: number
  clipsFound: number
  currentDir: string
}

export interface ScanResult {
  clips: ClipRef[]
  raw: RawNotice[]
}

export async function scanFolder(
  root: DirectoryHandleLike,
  onProgress?: (p: ScanProgress) => void,
): Promise<ScanResult> {
  const clips: ClipRef[] = []
  const raw: RawNotice[] = []
  let filesSeen = 0

  async function walk(dir: DirectoryHandleLike, prefix: string): Promise<void> {
    for await (const [, entry] of dir.entries()) {
      if (entry.kind === 'directory') {
        if (!isJunkName(entry.name)) await walk(entry, `${prefix}${entry.name}/`)
        continue
      }
      if (isJunkName(entry.name)) continue
      filesSeen += 1
      const relativePath = `${prefix}${entry.name}`
      if (isSupportedMediaExtension(entry.name)) {
        clips.push(await toRef(entry, relativePath))
      } else if (isKnownRawExtension(entry.name)) {
        raw.push(await toRef(entry, relativePath))
      }
      onProgress?.({ filesSeen, clipsFound: clips.length, currentDir: prefix })
    }
  }

  await walk(root, '')
  return { clips, raw }
}

async function toRef(entry: FileHandleLike, relativePath: string): Promise<ClipRef> {
  const { size } = await entry.getFile()
  return {
    id: relativePath,
    fileName: entry.name,
    relativePath,
    extension: fileExtensionOf(entry.name),
    sizeBytes: size,
    file: entry,
  }
}
