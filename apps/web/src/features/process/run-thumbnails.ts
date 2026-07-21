import {
  type ClipRef,
  type EmbeddedPreview,
  extractCrmPreview,
  extractMovTailPreview,
  extractRtnJpeg,
  runPool,
  THUMBNAIL_TARGET_WIDTH,
  type ThumbnailFrame,
  thumbnailRouteFor,
  thumbnailTimestamps,
} from '@luna-web/core'
import { errorMessage } from '@/lib/errors'
import { logger } from '@/lib/logger'
import { scanStore } from '../scan/store'
import { createFfmpegEngine, type FfmpegEngine } from './ffmpeg-engine'
import { previewToFrame } from './preview-frame'
import {
  guardedUpdate,
  isRunCurrent,
  poolSizeFor,
  withCancellation,
  withTimeout,
} from './run-processing'
import { createThumbsWorker, type ThumbsWorkerHandle } from './thumbs-client'

const MEDIABUNNY_TIMEOUT_MS = 60_000 // spec §10.3
const FFMPEG_TIMEOUT_MS = 180_000 // spec §10.3
const PREVIEW_TIMEOUT_MS = 30_000

// withTimeout/withCancellation prefix the clip's filename ("<name>: <reason>"),
// and filenames legitimately contain words like "container" or "format" — so
// classify on the reason, never the whole message. Decoder errors arrive
// unprefixed (mediabunny throws 'NO_DECODER'; Comlink re-throws it verbatim),
// hence the no-separator fallback.
function reasonOf(message: string): string {
  const sep = message.indexOf(': ')
  return sep === -1 ? message : message.slice(sep + 2)
}

// A decoder/container failure is a routing fact, not a clip defect — it feeds
// the cascade (mediabunny → ffmpeg) and the preview salvage (ffmpeg → embedded
// preview). Exported for run-thumbnails.test.ts so the tests assert these
// predicates rather than copies that can drift.
export function isDecoderFailure(message: string): boolean {
  const reason = reasonOf(message)
  return reason.includes('NO_DECODER') || /format|recognized|container/i.test(reason)
}

// A timeout is also non-terminal: spec §10.3, and a clip stays in the report
// whether or not a thumbnail could be made for it. It routes to the same
// salvage pass as a decoder failure.
export function isTimeout(message: string): boolean {
  return /timed out after \d+s$/.test(reasonOf(message))
}

