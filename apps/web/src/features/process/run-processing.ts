import {
  type ClipRef,
  extractSonyMxfCameraModel,
  mapMediaInfoToClipMetadata,
  runPool,
} from '@luna-web/core'
import { errorMessage } from '@/lib/errors'
import { beginOperation, logger } from '@/lib/logger'
import { type ScanState, scanStore } from '../scan/store'
import { settingsStore } from '../settings/settings-store'
import { createMetadataWorker, type MetadataWorkerHandle } from './metadata-client'
import { startThumbnails } from './run-thumbnails'

const METADATA_TIMEOUT_MS = 5 * 60_000 // spec §10.3: per-clip metadata timeout

// Run token: bumped by every start and by cancelProcessing(). Loops from a
// superseded run stop claiming clips, and their late writes are dropped, so
// "Start over" (or a new scan) can never be corrupted by orphaned workers.
let currentRun = 0

export function cancelProcessing(): void {
  currentRun += 1
}

export function isRunCurrent(run: number): boolean {
  return run === currentRun
}

// Drop writes from superseded runs or after the pipeline left its active
// phases ('processing' | 'thumbnailing').
export function guardedUpdate(run: number, updater: (s: ScanState) => ScanState): void {
  if (run !== currentRun) return
  scanStore.setState((s) =>
    s.phase === 'processing' || s.phase === 'thumbnailing' ? updater(s) : s,
  )
}

export function poolSizeFor(itemCount: number): number {
  // Settings-driven cap (spec §14, default 4, clamped 1–8 at the source);
  // hardwareConcurrency and the item count still bound it below that.
  const cap = settingsStore.state.workerPoolCap
  return Math.max(1, Math.min(cap, navigator.hardwareConcurrency || 2, itemCount))
}

export function withTimeout<T>(promise: Promise<T>, ms: number, label: string): Promise<T> {
  return new Promise<T>((resolve, reject) => {
    const timer = setTimeout(
      () => reject(new Error(`${label}: timed out after ${Math.round(ms / 1000)}s`)),
      ms,
    )
    promise.then(
      (v) => {
        clearTimeout(timer)
        resolve(v)
      },
      (e) => {
        clearTimeout(timer)
        reject(e)
      },
    )
  })
}

const CANCEL_POLL_MS = 250

// runPool checks cancellation only when claiming the next item, and nothing
// aborts work already inside hooks.run() — so a superseded lane holds whatever
// the item allocated until that item ends on its own. For ffmpeg that is a live
// wasm instance per lane, and "Start over" sits on the processing screen where
// the next action is usually picking another folder, so the old run's lanes
// would overlap the new run's. Rejecting early hands the lane to runPool's
// failure path, which destroys it. The message must stay clear of
// isDecoderFailure() in run-thumbnails and distinct from a timeout; the status
// write is dropped by guardedUpdate anyway, so it is for the activity log only.
export function withCancellation<T>(
  start: () => Promise<T>,
  isCancelled: () => boolean,
  label: string,
): Promise<T> {
  // Checked before starting too: runPool retries a failed item, and a retry
  // that re-created the resource would strand exactly what this prevents.
  if (isCancelled()) return Promise.reject(new Error(`${label}: cancelled`))
  return new Promise<T>((resolve, reject) => {
    const poll = setInterval(() => {
      if (!isCancelled()) return
      clearInterval(poll)
      reject(new Error(`${label}: cancelled`))
    }, CANCEL_POLL_MS)
    start().then(
      (v) => {
        clearInterval(poll)
        resolve(v)
      },
      (e) => {
        clearInterval(poll)
        reject(e)
      },
    )
  })
}

