/// <reference lib="webworker" />
import type { ThumbnailFrame } from '@luna-web/core'
import { THUMBNAIL_ENCODE_QUALITY, THUMBNAIL_POSITIONS } from '@luna-web/core'
import { registerProresDecoder } from '@mediabunny/prores'
import * as Comlink from 'comlink'
import { ALL_FORMATS, BlobSource, CanvasSink, Input } from 'mediabunny'
import type { ThumbsWorkerApi } from '../features/process/thumbs-client'

registerProresDecoder()

const api = {
  async thumbnails(
    file: File,
    timestamps: number[],
    width: number,
  ): Promise<ThumbnailFrame<Blob>[]> {
    const input = new Input({ formats: ALL_FORMATS, source: new BlobSource(file) })
    try {
      const track = await input.getPrimaryVideoTrack()
      if (track === null) throw new Error('NO_DECODER')
      if (!(await track.canDecode())) throw new Error('NO_DECODER')

      const displayWidth = await track.getDisplayWidth()
      const displayHeight = await track.getDisplayHeight()
      const height =
        displayWidth > 0 ? Math.round((width * displayHeight) / displayWidth) : undefined

      const sink = new CanvasSink(track, {
        width,
        ...(height !== undefined ? { height } : {}),
        fit: 'contain',
        poolSize: 2,
      })

      const frames: ThumbnailFrame<Blob>[] = []
      let index = 0
      for await (const wrapped of sink.canvasesAtTimestamps(timestamps)) {
        const positionRatio = THUMBNAIL_POSITIONS[index] ?? THUMBNAIL_POSITIONS[0] ?? 0.1
        const timestampSeconds = timestamps[index] ?? 0
        index += 1
        if (wrapped === null) {
          frames.push({ positionRatio, timestampSeconds, outcome: 'SeekFailed' })
          continue
        }
        // mediabunny's WrappedCanvas.canvas is typed as HTMLCanvasElement | OffscreenCanvas;
        // in a worker context (no DOM) it is always an OffscreenCanvas.
        const canvas = wrapped.canvas as OffscreenCanvas
        const blob = await canvas.convertToBlob({
          type: 'image/webp',
          quality: THUMBNAIL_ENCODE_QUALITY,
        })
        frames.push({
          positionRatio,
          timestampSeconds,
          image: blob,
          mime: 'image/webp',
          outcome: 'Success',
        })
      }
      return frames
    } finally {
      input.dispose()
    }
  },
} satisfies ThumbsWorkerApi

Comlink.expose(api)
