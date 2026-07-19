import type { EmbeddedPreview, ThumbnailFrame } from '@luna-web/core'

const PREVIEW_MAX_WIDTH = 1280
const WEBP_QUALITY = 0.85

/**
 * Normalizes an embedded RAW preview (Task 2/3 extractors: Canon `.crm`'s
 * PRVW box, ProRes RAW's `moov/udta` tail, RED's `.rtn` sidecar) into the
 * same `ThumbnailFrame<Blob>` shape the mediabunny/ffmpeg decode paths
 * already produce (thumbs.worker.ts) — a single WebP poster frame — so
 * downstream rendering/export code never needs a third image-format branch.
 * These previews arrive as raw embedded JPEG bytes at whatever resolution
 * the camera stored, never a decoded video frame, so there is exactly one
 * frame to build (variable per-clip frame counts are already handled
 * downstream for the decode paths).
 */
export async function previewToFrame(preview: EmbeddedPreview): Promise<ThumbnailFrame<Blob>> {
  const jpegBlob = new Blob([preview.jpeg as BlobPart], { type: 'image/jpeg' })
  const bitmap = await createImageBitmap(jpegBlob)
  try {
    const scale = Math.min(1, PREVIEW_MAX_WIDTH / bitmap.width) // never upscale
    const width = Math.max(1, Math.round(bitmap.width * scale))
    const height = Math.max(1, Math.round(bitmap.height * scale))
    const canvas = new OffscreenCanvas(width, height)
    const ctx = canvas.getContext('2d')
    if (!ctx) throw new Error('Canvas 2d context unavailable')
    ctx.drawImage(bitmap, 0, 0, width, height)
    const image = await canvas.convertToBlob({ type: 'image/webp', quality: WEBP_QUALITY })
    return {
      positionRatio: 0.5,
      timestampSeconds: 0,
      image,
      mime: 'image/webp',
      outcome: 'Success',
    }
  } finally {
    bitmap.close()
  }
}
