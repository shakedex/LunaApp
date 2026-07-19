import type { ReportModel } from './model'

// What the /reports list renders — deliberately tiny and Blob-free so listing
// the library never loads a single stored frame.
export interface ReportSummary {
  id: string
  savedAt: number // epoch ms, caller-supplied (core stays clock-free)
  title: string
  sourceRoot: string
  clipCount: number
  otherFileCount: number
  totalSizeBytes: number // the card's bytes (model.stats), not the stored record's
  storedFrameBytes: number // bytes of thumbnail images persisted with the report
  hasThumbnails: boolean
}

export function summarizeReport<TImage>(
  model: ReportModel<TImage>,
  meta: { id: string; savedAt: number },
  imageBytesOf: (image: TImage) => number,
): ReportSummary {
  let storedFrameBytes = 0
  let hasThumbnails = false
  for (const reel of model.reels) {
    for (const clip of reel.clips) {
      for (const frame of clip.thumbnails) {
        if (frame.image !== undefined) storedFrameBytes += imageBytesOf(frame.image)
        if (frame.outcome === 'Success' && frame.image !== undefined) hasThumbnails = true
      }
    }
  }
  const projectTitle = model.cover.projectTitle
  const title =
    (typeof projectTitle === 'string' && projectTitle.trim() !== '' && projectTitle) ||
    (model.sourceRoot !== '' && model.sourceRoot) ||
    'Camera report'
  return {
    id: meta.id,
    savedAt: meta.savedAt,
    title,
    sourceRoot: model.sourceRoot,
    clipCount: model.stats.clipCount,
    otherFileCount: model.stats.otherFileCount,
    totalSizeBytes: model.stats.totalSizeBytes,
    storedFrameBytes,
    hasThumbnails,
  }
}

// Defensive read of persisted summaries — any past/future version may have
// written them. Invalid members are dropped, never repaired.
export function normalizeReportSummaries(raw: unknown): ReportSummary[] {
  if (!Array.isArray(raw)) return []
  return raw.filter(isReportSummary).sort((a, b) => b.savedAt - a.savedAt || (a.id < b.id ? 1 : -1))
}

function isReportSummary(value: unknown): value is ReportSummary {
  if (typeof value !== 'object' || value === null) return false
  const s = value as Record<string, unknown>
  return (
    typeof s.id === 'string' &&
    typeof s.savedAt === 'number' &&
    Number.isFinite(s.savedAt) &&
    typeof s.title === 'string' &&
    typeof s.sourceRoot === 'string' &&
    typeof s.clipCount === 'number' &&
    typeof s.otherFileCount === 'number' &&
    typeof s.totalSizeBytes === 'number' &&
    typeof s.storedFrameBytes === 'number' &&
    typeof s.hasThumbnails === 'boolean'
  )
}
