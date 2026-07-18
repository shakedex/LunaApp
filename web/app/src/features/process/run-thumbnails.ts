import {
  type ClipRef,
  decodePathFor,
  runPool,
  type ThumbnailFrame,
  thumbnailTimestamps,
} from '@luna-web/core'
import { scanStore } from '../scan/store'
import { createFfmpegEngine, type FfmpegEngine } from './ffmpeg-engine'
import { guardedUpdate, isRunCurrent, poolSizeFor, withTimeout } from './run-processing'
import { createThumbsWorker, type ThumbsWorkerHandle } from './thumbs-client'

const THUMB_WIDTH = 1280
const MEDIABUNNY_TIMEOUT_MS = 60_000 // spec §10.3
const FFMPEG_TIMEOUT_MS = 180_000 // spec §10.3

export async function startThumbnails(run: number): Promise<void> {
  if (!isRunCurrent(run)) return
  const state = scanStore.state
  const clips = state.clips

  const mediabunnyClips: ClipRef[] = []
  const ffmpegClips: ClipRef[] = []
  for (const clip of clips) {
    const path = decodePathFor(clip.extension)
    if (path === 'mediabunny') mediabunnyClips.push(clip)
    else if (path === 'ffmpeg') ffmpegClips.push(clip)
  }

  if (mediabunnyClips.length === 0 && ffmpegClips.length === 0) {
    guardedUpdate(run, (s) => ({ ...s, phase: 'processed' }))
    return
  }

  guardedUpdate(run, (s) => ({
    ...s,
    phase: 'thumbnailing',
    thumbStatus: Object.fromEntries(
      clips
        .filter((c) => {
          const path = decodePathFor(c.extension)
          // decodePathFor can now return 'preview' (.crm/.r3d reclassified as
          // clips, thumbs/router.ts). Task 4 wires the preview queue — treat
          // 'preview' like 'none' for now so these clips aren't seeded into
          // thumbStatus as 'queued' with no pool to ever resolve them (that
          // would wedge thumbDoneCount/thumbTotal and the per-row spinner).
          return path === 'mediabunny' || path === 'ffmpeg'
        })
        .map((c) => [c.id, 'queued' as const]),
    ),
  }))

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

  guardedUpdate(run, (s) => ({ ...s, phase: 'processed' }))
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
