import type { ClipMetadata } from '../metadata/model'
import { detectReels } from '../reels/detect'
import type { ClipRef, RawNotice } from '../scan/model'
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
  totalSizeBytes: number
  totalDurationSeconds: number
}

export interface Reel<TImage = unknown> {
  name: string
  clips: ReportClip<TImage>[]
  stats: ReelStats
}

export interface ReportStats {
  cardCount: number
  clipCount: number
  rawCount: number
  totalDurationSeconds: number
  totalSizeBytes: number
}

export interface ReportModel<TImage = unknown> {
  cover: CoverFields<TImage>
  stats: ReportStats
  reels: Reel<TImage>[]
  raw: RawNotice[]
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
  raw: readonly RawNotice[]
  metadataById: Readonly<Record<string, ClipMetadata>>
  thumbsById: Readonly<Record<string, ThumbnailFrame<TImage>[]>>
  cover: CoverFields<TImage>
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
  let totalSizeBytes = 0
  for (const clip of reportClips) {
    totalDurationSeconds += clip.metadata.durationSeconds ?? 0
    totalSizeBytes += clip.sizeBytes
  }

  const reels = detectReels(reportClips).map((reel) => {
    const clips = reel.clips.map(({ reelName: _drop, ...clip }) => clip)
    let totalSizeBytes = 0
    let totalDurationSeconds = 0
    for (const clip of clips) {
      totalSizeBytes += clip.sizeBytes
      totalDurationSeconds += clip.metadata.durationSeconds ?? 0
    }
    return {
      name: reel.name,
      clips,
      stats: { clipCount: clips.length, totalSizeBytes, totalDurationSeconds },
    }
  })

  return {
    cover: input.cover,
    stats: {
      cardCount: cardCountFrom(input.clips.map((c) => c.relativePath)),
      clipCount: input.clips.length,
      rawCount: input.raw.length,
      totalDurationSeconds,
      totalSizeBytes,
    },
    reels,
    raw: [...input.raw],
  }
}
