# Luna Web — Plan 03: Metadata Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** After the user confirms a scan, Luna Web extracts real per-clip metadata (resolution, codec, frame rate, duration, color space, timecode, reel name) with mediainfo.js running in a worker pool, and shows it live in the clip list — all local.

**Architecture:** mediainfo.js (WASM) runs inside module Workers exposed via Comlink; each worker analyzes one clip at a time with chunked `file.slice` reads (no whole-file loads). A small pool (≤4 workers) processes clips concurrently with per-clip timeout and retry-once-on-fresh-worker. The mediainfo→`ClipMetadata` field mapping is **pure logic in `packages/core`**, unit-tested against fixture JSON — the worker returns the raw mediainfo object; the main thread maps it. Core's `FileHandleLike.getFile()` widens to a structural `BlobLike` (Plan 02 carry-forward) so metadata now — and ffmpeg/WebCodecs later — share one contract.

**Tech Stack (additions):** `mediainfo.js`, `comlink` — added via `bun add` at latest.

**Spec:** `docs/superpowers/specs/2026-07-17-luna-web-design.md` §8.3 (orchestrator + pool), §8.6 (metadata), §9 (`ClipMetadata`), §10.3 (5-min per-clip metadata timeout), §13 (fidelity: missing fields stay blank, never fabricated), §15 (worker crash isolated, retried once).

## Global Constraints

- **Never hand-write a dependency version.** New deps via `bun add` (latest). Workspace stays unified on **TypeScript 6**.
- **`packages/core` stays DOM-free.** `BlobLike` is structural (no DOM `Blob`/`File` types); the mapper takes plain JSON.
- **Tests:** `bun test`, core only. The mapper and the widened handles get tests; worker/pool/UI are maintainer-QA.
- **Gates green after every task** (from `web/`): `bun run lint && bun run typecheck && bun test && bun run build` — all exit 0 before each commit (`bunx @biomejs/biome check --write .` for formatting drift).
- **Metadata fidelity (spec §13):** a field missing from the container maps to `undefined` and renders blank (`—`). Never invent values.
- **Exactly TWO documented boundary casts after this plan:** the existing scan cast (`handle as unknown as DirectoryHandleLike`) and one new app-side cast where the structural `BlobLike` from `getFile()` is known to be a real `File` (`(await clip.file.getFile()) as File`) — each with its explanatory comment. No other casts.
- **No COOP/COEP, no SharedArrayBuffer** (mediainfo.js single-thread build needs neither).
- **No interactive-CLI handoffs in this plan.** The existing .NET app at the repo root is never touched.
- **Known caveat to verify in manual QA:** MediaInfo's JSON `Duration` on the General track is treated as **seconds**. If real-file QA shows milliseconds for some container, fix the mapper + fixture in a follow-up — do not pre-guess.

---

## File Structure

```
web/packages/core/src/
  scan/handles.ts              MODIFY: add BlobLike; getFile(): Promise<BlobLike>
  metadata/model.ts            NEW: ClipMetadata (full spec §9 shape)
  metadata/mediainfo.ts        NEW: MediaInfoObjectResult types + mapMediaInfoToClipMetadata
  index.ts                     MODIFY: export new modules
web/packages/core/test/
  walker.test.ts               MODIFY: fakes return a minimal BlobLike
  scan-model.test.ts           MODIFY: same fake update
  mediainfo-map.test.ts        NEW
web/app/src/
  workers/metadata.worker.ts   NEW: Comlink-exposed mediainfo analyzer
  features/process/metadata-client.ts  NEW: worker + Comlink wrapper factory
  features/process/run-processing.ts   NEW: pool, timeout, retry, store updates
  features/scan/store.ts       MODIFY: processing phases + per-clip status/metadata
  features/scan/run-scan.ts    MODIFY: in-flight guard; remove confirmScan
  features/scan/scan-screen.tsx MODIFY: processing/processed views, metadata columns
  lib/format.ts                MODIFY: add formatDuration
```

