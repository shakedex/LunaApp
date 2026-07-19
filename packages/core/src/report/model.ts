import type { ClipMetadata } from '../metadata/model'
import { detectReels } from '../reels/detect'
import type { ClipRef, OtherFileRef } from '../scan/model'
import type { ThumbnailFrame } from '../thumbs/model'

export interface CoverFields<TImage = unknown> {
  projectTitle?: string
  productionCompany?: string
  dit?: string
  director?: string
  dp?: string
  date?: string
  logo?: TImage
}

export interface ReportClip<TImage = unknown> {
  id: string
  fileName: string
  relativePath: string
  extension: string
  sizeBytes: number
  metadata: ClipMetadata
  thumbnails: ThumbnailFrame<TImage>[]
}

export interface ReelStats {
  clipCount: number
  otherFileCount: number
  otherFileSizeBytes: number
  // ALL bytes in this reel — clips plus other files.
  totalSizeBytes: number
  totalDurationSeconds: number
}

export interface Reel<TImage = unknown> {
  name: string
  clips: ReportClip<TImage>[]
  // Listed per-file (name, path, size), not just counted: the report is a 1:1
  // inventory of the card, so every file appears where it was discovered.
  otherFiles: OtherFileRef[]
  stats: ReelStats
}

export interface ReportStats {
  cardCount: number
  clipCount: number
  otherFileCount: number
  otherFileSizeBytes: number
  totalDurationSeconds: number
  // Sum of EVERY surfaced file on the card — the number a DIT compares
  // byte-for-byte against the delivered backup. Never a subset.
  totalSizeBytes: number
}

export interface ReportModel<TImage = unknown> {
  cover: CoverFields<TImage>
  sourceRoot: string
  stats: ReportStats
  reels: Reel<TImage>[]
}

// spec §8.8: a "card" is a top-level media subfolder; media at the root with
// no subfolders counts as one card; an empty scan has zero.
export function cardCountFrom(relativePaths: readonly string[]): number {
  if (relativePaths.length === 0) return 0
  const folders = new Set<string>()
  for (const path of relativePaths) {
    const slash = path.indexOf('/')
    if (slash > 0) folders.add(path.slice(0, slash))
  }
  return folders.size > 0 ? folders.size : 1
}

export interface BuildReportInput<TImage = unknown> {
  clips: readonly ClipRef[]
  otherFiles: readonly OtherFileRef[]
  metadataById: Readonly<Record<string, ClipMetadata>>
  thumbsById: Readonly<Record<string, ThumbnailFrame<TImage>[]>>
  cover: CoverFields<TImage>
  sourceRoot?: string
}

export function buildReportModel<TImage = unknown>(
  input: BuildReportInput<TImage>,
): ReportModel<TImage> {
  const reportClips = input.clips.map((clip) => {
    const metadata = input.metadataById[clip.id] ?? {}
    return {
      id: clip.id,
      fileName: clip.fileName,
      relativePath: clip.relativePath,
      extension: clip.extension,
      sizeBytes: clip.sizeBytes,
      metadata,
      thumbnails: input.thumbsById[clip.id] ?? [],
      reelName: metadata.reelName,
    }
  })

  let totalDurationSeconds = 0
  let clipSizeBytes = 0
  for (const clip of reportClips) {
    totalDurationSeconds += clip.metadata.durationSeconds ?? 0
    clipSizeBytes += clip.sizeBytes
  }
  let otherFileSizeBytes = 0
  for (const f of input.otherFiles) otherFileSizeBytes += f.sizeBytes

  // Other files group by top folder (they carry no reelName metadata) and are
  // listed inside their reel; folders holding only other files still become
  // (clipless) reels so their bytes are never dropped. detectReels already
  // sorts each group by relativePath.
  const otherByReel = new Map<string, OtherFileRef[]>()
  for (const group of detectReels(input.otherFiles)) {
    otherByReel.set(group.name, group.clips)
  }
  const sumBytes = (files: readonly OtherFileRef[]) =>
    files.reduce((sum, f) => sum + f.sizeBytes, 0)

  const reels: Reel<TImage>[] = detectReels(reportClips).map((reel) => {
    const clips = reel.clips.map(({ reelName: _drop, ...clip }) => clip)
    let sizeBytes = 0
    let durationSeconds = 0
    for (const clip of clips) {
      sizeBytes += clip.sizeBytes
      durationSeconds += clip.metadata.durationSeconds ?? 0
    }
    const others = otherByReel.get(reel.name) ?? []
    otherByReel.delete(reel.name)
    return {
      name: reel.name,
      clips,
      otherFiles: others,
      stats: {
        clipCount: clips.length,
        otherFileCount: others.length,
        otherFileSizeBytes: sumBytes(others),
        totalSizeBytes: sizeBytes + sumBytes(others),
        totalDurationSeconds: durationSeconds,
      },
    }
  })
  for (const [name, others] of otherByReel) {
    reels.push({
      name,
      clips: [],
      otherFiles: others,
      stats: {
        clipCount: 0,
        otherFileCount: others.length,
        otherFileSizeBytes: sumBytes(others),
        totalSizeBytes: sumBytes(others),
        totalDurationSeconds: 0,
      },
    })
  }
  reels.sort((a, b) => a.name.localeCompare(b.name, undefined, { numeric: true }))

  return {
    cover: input.cover,
    sourceRoot: input.sourceRoot ?? '',
    stats: {
      cardCount: cardCountFrom([
        ...input.clips.map((c) => c.relativePath),
        ...input.otherFiles.map((f) => f.relativePath),
      ]),
      clipCount: input.clips.length,
      otherFileCount: input.otherFiles.length,
      otherFileSizeBytes,
      totalDurationSeconds,
      totalSizeBytes: clipSizeBytes + otherFileSizeBytes,
    },
    reels,
  }
}
