import { fileExtensionOf, isSupportedMediaExtension } from '../media/extensions'
import type { DirectoryHandleLike, FileHandleLike } from './handles'
import { isJunkName } from './junk'
import type { ClipRef, OtherFileRef, UnreadableFileRef } from './model'

export interface ScanProgress {
  filesSeen: number
  clipsFound: number
  currentDir: string
}

export interface ScanResult {
  clips: ClipRef[]
  // Every non-junk, non-media file — nothing on the card is silently dropped.
  otherFiles: OtherFileRef[]
  // Entries the walk could not open. Their path is known; their size is not, so
  // they are excluded from every byte total the report shows.
  unreadable: UnreadableFileRef[]
}

const RTN_EXTENSION = '.rtn'

function unreadableRef(name: string, relativePath: string, err: unknown): UnreadableFileRef {
  return {
    fileName: name,
    relativePath,
    extension: fileExtensionOf(name),
    reason: err instanceof Error ? err.message : String(err),
  }
}

export async function scanFolder(
  root: DirectoryHandleLike,
  onProgress?: (p: ScanProgress) => void,
): Promise<ScanResult> {
  const clips: ClipRef[] = []
  const otherFiles: OtherFileRef[] = []
  const unreadable: UnreadableFileRef[] = []
  // `.rtn` sidecars are also tracked separately so they can be associated with
  // their `.r3d` clip once the whole tree is known — but their bytes still
  // count as other files (they were delivered on the card).
  const rtnFiles: { relativePath: string; file: FileHandleLike }[] = []
  let filesSeen = 0

  async function walk(dir: DirectoryHandleLike, prefix: string): Promise<void> {
    for await (const [, entry] of dir.entries()) {
      if (isJunkName(entry.name)) continue
      const relativePath = `${prefix}${entry.name}`
      if (entry.kind === 'directory') {
        // A directory that dies mid-iteration (card unplugged, unreadable node)
        // must not sink the whole scan — record it and keep walking its
        // siblings. The ROOT call below is deliberately NOT wrapped: a dead root
        // means "the folder is gone", which run-scan reports with its own
        // message and its stale-recent-source handling.
        try {
          await walk(entry, `${relativePath}/`)
        } catch (err) {
          unreadable.push(unreadableRef(entry.name, relativePath, err))
        }
        continue
      }
      // Counted before the read is attempted: a file we could not open was
      // still seen, and progress must not stall on it.
      filesSeen += 1
      try {
        if (isSupportedMediaExtension(entry.name)) {
          clips.push(await toRef(entry, relativePath))
        } else {
          const { size } = await entry.getFile()
          otherFiles.push({
            fileName: entry.name,
            relativePath,
            extension: fileExtensionOf(entry.name),
            sizeBytes: size,
          })
          if (fileExtensionOf(entry.name) === RTN_EXTENSION) {
            rtnFiles.push({ relativePath, file: entry })
          }
        }
      } catch (err) {
        unreadable.push(unreadableRef(entry.name, relativePath, err))
      }
      onProgress?.({ filesSeen, clipsFound: clips.length, currentDir: prefix })
    }
  }

  await walk(root, '')
  associateRtnSidecars(clips, rtnFiles)
  return { clips, otherFiles, unreadable }
}

/** A clip's directory prefix, e.g. `'A001/A001C001.r3d'` -> `'A001/'`. */
function dirOf(relativePath: string): string {
  const slash = relativePath.lastIndexOf('/')
  return slash === -1 ? '' : relativePath.slice(0, slash + 1)
}

/** Lowercased basename without extension, e.g. `'A001/A001C001.R3D'` -> `'a001c001'`. */
function basenameNoExtLower(relativePath: string): string {
  const name = relativePath.slice(relativePath.lastIndexOf('/') + 1)
  const dot = name.lastIndexOf('.')
  const base = dot <= 0 ? name : name.slice(0, dot)
  return base.toLowerCase()
}

function associateRtnSidecars(
  clips: ClipRef[],
  rtnFiles: readonly { relativePath: string; file: FileHandleLike }[],
): void {
  for (const clip of clips) {
    if (clip.extension !== '.r3d') continue
    const clipDir = dirOf(clip.relativePath)
    const clipBase = basenameNoExtLower(clip.relativePath)
    const match = rtnFiles.find(
      (r) => dirOf(r.relativePath) === clipDir && basenameNoExtLower(r.relativePath) === clipBase,
    )
    if (match) clip.previewSidecar = match.file
  }
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