---

## Task 1: Widen `FileHandleLike.getFile()` to `BlobLike` (core)

**Files:**
- Modify: `web/packages/core/src/scan/handles.ts`, `web/packages/core/src/index.ts`
- Modify (test fakes): `web/packages/core/test/walker.test.ts`, `web/packages/core/test/scan-model.test.ts`

**Interfaces:**
- Consumes: Plan 02's handles.
- Produces (exact):
  - `interface BlobLike { readonly size: number; slice(start?: number, end?: number): BlobLike; arrayBuffer(): Promise<ArrayBuffer> }`
  - `FileHandleLike.getFile(): Promise<BlobLike>` (was `Promise<{ readonly size: number }>`)
  - The real DOM `File`/`Blob` satisfy `BlobLike` structurally; core never names them.

- [ ] **Step 1: Update the fakes in both test files first (red)**

In `web/packages/core/test/walker.test.ts` replace the `file` helper:

```ts
import type { BlobLike } from '../src/scan/handles'

function fakeBlob(size: number): BlobLike {
  return { size, slice: () => fakeBlob(0), arrayBuffer: async () => new ArrayBuffer(0) }
}
function file(name: string, size: number): FileHandleLike {
  return { kind: 'file', name, getFile: async () => fakeBlob(size) }
}
```
(add `BlobLike` to the type import). In `web/packages/core/test/scan-model.test.ts` replace `fakeFile`:

```ts
import type { BlobLike, FileHandleLike } from '../src/scan/handles'

const fakeBlob: BlobLike = { size: 0, slice: () => fakeBlob, arrayBuffer: async () => new ArrayBuffer(0) }
const fakeFile: FileHandleLike = { kind: 'file', name: 'x', getFile: async () => fakeBlob }
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cd web/packages/core && bun test`
Expected: FAIL — `BlobLike` is not exported from `../src/scan/handles`.

- [ ] **Step 3: Implement in `web/packages/core/src/scan/handles.ts`**

Add above `FileHandleLike` and change its `getFile` return:

```ts
// Structural subset of the DOM Blob/File the pipeline relies on. mediainfo.js
// reads chunks via slice().arrayBuffer(); later plans (ffmpeg WORKERFS mount,
// WebCodecs demux) consume the same surface. Core never names DOM types.
export interface BlobLike {
  readonly size: number
  slice(start?: number, end?: number): BlobLike
  arrayBuffer(): Promise<ArrayBuffer>
}

export interface FileHandleLike {
  readonly kind: 'file'
  readonly name: string
  getFile(): Promise<BlobLike>
}
```

Add to `web/packages/core/src/index.ts`: `export type { BlobLike } from './scan/handles'` (alongside the existing handle type exports).

- [ ] **Step 4: Run tests to verify they pass**

Run: `cd web/packages/core && bun test`
Expected: PASS — all 12 existing tests green with the new fakes.

- [ ] **Step 5: Gates and commit**

From `web/`: `bun run lint && bun run typecheck && bun test && bun run build` — all exit 0 (the app compiles unchanged: its only current use of `getFile()` results is `.size` in core).

```bash
git add web/packages/core
git commit -m "feat(core): widen FileHandleLike.getFile to structural BlobLike"
```

---

## Task 2: `ClipMetadata` + mediainfo mapper (core, TDD)

**Files:**
- Create: `web/packages/core/src/metadata/model.ts`, `web/packages/core/src/metadata/mediainfo.ts`
- Modify: `web/packages/core/src/index.ts`
- Test: `web/packages/core/test/mediainfo-map.test.ts`

