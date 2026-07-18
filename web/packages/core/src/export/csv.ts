import type { ReportModel } from '../report/model'
import type { ThumbnailFrame, ThumbnailOutcome } from '../thumbs/model'

// Spec §12.2 — exact column order. Note: deliberately no "card" column; if one
// is ever added it derives from the top-level folder, NOT the reel name.
export const CSV_COLUMNS: readonly string[] = [
  'reel',
  'fileName',
  'relativePath',
  'startTimecode',
  'width',
  'height',
  'codec',
  'frameRate',
  'durationSeconds',
  'sizeBytes',
  'colorSpace',
  'camera',
  'iso',
  'whiteBalance',
  'lens',
  'focalLength',
  'aperture',
  'shutter',
  'gamma',
  'thumbnailOutcome',
]

export function aggregateThumbnailOutcome(frames: readonly ThumbnailFrame[]): ThumbnailOutcome {
  if (frames.length === 0) return 'NotAttempted'
  if (frames.some((f) => f.outcome === 'Success')) return 'Success'
  return frames[0]?.outcome ?? 'NotAttempted'
}

function field(value: unknown): string {
  if (value === undefined || value === null) return ''
  let text = String(value)
  // Spreadsheet formula-injection guard: neutralize leading = + - @
  if (/^[=+\-@]/.test(text)) text = `'${text}`
  return /[",\r\n]/.test(text) ? `"${text.replaceAll('"', '""')}"` : text
}

export function generateReportCsv(report: ReportModel): string {
  const lines = [CSV_COLUMNS.join(',')]
  for (const reel of report.reels) {
    for (const clip of reel.clips) {
      const m = clip.metadata
      lines.push(
        [
          reel.name,
          clip.fileName,
          clip.relativePath,
          m.startTimecode,
          m.width,
          m.height,
          m.codec,
          m.frameRate,
          m.durationSeconds,
          clip.sizeBytes,
          m.colorSpace,
          m.camera,
          m.iso,
          m.whiteBalance,
          m.lens,
          m.focalLength,
          m.aperture,
          m.shutter,
          m.gamma,
          aggregateThumbnailOutcome(clip.thumbnails),
        ]
          .map(field)
          .join(','),
      )
    }
  }
  return lines.join('\r\n')
}
