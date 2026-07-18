import type { FileHandleLike } from './handles'

export interface ClipRef {
  id: string // relativePath — unique within one scan
  fileName: string
  relativePath: string
  extension: string
  sizeBytes: number
  file: FileHandleLike
}

export interface RawNotice {
  id: string
  fileName: string
  relativePath: string
  extension: string
  sizeBytes: number
  file: FileHandleLike
}

export interface ScanSummary {
  clipCount: number
  rawCount: number
  totalClipSizeBytes: number
  byExtension: Record<string, number>
}

export function buildScanSummary(
  clips: readonly ClipRef[],
  raw: readonly RawNotice[],
): ScanSummary {
  const byExtension: Record<string, number> = {}
  let totalClipSizeBytes = 0
  for (const c of clips) {
    totalClipSizeBytes += c.sizeBytes
    byExtension[c.extension] = (byExtension[c.extension] ?? 0) + 1
  }
  return { clipCount: clips.length, rawCount: raw.length, totalClipSizeBytes, byExtension }
}