**Interfaces:**
- Consumes: nothing new.
- Produces (exact, used by Tasks 4–5 and later exporter plans):
  - `interface ClipMetadata { width?: number; height?: number; codec?: string; frameRate?: number; durationSeconds?: number; colorSpace?: string; startTimecode?: string; reelName?: string; camera?: string; iso?: string; whiteBalance?: string; lens?: string; focalLength?: string; aperture?: string; shutter?: string; gamma?: string }`
  - `interface MediaInfoTrack { '@type': string; [key: string]: unknown }`
  - `interface MediaInfoObjectResult { media?: { track?: MediaInfoTrack[] } }`
  - `mapMediaInfoToClipMetadata(result: MediaInfoObjectResult): ClipMetadata`

- [ ] **Step 1: Write the failing test**

Create `web/packages/core/test/mediainfo-map.test.ts`:

```ts
import { describe, expect, test } from 'bun:test'
import { mapMediaInfoToClipMetadata } from '../src/metadata/mediainfo'

const mxfLike = {
  media: {
    track: [
      { '@type': 'General', Duration: '5.000', FileSize: '104857600', Reel_Name: 'A001R2B' },
      {
        '@type': 'Video',
        Width: '1920',
        Height: '1080',
        Format: 'ProRes',
        FrameRate: '25.000',
        colour_primaries: 'BT.709',
      },
      { '@type': 'Audio', Format: 'PCM' },
      { '@type': 'Other', Type: 'Time code', TimeCode_FirstFrame: '10:20:30:00' },
    ],
  },
}

describe('mapMediaInfoToClipMetadata', () => {
  test('maps a full result (string-typed values)', () => {
    const m = mapMediaInfoToClipMetadata(mxfLike)
    expect(m.width).toBe(1920)
    expect(m.height).toBe(1080)
    expect(m.codec).toBe('ProRes')
    expect(m.frameRate).toBe(25)
    expect(m.durationSeconds).toBe(5)
    expect(m.colorSpace).toBe('BT.709')
    expect(m.startTimecode).toBe('10:20:30:00')
    expect(m.reelName).toBe('A001R2B')
  })

  test('handles number-typed values and ColorSpace fallback', () => {
    const m = mapMediaInfoToClipMetadata({
      media: {
        track: [
          { '@type': 'General', Duration: 12.5 },
          { '@type': 'Video', Width: 3840, Height: 2160, Format: 'AVC', ColorSpace: 'YUV' },
        ],
      },
    })
    expect(m.width).toBe(3840)
    expect(m.durationSeconds).toBe(12.5)
    expect(m.colorSpace).toBe('YUV')
  })

  test('missing fields stay undefined — never fabricated', () => {
    const m = mapMediaInfoToClipMetadata({})
    expect(m.width).toBeUndefined()
    expect(m.codec).toBeUndefined()
    expect(m.startTimecode).toBeUndefined()
    expect(m.reelName).toBeUndefined()
    expect(m.iso).toBeUndefined()
  })

  test('empty strings and NaN are treated as missing', () => {
    const m = mapMediaInfoToClipMetadata({
      media: { track: [{ '@type': 'Video', Width: '', Format: '  ', FrameRate: 'abc' }] },
    })
    expect(m.width).toBeUndefined()
    expect(m.codec).toBeUndefined()
    expect(m.frameRate).toBeUndefined()
  })
})
```

- [ ] **Step 2: Run to verify it fails**

Run: `cd web/packages/core && bun test mediainfo`
Expected: FAIL — module does not exist.

- [ ] **Step 3: Implement**

Create `web/packages/core/src/metadata/model.ts`:

```ts
// Spec §9. Standard fields are reliable across supported formats; camera
// fields populate only when the container carries them (§13) — the ffmpeg
// metadata-dictionary merge in a later plan fills most of the camera block.
export interface ClipMetadata {
  width?: number
  height?: number
  codec?: string
  frameRate?: number
  durationSeconds?: number
  colorSpace?: string
  startTimecode?: string
  reelName?: string
  camera?: string
  iso?: string
  whiteBalance?: string
  lens?: string
  focalLength?: string
  aperture?: string
  shutter?: string
  gamma?: string
}
```

Create `web/packages/core/src/metadata/mediainfo.ts`:

