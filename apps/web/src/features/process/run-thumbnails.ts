import {
  type ClipRef,
  type EmbeddedPreview,
  extractCrmPreview,
  extractMovTailPreview,
  extractRtnJpeg,
  runPool,
  type ThumbnailFrame,
  thumbnailRouteFor,
  thumbnailTimestamps,
} from '@luna-web/core'
import { scanStore } from '../scan/store'
import { createFfmpegEngine, type FfmpegEngine } from './ffmpeg-engine'
import { previewToFrame } from './preview-frame'
import { guardedUpdate, isRunCurrent, poolSizeFor, withTimeout } from './run-processing'
import { createThumbsWorker, type ThumbsWorkerHandle } from './thumbs-client'

const THUMB_WIDTH = 1280
const MEDIABUNNY_TIMEOUT_MS = 60_000 // spec §10.3
const FFMPEG_TIMEOUT_MS = 180_000 // spec §10.3
const PREVIEW_TIMEOUT_MS = 30_000

export async function startThumbnails(run: number): Promise<void> {
  if (!isRunCurrent(run)) return
  const state = scanStore.state
  const clips = state.clips

  const mediabunnyClips: ClipRef[] = []
  const ffmpegClips: ClipRef[] = []
  const previewClips: ClipRef[] = []
  // Route 'none' but still a real clip — currently only `.braw` (no decode
  // path, no embedded preview, FINDINGS.md). Gets one NoDecoder placeholder
  // frame immediately below, no queue.
  const noPreviewClips: ClipRef[] = []
  for (const clip of clips) {
    const codec = state.metadataById[clip.id]?.codec
    const route = thumbnailRouteFor(clip.extension, codec)
    if (route === 'mediabunny') mediabunnyClips.push(clip)
    else if (route === 'ffmpeg') ffmpegClips.push(clip)
    else if (route === 'preview') previewClips.push(clip)
    else if (route === 'none') noPreviewClips.push(clip)
  }

  if (
    mediabunnyClips.length === 0 &&
    ffmpegClips.length === 0 &&
    previewClips.length === 0 &&
    noPreviewClips.length === 0
  ) {
    guardedUpdate(run, (s) => ({ ...s, phase: 'processed' }))
    return
  }

  guardedUpdate(run, (s) => ({
    ...s,
    phase: 'thumbnailing',
    thumbStatus: Object.fromEntries(
      [...mediabunnyClips, ...ffmpegClips, ...previewClips, ...noPreviewClips].map((c) => [
        c.id,
        'queued' as const,
      ]),
    ),
  }))

  for (const clip of noPreviewClips) {
    finishClip(run, clip.id, [noDecoderFrame()])
  }

  // Cascade queue: mediabunny container/codec failures retry on ffmpeg.
  const cascaded: ClipRef[] = []
  const cascadedIds = new Set<string>()

  const mediabunnyPass = runPool<ThumbsWorkerHandle, ClipRef, ThumbnailFrame<Blob>[]>(
    mediabunnyClips,
    {
      createLane: () => createThumbsWorker(),
      destroyLane: (lane) => lane.worker.terminate(),
      run: async (lane, clip) => {
        const file = (await clip.file.getFile()) as File // documented boundary (see run-processing)
        const duration = scanStore.state.metadataById[clip.id]?.durationSeconds ?? 0
        const timestamps = thumbnailTimestamps(duration)
        return withTimeout(
          lane.api.thumbnails(file, timestamps, THUMB_WIDTH),
          MEDIABUNNY_TIMEOUT_MS,
          clip.fileName,
        )
      },
    },
    {
      onItemStart: (clip) => setThumbStatus(run, clip.id, 'decoding'),
      onItemSuccess: (clip, frames) => finishClip(run, clip.id, frames),
      onItemFailure: (clip, err) => {
        // Container/codec failure → cascade to ffmpeg (desktop NoDecoder
        // cascade, spec §10.2). Other errors fail the clip.
        const message = err instanceof Error ? err.message : String(err)
        if (message.includes('NO_DECODER') || /format|recognized|container/i.test(message)) {
          cascaded.push(clip)
          cascadedIds.add(clip.id)
          setThumbStatus(run, clip.id, 'queued')
        } else {
          failClip(run, clip.id, message)
        }
      },
    },
    {
      concurrency: poolSizeFor(mediabunnyClips.length),
      isCancelled: () => !isRunCurrent(run),
      maxAttempts: 1, // deterministic NO_DECODER must not retry — the ffmpeg cascade IS the retry
    },
  )

  await mediabunnyPass.catch((err) => {
    // runPool settles all lanes before rejecting, so no stragglers are
    // running here: statuses are final and the cascade list is complete.
    const message = err instanceof Error ? err.message : String(err)
    for (const clip of mediabunnyClips) {
      if (cascadedIds.has(clip.id)) continue // legitimately re-queued for ffmpeg
      const st = scanStore.state.thumbStatus[clip.id]
      if (st === 'queued' || st === 'decoding') failClip(run, clip.id, message)
    }
  })

  const ffmpegQueue = [...ffmpegClips, ...cascaded]
  if (ffmpegQueue.length > 0 && isRunCurrent(run)) {
    await runPool<FfmpegEngine, ClipRef, ThumbnailFrame<Blob>[]>(
      ffmpegQueue,
      {
        createLane: () => createFfmpegEngine(),
        destroyLane: (lane) => lane.dispose(),
        run: async (lane, clip) => {
          const file = (await clip.file.getFile()) as File // documented boundary (see run-processing)
          const duration = scanStore.state.metadataById[clip.id]?.durationSeconds ?? 0
          const timestamps = thumbnailTimestamps(duration)
          return withTimeout(
            lane.thumbnails(file, timestamps, THUMB_WIDTH),
            FFMPEG_TIMEOUT_MS,
            clip.fileName,
          )
        },
      },
      {
        onItemStart: (clip) => setThumbStatus(run, clip.id, 'decoding'),
        onItemSuccess: (clip, frames) => finishClip(run, clip.id, frames),
        onItemFailure: (clip, err) =>
          failClip(run, clip.id, err instanceof Error ? err.message : String(err)),
      },
      // Each ffmpeg lane instantiates a ~31 MB wasm — keep it to ONE lane.
      { concurrency: 1, isCancelled: () => !isRunCurrent(run) },
    ).catch((err) => {
      const message = err instanceof Error ? err.message : String(err)
      for (const clip of ffmpegQueue) {
        const st = scanStore.state.thumbStatus[clip.id]
        if (st === 'queued' || st === 'decoding') failClip(run, clip.id, message)
      }
    })
  }

  if (previewClips.length > 0 && isRunCurrent(run)) {
    await runPool<object, ClipRef, ThumbnailFrame<Blob>>(
      previewClips,
      {
        createLane: () => ({}),
        destroyLane: () => {},
        run: (_lane, clip) =>
          withTimeout(buildPreviewFrame(clip), PREVIEW_TIMEOUT_MS, clip.fileName),
      },
      {
        onItemStart: (clip) => setThumbStatus(run, clip.id, 'decoding'),
        onItemSuccess: (clip, frame) => finishClip(run, clip.id, [frame]),
        onItemFailure: (clip, err) =>
          failClip(run, clip.id, err instanceof Error ? err.message : String(err)),
      },
      { concurrency: 2, isCancelled: () => !isRunCurrent(run) },
    ).catch((err) => {
      // Mirrors the ffmpeg pool-failure sweep — no cascade target here, so
      // straggling queued/decoding preview clips just fail terminally.
      const message = err instanceof Error ? err.message : String(err)
      for (const clip of previewClips) {
        const st = scanStore.state.thumbStatus[clip.id]
        if (st === 'queued' || st === 'decoding') failClip(run, clip.id, message)
      }
    })
  }

  guardedUpdate(run, (s) => ({ ...s, phase: 'processed' }))
}

