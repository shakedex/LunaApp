import type { FileHandleLike } from './handles'

export interface ClipRef {
  id: string // relativePath — unique within one scan
  fileName: string
  relativePath: string
  extension: string
  sizeBytes: number
  file: FileHandleLike
  // Present only for `.r3d` clips that have a same-directory `.rtn` sidecar
  // (REDCINE-X's embedded-JPEG thumbnail source — FINDINGS.md), associated by
  // the walker after the scan completes.
  previewSidecar?: FileHandleLike
}

// A non-media file on the card (WAV, LUT, sidecar, anything). Surfaced so the
// report accounts for every delivered byte; rolled into per-reel/report counts
// and sizes rather than listed per-file (2026-07-19 backlog, option b).
export interface OtherFileRef {
  fileName: string
  relativePath: string
  extension: string
  sizeBytes: number
}

// An entry the walk could not open — a file, or a directory whose iteration died.
// Its path is known; its size is not, so it is excluded from every byte total. A
// directory is the worse case: an unknown number of clips sit behind it, which is
// why callers must count and word the two kinds apart.
export interface UnreadableEntryRef {
  kind: 'file' | 'directory'
  fileName: string
  relativePath: string
  extension: string
  /** Why the entry could not be opened, verbatim from the thrown error. */
  reason: string
}

export interface ScanSummary {
  clipCount: number
  otherFileCount: number
  otherFileSizeBytes: number
  // Sum of EVERY surfaced file — clips and other files. The report's whole
  // point is byte-for-byte comparability with the source card.
  totalSizeBytes: number
  // Counted apart because they cost different amounts: a file is one missing entry,
  // a directory hides an unknown number of them.
  unreadableFileCount: number
  unreadableDirectoryCount: number
  byExtension: Record<string, number>
}

export function buildScanSummary(
  clips: readonly ClipRef[],
  otherFiles: readonly OtherFileRef[],
  unreadable: readonly UnreadableEntryRef[] = [],
): ScanSummary {
  const byExtension: Record<string, number> = {}
  let totalSizeBytes = 0
  for (const c of clips) {
    totalSizeBytes += c.sizeBytes
    byExtension[c.extension] = (byExtension[c.extension] ?? 0) + 1
  }
  let otherFileSizeBytes = 0
  for (const f of otherFiles) otherFileSizeBytes += f.sizeBytes
  return {
    clipCount: clips.length,
    otherFileCount: otherFiles.length,
    otherFileSizeBytes,
    totalSizeBytes: totalSizeBytes + otherFileSizeBytes,
    unreadableFileCount: unreadable.filter((u) => u.kind === 'file').length,
    unreadableDirectoryCount: unreadable.filter((u) => u.kind === 'directory').length,
    byExtension,
  }
}