```ts
import type { ClipMetadata } from './model'

export interface MediaInfoTrack {
  '@type': string
  [key: string]: unknown
}

export interface MediaInfoObjectResult {
  media?: { track?: MediaInfoTrack[] }
}

function num(v: unknown): number | undefined {
  if (typeof v === 'number') return Number.isFinite(v) ? v : undefined
  if (typeof v === 'string' && v.trim() !== '') {
    const n = Number(v)
    return Number.isFinite(n) ? n : undefined
  }
  return undefined
}

function str(v: unknown): string | undefined {
  return typeof v === 'string' && v.trim() !== '' ? v : undefined
}

export function mapMediaInfoToClipMetadata(result: MediaInfoObjectResult): ClipMetadata {
  const tracks = result.media?.track ?? []
  const general = tracks.find((t) => t['@type'] === 'General')
  const video = tracks.find((t) => t['@type'] === 'Video')
  const timecode = tracks.find(
    (t) => t['@type'] === 'Other' && str(t.Type)?.toLowerCase() === 'time code',
  )
  return {
    width: num(video?.Width),
    height: num(video?.Height),
    codec: str(video?.Format),
    frameRate: num(video?.FrameRate),
    // MediaInfo JSON reports General.Duration in seconds — verify in manual QA.
    durationSeconds: num(general?.Duration),
    colorSpace: str(video?.colour_primaries) ?? str(video?.ColorSpace),
    startTimecode: str(timecode?.TimeCode_FirstFrame),
    reelName: str(general?.Reel_Name) ?? str(video?.Reel_Name),
  }
}
```

Add to `web/packages/core/src/index.ts`:

```ts
export type { ClipMetadata } from './metadata/model'
export { mapMediaInfoToClipMetadata } from './metadata/mediainfo'
export type { MediaInfoObjectResult, MediaInfoTrack } from './metadata/mediainfo'
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cd web/packages/core && bun test`
Expected: PASS — 16 tests total green.

- [ ] **Step 5: Gates and commit**

From `web/`: `bun run lint && bun run typecheck && bun test && bun run build` — all exit 0.

```bash
git add web/packages/core
git commit -m "feat(core): ClipMetadata model and mediainfo mapper"
```

---

## Task 3: Metadata worker + Comlink client (app)

**Files:**
- Create: `web/app/src/workers/metadata.worker.ts`, `web/app/src/features/process/metadata-client.ts`
- Modify: `web/app/package.json` + `web/bun.lock` (via bun add)

**Interfaces:**
- Consumes: nothing from the store (pure infrastructure).
- Produces (exact, used by Task 4):
  - `interface MetadataWorkerApi { analyze(file: File): Promise<unknown> }` — returns the RAW mediainfo object result (structured-clone-safe); mapping happens on the main thread.
  - `interface MetadataWorkerHandle { api: Comlink.Remote<MetadataWorkerApi>; worker: Worker }`
  - `createMetadataWorker(): MetadataWorkerHandle`

- [ ] **Step 1: Add dependencies (latest, via bun)**

Run: `cd web/app && bun add mediainfo.js comlink`
Expected: both land in `dependencies` with bun-resolved versions (both ship their own TypeScript types).

- [ ] **Step 2: Create `web/app/src/workers/metadata.worker.ts`**

```ts
/// <reference lib="webworker" />
import * as Comlink from 'comlink'
import mediaInfoFactory, { type MediaInfo } from 'mediainfo.js'
import wasmUrl from 'mediainfo.js/MediaInfoModule.wasm?url'

// One MediaInfo instance per worker, reused sequentially — the pool assigns
// one clip at a time per worker, so no concurrent analyzeData on an instance.
let miPromise: Promise<MediaInfo<'object'>> | null = null

function getMediaInfo(): Promise<MediaInfo<'object'>> {
  miPromise ??= mediaInfoFactory({
    format: 'object',
    locateFile: (path: string) => (path.endsWith('.wasm') ? wasmUrl : path),
  })
  return miPromise
}

const api = {
  async analyze(file: File): Promise<unknown> {
    const mediainfo = await getMediaInfo()
    const readChunk = async (chunkSize: number, offset: number) =>
      new Uint8Array(await file.slice(offset, offset + chunkSize).arrayBuffer())
    return await mediainfo.analyzeData(file.size, readChunk)
  },
}

Comlink.expose(api)
```