export async function startThumbnails(run: number): Promise<void> {
  if (!isRunCurrent(run)) return
  const state = scanStore.state
  const clips = state.clips

  const mediabunnyClips: ClipRef[] = []
  const ffmpegClips: ClipRef[] = []
  const previewClips: ClipRef[] = []
  // Route 'none' but still a real clip — `.braw` and `.ari` (no decode path,
  // no embedded preview, FINDINGS.md). Gets one NoDecoder placeholder frame
  // immediately below, no queue.
  const noPreviewClips: ClipRef[] = []
  for (const clip of clips) {
    const codec = state.metadataById[clip.id]?.codec
    const route = thumbnailRouteFor(clip.extension, codec)
    if (route === 'mediabunny') mediabunnyClips.push(clip)
    else if (route === 'ffmpeg') ffmpegClips.push(clip)
    else if (route === 'preview') previewClips.push(clip)
    else if (route === 'none') noPreviewClips.push(clip)
  }

  logger.info(
    'Thumbnail pass started',
    `${mediabunnyClips.length} via WebCodecs, ${ffmpegClips.length} via ffmpeg, ${previewClips.length} via embedded preview, ${noPreviewClips.length} placeholders`,
  )

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

  if (noPreviewClips.length > 0) {
    logger.info(
      `${noPreviewClips.length} clip(s) have no browser decode path (BRAW/ARRIRAW) — placeholder frames used`,
    )
    for (const clip of noPreviewClips) {
      finishClip(run, clip.id, [noDecoderFrame()])
    }
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
          lane.api.thumbnails(file, timestamps, THUMBNAIL_TARGET_WIDTH),
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
        const message = errorMessage(err)
        if (isDecoderFailure(message)) {
          // Guarded: superseded-run stragglers must not log under the new operation.
          if (isRunCurrent(run)) logger.debug(`Cascading ${clip.fileName} to ffmpeg`, message)
          cascaded.push(clip)
          cascadedIds.add(clip.id)
          setThumbStatus(run, clip.id, 'queued')
        } else {
          if (isRunCurrent(run)) logger.warn(`Thumbnails failed for ${clip.fileName}`, message)
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
    const message = errorMessage(err)
    for (const clip of mediabunnyClips) {
      if (cascadedIds.has(clip.id)) continue // legitimately re-queued for ffmpeg
      const st = scanStore.state.thumbStatus[clip.id]
      if (st === 'queued' || st === 'decoding') failClip(run, clip.id, message)
    }
  })

  // Clips that BOTH decoders rejected (e.g. ProRes RAW whose metadata pass
  // failed, so codec routing never sent them to the preview path) get one last
  // chance: the embedded tail preview. Decoder failures and timeouts qualify —
  // other errors stay failed.
  const salvage: ClipRef[] = []
  const salvageIds = new Set<string>()

  const ffmpegQueue = [...ffmpegClips, ...cascaded]
  if (ffmpegQueue.length > 0 && isRunCurrent(run)) {
    // Lanes are whole ffmpeg wasm instances — ~31 MB binary plus a per-lane
    // decode heap that grows well past that for 4K frames. Nothing bounds the
    // bytes: poolSizeFor bounds the lane count only (Settings worker cap,
    // hardwareConcurrency, queue size), so the worker cap is the only lever
    // over peak memory.
    // Cold-start binary downloads are single-flighted in engine-cache.
    const ffmpegLanes = poolSizeFor(ffmpegQueue.length)
    logger.info('ffmpeg pass started', `${ffmpegQueue.length} clips, ${ffmpegLanes} workers`)
    await runPool<FfmpegEngine, ClipRef, ThumbnailFrame<Blob>[]>(
      ffmpegQueue,
      {
        createLane: () => createFfmpegEngine(),
        destroyLane: (lane) => lane.dispose(),
        // Cancelled lanes must let go of their wasm engine promptly — see
        // withCancellation. Only this pool needs it: mediabunny/preview lanes
        // are cheap to strand.
        run: (lane, clip) =>
          withCancellation(
            async () => {
              const file = (await clip.file.getFile()) as File // documented boundary (see run-processing)
              const duration = scanStore.state.metadataById[clip.id]?.durationSeconds ?? 0
              const timestamps = thumbnailTimestamps(duration)
              return await withTimeout(
                lane.thumbnails(file, timestamps, THUMBNAIL_TARGET_WIDTH),
                FFMPEG_TIMEOUT_MS,
                clip.fileName,
              )
            },
            () => !isRunCurrent(run),
            clip.fileName,
          ),
      },
      {
        onItemStart: (clip) => setThumbStatus(run, clip.id, 'decoding'),
        onItemSuccess: (clip, frames) => finishClip(run, clip.id, frames),
        onItemFailure: (clip, err) => {
          const message = errorMessage(err)
          // A decoder failure only salvages for cascaded clips (a directly
          // routed clip already had its decoder); a timeout salvages for any
          // clip, because the clip must stay in the report either way.
          const salvageable =
            isTimeout(message) || (cascadedIds.has(clip.id) && isDecoderFailure(message))
          if (salvageable) {
            salvage.push(clip)
            salvageIds.add(clip.id)
            // Guarded: superseded-run stragglers must not log under the new operation.
            if (isRunCurrent(run)) {
              logger.info(
                isTimeout(message)
                  ? `${clip.fileName} timed out — trying embedded preview`
                  : `No browser decoder for ${clip.fileName} — trying embedded preview`,
              )
            }
            setThumbStatus(run, clip.id, 'queued')
          } else {
            if (isRunCurrent(run)) logger.warn(`Thumbnails failed for ${clip.fileName}`, message)
            failClip(run, clip.id, message)
          }
        },
      },
      {
        concurrency: ffmpegLanes,
        isCancelled: () => !isRunCurrent(run),
        maxAttempts: 1, // a decode that exhausted its budget will not finish on a rerun
      },
    ).catch((err) => {
      const message = errorMessage(err)
      for (const clip of ffmpegQueue) {
        if (salvageIds.has(clip.id)) continue // legitimately re-queued for preview
        const st = scanStore.state.thumbStatus[clip.id]
        if (st === 'queued' || st === 'decoding') failClip(run, clip.id, message)
      }
    })
  }

  const previewQueue = [...previewClips, ...salvage]
  if (previewQueue.length > 0 && isRunCurrent(run)) {
    await runPool<object, ClipRef, ThumbnailFrame<Blob>>(
      previewQueue,
      {
        createLane: () => ({}),
        destroyLane: () => {},
        run: (_lane, clip) =>
          withTimeout(buildPreviewFrame(run, clip), PREVIEW_TIMEOUT_MS, clip.fileName),
      },
      {
        onItemStart: (clip) => setThumbStatus(run, clip.id, 'decoding'),
        onItemSuccess: (clip, frame) => finishClip(run, clip.id, [frame]),
        onItemFailure: (clip, err) => failClip(run, clip.id, errorMessage(err)),
      },
      // Not poolSizeFor: a preview lane is `{}` — no worker, no wasm — so this
      // bounds concurrent file reads, not CPU.
      { concurrency: 2, isCancelled: () => !isRunCurrent(run) },
    ).catch((err) => {
      // Mirrors the ffmpeg pool-failure sweep — no cascade target here, so
      // straggling queued/decoding preview clips just fail terminally.
      const message = errorMessage(err)
      for (const clip of previewQueue) {
        const st = scanStore.state.thumbStatus[clip.id]
        if (st === 'queued' || st === 'decoding') failClip(run, clip.id, message)
      }
    })
  }

  // Guarded: a superseded run must not stamp a false "complete" into /activity.
  if (isRunCurrent(run)) logger.info('Thumbnail pass complete')
  guardedUpdate(run, (s) => ({ ...s, phase: 'processed' }))
}

