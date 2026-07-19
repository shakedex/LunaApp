import type {
  ClipMetadata,
  ReelStats,
  ReportModel,
  ReportStats,
  ThumbnailOutcome,
} from '@luna-web/core'

export interface PdfFrame {
  dataUrl: string | null
  outcome: ThumbnailOutcome
}

export interface PdfClip {
  fileName: string
  relativePath: string
  sizeBytes: number
  metadata: ClipMetadata
  frames: PdfFrame[]
}

export interface PdfReel {
  name: string
  stats: ReelStats
  clips: PdfClip[]
}

export interface PdfReport {
  cover: {
    projectTitle?: string
    productionCompany?: string
    dit?: string
    director?: string
    dp?: string
    date: string
    logoDataUrl: string | null
  }
  stats: ReportStats
  reels: PdfReel[]
  rawCount: number
}

function blobToDataUrl(blob: Blob): Promise<string> {
  return new Promise((resolve, reject) => {
    const reader = new FileReader()
    reader.onload = () => resolve(reader.result as string)
    reader.onerror = () => reject(reader.error ?? new Error('Failed to read image'))
    reader.readAsDataURL(blob)
  })
}

const PDF_FRAME_MAX_WIDTH = 480

// react-pdf renders JPEG/PNG only. ffmpeg-path frames are already JPEG and
// pass through; mediabunny-path frames are WebP and are re-encoded via canvas.
// Always decode + downscale: thumbnails are stored at 1280px but render at
// ~160pt in the PDF — embedding full size would bloat a 200-clip report by
// hundreds of MB (final-review finding). JPEG passthrough only applies when
// the source is already small enough.
async function frameToJpegDataUrl(image: Blob, _mime: string | undefined): Promise<string> {
  const bitmap = await createImageBitmap(image)
  try {
    if (_mime === 'image/jpeg' && bitmap.width <= PDF_FRAME_MAX_WIDTH) return blobToDataUrl(image)
    const scale = Math.min(1, PDF_FRAME_MAX_WIDTH / bitmap.width)
    const width = Math.max(1, Math.round(bitmap.width * scale))
    const height = Math.max(1, Math.round(bitmap.height * scale))
    const canvas = new OffscreenCanvas(width, height)
    const ctx = canvas.getContext('2d')
    if (!ctx) throw new Error('Canvas 2d context unavailable')
    ctx.drawImage(bitmap, 0, 0, width, height)
    const jpeg = await canvas.convertToBlob({ type: 'image/jpeg', quality: 0.85 })
    return blobToDataUrl(jpeg)
  } finally {
    bitmap.close()
  }
}

// Logo keeps alpha: re-encode to PNG unless it already is one.
async function logoToPngDataUrl(logo: Blob): Promise<string> {
  if (logo.type === 'image/png') return blobToDataUrl(logo)
  const bitmap = await createImageBitmap(logo)
  try {
    const canvas = new OffscreenCanvas(bitmap.width, bitmap.height)
    const ctx = canvas.getContext('2d')
    if (!ctx) throw new Error('Canvas 2d context unavailable')
    ctx.drawImage(bitmap, 0, 0)
    return blobToDataUrl(await canvas.convertToBlob({ type: 'image/png' }))
  } finally {
    bitmap.close()
  }
}

export async function prepareReportForPdf(report: ReportModel<Blob>): Promise<PdfReport> {
  const reels: PdfReel[] = []
  for (const reel of report.reels) {
    const clips: PdfClip[] = []
    for (const clip of reel.clips) {
      const frames: PdfFrame[] = []
      for (const frame of clip.thumbnails) {
        if (frame.outcome === 'Success' && frame.image) {
          try {
            frames.push({
              dataUrl: await frameToJpegDataUrl(frame.image, frame.mime),
              outcome: frame.outcome,
            })
          } catch {
            frames.push({ dataUrl: null, outcome: 'DecodeFailed' })
          }
        } else {
          frames.push({ dataUrl: null, outcome: frame.outcome })
        }
      }
      clips.push({
        fileName: clip.fileName,
        relativePath: clip.relativePath,
        sizeBytes: clip.sizeBytes,
        metadata: clip.metadata,
        frames,
      })
    }
    reels.push({ name: reel.name, stats: reel.stats, clips })
  }

  let logoDataUrl: string | null = null
  if (report.cover.logo) {
    try {
      logoDataUrl = await logoToPngDataUrl(report.cover.logo)
    } catch {
      logoDataUrl = null // a bad logo must not sink the export
    }
  }

  return {
    cover: {
      projectTitle: report.cover.projectTitle,
      productionCompany: report.cover.productionCompany,
      dit: report.cover.dit,
      director: report.cover.director,
      dp: report.cover.dp,
      date: report.cover.date ?? new Date().toISOString().slice(0, 10),
      logoDataUrl,
    },
    stats: report.stats,
    reels,
    rawCount: report.raw.length,
  }
}