- [ ] **Step 3: Create `web/app/src/features/process/metadata-client.ts`**

```ts
import * as Comlink from 'comlink'

export interface MetadataWorkerApi {
  analyze(file: File): Promise<unknown>
}

export interface MetadataWorkerHandle {
  api: Comlink.Remote<MetadataWorkerApi>
  worker: Worker
}

export function createMetadataWorker(): MetadataWorkerHandle {
  const worker = new Worker(new URL('../../workers/metadata.worker.ts', import.meta.url), {
    type: 'module',
  })
  return { api: Comlink.wrap<MetadataWorkerApi>(worker), worker }
}
```

- [ ] **Step 4: Verify the worker bundles**

Run from `web/`: `bun run lint && bun run typecheck && bun test && bun run build` — all exit 0. Confirm the build emitted a separate worker chunk AND the wasm asset: `ls web/app/dist/assets/ | grep -iE "worker|wasm"` — expect a `metadata.worker-*.js` (or similar) and a `MediaInfoModule-*.wasm`. If Vite errors on the `?url` wasm import inside the worker, report the exact error as a concern rather than improvising a config change.

- [ ] **Step 5: Commit**

```bash
git add web/app web/bun.lock
git commit -m "feat(app): mediainfo metadata worker with comlink client"
```

---

## Task 4: Worker pool, processing orchestration, store extensions

**Files:**
- Create: `web/app/src/features/process/run-processing.ts`
- Modify: `web/app/src/features/scan/store.ts` (full replacement below), `web/app/src/features/scan/run-scan.ts`

**Interfaces:**
- Consumes: Task 2's mapper, Task 3's `createMetadataWorker`.
- Produces (exact, used by Task 5):
  - Store: `ScanPhase = 'idle' | 'scanning' | 'summary' | 'processing' | 'processed' | 'error'`; new state fields `clipStatus: Record<string, ClipProcessStatus>`, `metadataById: Record<string, ClipMetadata>`, `clipErrors: Record<string, string>`, `processedCount: number`; `type ClipProcessStatus = 'queued' | 'processing' | 'done' | 'failed'`.
  - `startProcessing(): Promise<void>` — replaces `confirmScan` (which is removed).
  - `run-scan.ts` gains an in-flight guard: `scanFrom` returns immediately when phase is `'scanning'` or `'processing'`.

- [ ] **Step 1: Replace `web/app/src/features/scan/store.ts`**

```ts
import type { ClipMetadata, ClipRef, RawNotice, ScanSummary } from '@luna-web/core'
import { Store } from '@tanstack/store'

export type ScanPhase = 'idle' | 'scanning' | 'summary' | 'processing' | 'processed' | 'error'

export type ClipProcessStatus = 'queued' | 'processing' | 'done' | 'failed'

export interface ScanState {
  phase: ScanPhase
  sourceName: string | null
  progress: { filesSeen: number; clipsFound: number } | null
  clips: ClipRef[]
  raw: RawNotice[]
  summary: ScanSummary | null
  error: string | null
  clipStatus: Record<string, ClipProcessStatus>
  metadataById: Record<string, ClipMetadata>
  clipErrors: Record<string, string>
  processedCount: number
}

export const initialScanState: ScanState = {
  phase: 'idle',
  sourceName: null,
  progress: null,
  clips: [],
  raw: [],
  summary: null,
  error: null,
  clipStatus: {},
  metadataById: {},
  clipErrors: {},
  processedCount: 0,
}

export const scanStore = new Store<ScanState>(initialScanState)
```