export async function startProcessing(): Promise<void> {
  const state = scanStore.state
  if (state.phase !== 'summary') return
  const clips = state.clips
  beginOperation('process', `Process: ${state.sourceName ?? 'card'} (${clips.length} clips)`)
  const run = ++currentRun

  scanStore.setState((s) => ({
    ...s,
    phase: 'processing',
    clipStatus: Object.fromEntries(clips.map((c) => [c.id, 'queued' as const])),
    metadataById: {},
    clipErrors: {},
    processedCount: 0,
    thumbStatus: {},
    thumbsById: {},
    thumbErrors: {},
    thumbDoneCount: 0,
  }))
  logger.info(
    'Metadata pass started',
    `${clips.length} clips, ${poolSizeFor(clips.length)} workers`,
  )

  try {
    await runPool<MetadataWorkerHandle, ClipRef, Awaited<ReturnType<typeof analyzeClip>>>(
      clips,
      {
        createLane: () => createMetadataWorker(),
        destroyLane: (lane) => lane.worker.terminate(),
        run: (lane, clip) => analyzeClip(lane, clip),
      },
      {
        onItemStart: (clip) =>
          guardedUpdate(run, (s) => ({
            ...s,
            clipStatus: { ...s.clipStatus, [clip.id]: 'processing' },
          })),
        onItemSuccess: (clip, metadata) =>
          guardedUpdate(run, (s) => ({
            ...s,
            clipStatus: { ...s.clipStatus, [clip.id]: 'done' },
            metadataById: { ...s.metadataById, [clip.id]: metadata },
            processedCount: s.processedCount + 1,
          })),
        onItemFailure: (clip, err) => {
          const message = errorMessage(err)
          // Guarded: a superseded run's late failure must not log under the
          // new operation that has since begun.
          if (isRunCurrent(run)) logger.warn(`Metadata failed for ${clip.fileName}`, message)
          guardedUpdate(run, (s) => ({
            ...s,
            clipStatus: { ...s.clipStatus, [clip.id]: 'failed' },
            clipErrors: { ...s.clipErrors, [clip.id]: message },
            processedCount: s.processedCount + 1,
          }))
        },
      },
      { concurrency: poolSizeFor(clips.length), isCancelled: () => run !== currentRun },
    )
  } catch (err) {
    // Stop sibling lanes doing wasted work, then surface the failure.
    cancelProcessing()
    const message = errorMessage(err)
    logger.error('Processing failed', message)
    scanStore.setState((s) =>
      s.phase === 'processing' ? { ...s, phase: 'error', error: message } : s,
    )
    return
  }
  // Guarded: a superseded run must not stamp a false "complete" into /activity.
  if (isRunCurrent(run)) logger.info('Metadata pass complete')

  if (run !== currentRun) return
  try {
    if (scanStore.state.generateThumbnails) {
      await startThumbnails(run)
    } else {
      logger.info('Thumbnails skipped (disabled for this run) — tech-data-only report')
      guardedUpdate(run, (s) => ({ ...s, phase: 'processed' }))
    }
  } catch (err) {
    // Same boundary as the metadata pass: never leave the UI wedged in
    // 'thumbnailing' — surface the failure and stop sibling work.
    cancelProcessing()
    const message = errorMessage(err)
    logger.error('Processing failed', message)
    scanStore.setState((s) =>
      s.phase === 'thumbnailing' || s.phase === 'processing'
        ? { ...s, phase: 'error', error: message }
        : s,
    )
  }
}

async function analyzeClip(handle: MetadataWorkerHandle, clip: ClipRef) {
  // App-side boundary: core types getFile() as structural BlobLike; here the
  // handle came from the real File System Access API, so this is a real File
  // (structured-cloneable into the worker).
  const file = (await clip.file.getFile()) as File
  const raw = await withTimeout(handle.api.analyze(file), METADATA_TIMEOUT_MS, clip.fileName)
  const metadata = mapMediaInfoToClipMetadata(raw)
  // Sony MXF resolves to the generic "Sony" from mediainfo alone — the model
  // string (RDD-18 CameraAttributes) is recorded per-frame in the essence
  // container, which the bounded KLV scan reads without touching the picture
  // essence (FINDINGS.md 2026-07-20).
  if (metadata.camera === 'Sony' && clip.extension === '.mxf') {
    const model = await extractSonyMxfCameraModel(file)
    if (model) return { ...metadata, camera: model }
  }
  return metadata
}