// Null preview (no embedded/sidecar frame found) is expected, not an error —
// a single placeholder frame keeps rows/PDF showing a "no preview" tile with
// status 'done' rather than 'failed'.
function noDecoderFrame(): ThumbnailFrame<Blob> {
  return { positionRatio: 0.5, timestampSeconds: 0, outcome: 'NoDecoder' }
}

async function buildPreviewFrame(clip: ClipRef): Promise<ThumbnailFrame<Blob>> {
  let preview: EmbeddedPreview | null
  if (clip.extension === '.crm') {
    preview = await extractCrmPreview(await clip.file.getFile())
  } else if (clip.extension === '.r3d') {
    preview = clip.previewSidecar
      ? extractRtnJpeg(new Uint8Array(await (await clip.previewSidecar.getFile()).arrayBuffer()))
      : null
  } else {
    // Any other route === 'preview' clip is a mediabunny-container extension
    // whose codec matched PRORES_RAW_CODEC_PATTERN (thumbnailRouteFor) — the
    // tail-of-container embedded preview (ProRes RAW's moov/udta).
    preview = await extractMovTailPreview(await clip.file.getFile())
  }
  return preview ? await previewToFrame(preview) : noDecoderFrame()
}

function setThumbStatus(run: number, id: string, status: 'queued' | 'decoding'): void {
  guardedUpdate(run, (s) => ({ ...s, thumbStatus: { ...s.thumbStatus, [id]: status } }))
}

function finishClip(run: number, id: string, frames: ThumbnailFrame<Blob>[]): void {
  guardedUpdate(run, (s) => ({
    ...s,
    thumbStatus: { ...s.thumbStatus, [id]: 'done' },
    thumbsById: { ...s.thumbsById, [id]: frames },
    thumbDoneCount: s.thumbDoneCount + 1,
  }))
}

function failClip(run: number, id: string, message: string): void {
  guardedUpdate(run, (s) => ({
    ...s,
    thumbStatus: { ...s.thumbStatus, [id]: 'failed' },
    thumbErrors: { ...s.thumbErrors, [id]: message },
    thumbDoneCount: s.thumbDoneCount + 1,
  }))
}