- [ ] **Step 2: Create `web/app/src/features/process/run-processing.ts`**

```ts
import { type ClipRef, mapMediaInfoToClipMetadata, type MediaInfoObjectResult } from '@luna-web/core'
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
        const clip = clips[index] as ClipRef
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
  return mapMediaInfoToClipMetadata(raw as MediaInfoObjectResult)
}

function withTimeout<T>(promise: Promise<T>, ms: number, label: string): Promise<T> {
  return new Promise<T>((resolve, reject) => {
    const timer = setTimeout(
      () => reject(new Error(`${label}: metadata extraction timed out after ${Math.round(ms / 1000)}s`)),
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
```

- [ ] **Step 3: Update `web/app/src/features/scan/run-scan.ts`**

Two changes:
1. Add the in-flight guard at the top of `scanFrom` (before the permission check):
```ts
const phase = scanStore.state.phase
if (phase === 'scanning' || phase === 'processing') return
```
2. Delete the `confirmScan` function entirely (Task 5 rewires the button to `startProcessing`). Leave `pickAndScan`, `resetScan`, and the rest untouched.

- [ ] **Step 4: Gates**

From `web/`: `bun run lint && bun run typecheck && bun test` — lint/test green; **typecheck/build are expected to FAIL right now** because `scan-screen.tsx` still imports `confirmScan`. That is the seam Task 5 closes — so for THIS task's commit, verify instead with: `cd web/packages/core && bun test` green plus `bunx @biomejs/biome check web/app/src` clean, and commit Tasks 4+5 knowing full gates run at Task 5. **If you prefer atomic green commits (recommended): implement Task 5's scan-screen change in this same working tree and commit both tasks' files together at Task 5's step — in that case skip this task's commit.**

- [ ] **Step 5: Commit (only if you kept the tree green by folding in Task 5 — otherwise skip; see Step 4)**

```bash
git add web/app/src
git commit -m "feat(app): metadata worker pool and processing orchestration"
```

---

## Task 5: Processing/processed UI with metadata columns

**Files:**
- Modify: `web/app/src/features/scan/scan-screen.tsx` (full replacement below), `web/app/src/lib/format.ts`

**Interfaces:**
- Consumes: Task 4's store shape + `startProcessing`, `formatBytes`/`formatDuration`.
- Produces: the complete phase-driven screen; `formatDuration(seconds: number): string` (`h:mm:ss` or `m:ss`).

- [ ] **Step 1: Add `formatDuration` to `web/app/src/lib/format.ts`**

```ts
export function formatDuration(seconds: number): string {
  const total = Math.round(seconds)
  const h = Math.floor(total / 3600)
  const m = Math.floor((total % 3600) / 60)
  const s = total % 60
  const mm = String(m).padStart(2, '0')
  const ss = String(s).padStart(2, '0')
  return h > 0 ? `${h}:${mm}:${ss}` : `${m}:${ss}`
}
```

- [ ] **Step 2: Replace `web/app/src/features/scan/scan-screen.tsx`**

