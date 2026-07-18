import type { ClipRef } from '@luna-web/core'
import { mapMediaInfoToClipMetadata } from '@luna-web/core'
import { type ScanState, scanStore } from '../scan/store'
import { createMetadataWorker, type MetadataWorkerHandle } from './metadata-client'

const POOL_CAP = 4
const METADATA_TIMEOUT_MS = 5 * 60_000 // spec §10.3: per-clip metadata timeout

// Run token: bumped by every start and by cancelProcessing(). Loops from a
// superseded run stop claiming clips, and their late writes are dropped, so
// "Start over" (or a new scan) can never be corrupted by orphaned workers.
let currentRun = 0

export function cancelProcessing(): void {
  currentRun += 1
}

export async function startProcessing(): Promise<void> {
  const state = scanStore.state
  if (state.phase !== 'summary') return
  const clips = state.clips
  const run = ++currentRun

  scanStore.setState((s) => ({
    ...s,
    phase: 'processing',
    clipStatus: Object.fromEntries(clips.map((c) => [c.id, 'queued' as const])),
    metadataById: {},
    clipErrors: {},
    processedCount: 0,
  }))

  const poolSize = Math.max(1, Math.min(POOL_CAP, navigator.hardwareConcurrency || 2, clips.length))
  let nextIndex = 0

  async function workerLoop(): Promise<void> {
    let handle: MetadataWorkerHandle | null = null
    try {
      handle = createMetadataWorker()
      for (;;) {
        if (run !== currentRun) return // superseded: stop claiming clips
        const index = nextIndex++
        if (index >= clips.length) return
        const clip = clips[index]
        updateRun(run, (s) => ({ ...s, clipStatus: { ...s.clipStatus, [clip.id]: 'processing' } }))
        let attempt = 0
        for (;;) {
          try {
            if (handle === null) handle = createMetadataWorker()
            const metadata = await analyzeClip(handle, clip)
            updateRun(run, (s) => ({
              ...s,
              clipStatus: { ...s.clipStatus, [clip.id]: 'done' },
              metadataById: { ...s.metadataById, [clip.id]: metadata },
              processedCount: s.processedCount + 1,
            }))
            break
          } catch (err) {
            // Isolate the failure to this clip: recycle the worker (it may be
            // wedged) and retry once on a fresh one (spec §15).
            handle?.worker.terminate()
            handle = null
            attempt += 1
            if (attempt >= 2) {
              const message = err instanceof Error ? err.message : String(err)
              updateRun(run, (s) => ({
                ...s,
                clipStatus: { ...s.clipStatus, [clip.id]: 'failed' },
                clipErrors: { ...s.clipErrors, [clip.id]: message },
                processedCount: s.processedCount + 1,
              }))
              break
            }
          }
        }
      }
    } finally {
      handle?.worker.terminate()
    }
  }

  try {
    await Promise.all(Array.from({ length: poolSize }, () => workerLoop()))
    updateRun(run, (s) => ({ ...s, phase: 'processed' }))
  } catch (err) {
    // Top-level boundary: a worker-construction or other unexpected failure
    // must surface as an error phase, never a forever-"processing" screen.
    const message = err instanceof Error ? err.message : String(err)
    updateRun(run, (s) => ({ ...s, phase: 'error', error: message }))
  }
}

// Apply an update only if this run is still current AND we're still in the
// processing phase — late writes from orphaned loops are dropped.
function updateRun(run: number, updater: (s: ScanState) => ScanState): void {
  if (run !== currentRun) return
  scanStore.setState((s) => (s.phase === 'processing' ? updater(s) : s))
}

async function analyzeClip(handle: MetadataWorkerHandle, clip: ClipRef) {
  // App-side boundary: core types getFile() as structural BlobLike; here the
  // handle came from the real File System Access API, so this is a real File
  // (structured-cloneable into the worker).
  const file = (await clip.file.getFile()) as File
  const raw = await withTimeout(handle.api.analyze(file), METADATA_TIMEOUT_MS, clip.fileName)
  return mapMediaInfoToClipMetadata(raw)
}

function withTimeout<T>(promise: Promise<T>, ms: number, label: string): Promise<T> {
  return new Promise<T>((resolve, reject) => {
    const timer = setTimeout(
      () =>
        reject(
          new Error(`${label}: metadata extraction timed out after ${Math.round(ms / 1000)}s`),
        ),
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