// Null preview (no embedded/sidecar frame found) is expected, not an error —
// a single placeholder frame keeps rows/PDF showing a "no preview" tile with
// status 'done' rather than 'failed'.
function noDecoderFrame(): ThumbnailFrame<Blob> {
  return { positionRatio: 0.5, timestampSeconds: 0, outcome: 'NoDecoder' }
}

// A `.rtn` is a ~50 KB thumbnail sidecar; anything huge is not a .rtn and must
// not be slurped into memory whole (P8 final-review carry-forward).
const RTN_MAX_BYTES = 8 * 1024 * 1024

async function buildPreviewFrame(run: number, clip: ClipRef): Promise<ThumbnailFrame<Blob>> {
  let preview: EmbeddedPreview | null
  if (clip.extension === '.crm') {
    preview = await extractCrmPreview(await clip.file.getFile())
  } else if (clip.extension === '.r3d') {
    if (clip.previewSidecar) {
      const sidecar = await clip.previewSidecar.getFile()
      if (sidecar.size > RTN_MAX_BYTES) {
        // Guarded: superseded-run stragglers must not log under the new operation.
        if (isRunCurrent(run)) {
          logger.warn(
            `${clip.fileName}: .rtn sidecar is ${sidecar.size} bytes — too large, skipping`,
          )
        }
        preview = null
      } else {
        preview = extractRtnJpeg(new Uint8Array(await sidecar.arrayBuffer()))
      }
    } else {
      preview = null
    }
  } else {
    // Any other route === 'preview' clip is a mediabunny-container extension
    // whose codec matched PRORES_RAW_CODEC_PATTERN (thumbnailRouteFor), or a
    // salvage clip both decoders rejected — the tail-of-container embedded
    // preview (ProRes RAW's moov/udta) is the last honest source of pixels.
    preview = await extractMovTailPreview(await clip.file.getFile())
  }
  if (!preview) {
    // Guarded: superseded-run stragglers must not log under the new operation.
    if (isRunCurrent(run))
      logger.info(`No embedded preview in ${clip.fileName} — placeholder frame used`)
    return noDecoderFrame()
  }
  return await previewToFrame(preview)
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