```tsx
import type { ClipRef } from '@luna-web/core'
import { useStore } from '@tanstack/react-store'
import { Button } from '@/components/ui/button'
import { startProcessing } from '@/features/process/run-processing'
import { formatBytes, formatDuration } from '@/lib/format'
import { RecentList } from './recent-list'
import { pickAndScan, resetScan } from './run-scan'
import { type ScanState, scanStore } from './store'

export function ScanScreen() {
  const state = useStore(scanStore)

  return (
    <main className="mx-auto flex min-h-screen max-w-4xl flex-col items-center justify-center gap-6 px-6 py-12">
      <h1 className="font-heading text-3xl font-semibold">Luna Web</h1>

      {state.phase === 'idle' && (
        <>
          <p className="text-muted-foreground">Pick a footage folder — everything stays on this device.</p>
          <Button onClick={() => void pickAndScan()}>Pick folder</Button>
          <RecentList />
        </>
      )}

      {state.phase === 'scanning' && (
        <p className="text-muted-foreground" aria-live="polite">
          Scanning {state.sourceName}…{' '}
          {state.progress ? `${state.progress.filesSeen} files, ${state.progress.clipsFound} clips` : ''}
        </p>
      )}

      {state.phase === 'summary' && state.summary && (
        <section className="w-full rounded-lg border p-6">
          <h2 className="mb-4 text-xl font-medium">{state.sourceName}</h2>
          <dl className="mb-4 grid grid-cols-3 gap-4 text-center">
            <div>
              <dt className="text-muted-foreground text-sm">Clips</dt>
              <dd className="text-2xl">{state.summary.clipCount}</dd>
            </div>
            <div>
              <dt className="text-muted-foreground text-sm">Total size</dt>
              <dd className="text-2xl">{formatBytes(state.summary.totalClipSizeBytes)}</dd>
            </div>
            <div>
              <dt className="text-muted-foreground text-sm">RAW (unsupported)</dt>
              <dd className="text-2xl">{state.summary.rawCount}</dd>
            </div>
          </dl>
          {state.summary.rawCount > 0 && (
            <p className="text-muted-foreground mb-4 text-sm">
              {state.summary.rawCount} ARRIRAW/R3D/BRAW file(s) were detected but cannot be decoded in a
              browser — they will be listed without thumbnails.
            </p>
          )}
          <div className="flex gap-3">
            <Button onClick={() => void startProcessing()}>Process {state.summary.clipCount} clips</Button>
            <Button variant="outline" onClick={resetScan}>
              Cancel
            </Button>
          </div>
        </section>
      )}

      {(state.phase === 'processing' || state.phase === 'processed') && (
        <section className="w-full">
          <div className="mb-4 flex items-center justify-between">
            <h2 className="text-xl font-medium" aria-live="polite">
              {state.phase === 'processing'
                ? `Reading metadata… ${state.processedCount}/${state.clips.length}`
                : `${state.clips.length} clips processed`}
            </h2>
            <Button variant="outline" onClick={resetScan}>
              Start over
            </Button>
          </div>
          <ClipTable state={state} />
          {state.raw.length > 0 && (
            <section className="mt-6">
              <h3 className="text-muted-foreground mb-2 text-sm font-medium">
                RAW files (not decodable in browser)
              </h3>
              <ul className="divide-y rounded-lg border">
                {state.raw.map((r) => (
                  <li key={r.id} className="text-muted-foreground flex items-center justify-between px-4 py-2 text-sm">
                    <span className="truncate">{r.relativePath}</span>
                    <span className="ml-4 shrink-0">RAW · {formatBytes(r.sizeBytes)}</span>
                  </li>
                ))}
              </ul>
            </section>
          )}
          {state.phase === 'processed' && (
            <p className="text-muted-foreground mt-3 text-sm">Thumbnails arrive in the next milestone.</p>
          )}
        </section>
      )}

      {state.phase === 'error' && (
        <section className="text-center">
          <p className="text-destructive mb-4">{state.error}</p>
          <Button variant="outline" onClick={resetScan}>
            Back
          </Button>
        </section>
      )}
    </main>
  )
}

function ClipTable({ state }: { state: ScanState }) {
  return (
    <ul className="divide-y rounded-lg border">
      {state.clips.map((c) => (
        <ClipRow key={c.id} clip={c} state={state} />
      ))}
    </ul>
  )
}

function ClipRow({ clip, state }: { clip: ClipRef; state: ScanState }) {
  const status = state.clipStatus[clip.id] ?? 'queued'
  const m = state.metadataById[clip.id]
  return (
    <li className="grid grid-cols-[1fr_auto_auto_auto_auto_auto] items-center gap-x-4 px-4 py-2 text-sm">
      <span className="truncate">{clip.relativePath}</span>
      <span className="text-muted-foreground tabular-nums">
        {m?.width && m?.height ? `${m.width}×${m.height}` : '—'}
      </span>
      <span className="text-muted-foreground">{m?.codec ?? '—'}</span>
      <span className="text-muted-foreground tabular-nums">
        {m?.frameRate !== undefined ? `${m.frameRate} fps` : '—'}
      </span>
      <span className="text-muted-foreground tabular-nums">
        {m?.durationSeconds !== undefined ? formatDuration(m.durationSeconds) : '—'}
      </span>
      <span className="text-muted-foreground tabular-nums">
        {status === 'done' ? formatBytes(clip.sizeBytes) : <StatusBadge status={status} error={state.clipErrors[clip.id]} />}
      </span>
    </li>
  )
}

function StatusBadge({ status, error }: { status: string; error?: string }) {
  if (status === 'failed') {
    return (
      <span className="text-destructive" title={error}>
        failed
      </span>
    )
  }
  return <span className="text-muted-foreground">{status}…</span>
}
```

