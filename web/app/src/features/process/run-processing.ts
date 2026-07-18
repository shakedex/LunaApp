import { type ClipRef, mapMediaInfoToClipMetadata } from '@luna-web/core'
import { scanStore } from '../scan/store'
import { createMetadataWorker, type MetadataWorkerHandle } from './metadata-client'

const POOL_CAP = 4
const METADATA_TIMEOUT_MS = 5 * 60_000 // spec §10.3: per-clip metadata timeout

export async function startProcessing(): Promise<void> {
  const state = scanStore.state
  if (state.phase !== 'summary') return
  const clips = state.clips

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
    let handle = createMetadataWorker()
    try {
      for (;;) {
        const index = nextIndex++
        if (index >= clips.length) return
        const clip = clips[index]
        setStatus(clip.id, 'processing')
        let attempt = 0
        for (;;) {
          try {
            const metadata = await analyzeClip(handle, clip)
            scanStore.setState((s) => ({
              ...s,
              clipStatus: { ...s.clipStatus, [clip.id]: 'done' },
              metadataById: { ...s.metadataById, [clip.id]: metadata },
              processedCount: s.processedCount + 1,
            }))
            break
          } catch (err) {
            // Isolate the failure to this clip: recycle the worker (it may be
            // wedged) and retry once on a fresh one (spec §15).
            handle.worker.terminate()
            handle = createMetadataWorker()
            attempt += 1
            if (attempt >= 2) {
              const message = err instanceof Error ? err.message : String(err)
              scanStore.setState((s) => ({
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
      handle.worker.terminate()
    }
  }

  await Promise.all(Array.from({ length: poolSize }, () => workerLoop()))
  scanStore.setState((s) => ({ ...s, phase: 'processed' }))
}

function setStatus(id: string, status: 'processing'): void {
  scanStore.setState((s) => ({ ...s, clipStatus: { ...s.clipStatus, [id]: status } }))
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