- [ ] **Step 3: Full gates (this is the seam-closing commit)**

From `web/`: `bun run lint && bun run typecheck && bun test && bun run build` — ALL exit 0.

- [ ] **Step 4: Commit**

```bash
git add web/app/src
git commit -m "feat(app): live metadata processing view"
```
(If Task 4's commit was skipped per its Step 4 note, this commit carries both tasks' files — use message `feat(app): metadata worker pool, processing orchestration, and live view`.)

---

## Definition of done (this plan)

- `bun test` green (16 core tests: extensions, scan model, walker, mediainfo mapper).
- All gates green from `web/`; the production build emits the worker chunk + `MediaInfoModule-*.wasm` asset.
- Commits per task (Tasks 4+5 may be one combined green commit).
- **Manual QA (maintainer, non-blocking, Chromium, `bun --filter '@luna-web/app' dev`):**
  1. Scan a folder → Process → rows fill in live with resolution/codec/fps/duration; counter advances n/total.
  2. **Duration sanity check (the flagged caveat):** a clip you know (e.g. ~10 s) shows ~0:10, not 2:46:40 (which would mean ms→seconds misread). Check an MP4 and an MXF if available.
  3. Timecode/reel: an MXF with embedded timecode shows a plausible start TC (visible in a later plan's detail view; for now confirm no crash).
  4. A corrupt/truncated file → its row shows "failed" with a tooltip, everything else completes.
  5. Start over mid-processing → returns to idle cleanly; re-scan works (in-flight guard: clicking a recent folder during processing does nothing).
  6. DevTools → Network: after first load, no network requests during processing (wasm served from cache/bundle; footage never leaves).

## Self-review notes

- **Spec coverage:** §8.3 pool (≤4, hardwareConcurrency-capped, Comlink, per-clip lifecycle, crash isolation + retry-once §15) ✓; §8.6 metadata via mediainfo.js chunked reads, mapping pure in core ✓; §9 ClipMetadata full shape (camera fields present, unpopulated until the ffmpeg-dict merge in the decode plan, per §13 no fabrication) ✓; §10.3 5-min timeout ✓. Deferred: ffmpeg metadata-dict merge + precedence (§13) to the decode plan; TanStack Table grid to Plan 06; stale-source UX (§15 polish) to Plan 08.
- **Plan 02 carry-forwards addressed:** BlobLike widening (Task 1) ✓; scanFrom in-flight guard (Task 4) ✓.
- **Placeholders:** none.
- **Type consistency:** `ClipProcessStatus`/`clipStatus`/`metadataById`/`clipErrors`/`processedCount` consistent across Tasks 4–5; `MetadataWorkerApi`/`MetadataWorkerHandle` across 3–4; `BlobLike` across 1 and 4 (the cast site).
- **Honest seam:** Task 4 alone breaks typecheck (removes `confirmScan` that the old screen imports); the plan explicitly allows folding Tasks 4+5 into one green commit rather than committing red.
