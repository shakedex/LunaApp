# Luna Web — Plan 04: Thumbnails Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** After metadata completes, every clip gets 3 thumbnails (10% / 50% / 90%, 1280 px wide) decoded locally — mediabunny (+TurboRes ProRes) as the fast primary path, ffmpeg.wasm as the fallback for MXF/DNxHD/legacy, with the desktop's cascade-on-failure semantics.

**Architecture:** A generic lane pool (`runPool`) is extracted into `packages/core` (pure, bun-tested) carrying the run-token cancellation and retry-once-on-fresh-lane semantics Plan 03 proved out; the metadata pass refactors onto it, and the new thumbnail pass reuses it. Thumbnails run as a **second pass** after metadata (phase `processing` → `thumbnailing` → `processed`), tracked per-clip in a `thumbStatus` sibling record. The router sends `.mp4/.m4v/.mov/.mkv/.webm/.3gp` to a mediabunny worker pool (WebCodecs + registered ProRes decoder, `CanvasSink` extraction) and `.mxf/.avi/.mts/.m2ts/.wmv/.flv` to a single-lane ffmpeg.wasm queue (lazy engine load from jsDelivr via Cache API, `WORKERFS` mount); a mediabunny container/codec failure re-queues the clip onto the ffmpeg queue before it is marked failed.

**Tech Stack (additions):** `mediabunny`, `@mediabunny/prores`, `@ffmpeg/ffmpeg`, `@ffmpeg/util` (+ `@ffmpeg/core` as devDependency solely so the CDN URL version is bun-managed) — all via `bun add` at latest.

**Spec:** `docs/superpowers/specs/2026-07-17-luna-web-design.md` §8.4 (router), §8.5 (encoder), §9 (`ThumbnailFrame`/`ThumbnailOutcome`), §10.2 (router decision + cascade), §10.3 (parameters/timeouts), §10.4 (seeking), §11 (engine loading), §15 (isolation/retry).

## Global Constraints

- **Never hand-write a dependency version.** All deps via `bun add` (latest). The one CDN URL (ffmpeg core) derives its version from the bun-installed `@ffmpeg/core` package — not a hand-typed string (Step noted in Task 5; documented fallback if the version import is blocked).
- **`packages/core` stays DOM-free.** `runPool` is pure async logic; `ThumbnailFrame` is generic over the image payload (`TImage = unknown`) so core never names `Blob`.
- **Tests:** `bun test`, core only (`runPool`, router, timestamp math). Workers/UI are maintainer-QA.
- **Gates green after every task** (from `web/`): `bun run lint && bun run typecheck && bun test && bun run build` — all exit 0 before each commit.
- **No COOP/COEP, no SharedArrayBuffer.** `@mediabunny/prores` runs in its degraded-threading mode; that is sufficient for 3 frames/clip.
- **Boundary casts:** the two existing documented casts stay; this plan may add at most ONE more documented cast if a concrete API boundary requires it — every cast carries a comment, and the final task audits the count.
- **Run-token discipline:** all store writes from pool passes go through the guarded updater; `resetScan` cancellation must stop both passes.
- **Timeouts (spec §10.3):** mediabunny thumbnails 60 s/clip; ffmpeg thumbnails 180 s/clip; metadata unchanged at 5 min.
- **Fidelity:** a failed/missing thumbnail renders a placeholder — never a fabricated image; RAW clips are `NotAttempted` by definition.
- **No interactive-CLI handoffs.** The .NET app at the repo root is never touched.

---

## File Structure

```
web/packages/core/src/
  pool/run-pool.ts             NEW: generic lane pool (cancellation, retry, hooks)
  thumbs/model.ts              NEW: ThumbnailOutcome, ThumbnailFrame<TImage>, thumbnailTimestamps
  thumbs/router.ts             NEW: decodePathFor(extension) → 'mediabunny' | 'ffmpeg' | 'none'
  index.ts                     MODIFY: export new modules
web/packages/core/test/
  run-pool.test.ts             NEW
  thumbs.test.ts               NEW
web/app/src/
  workers/thumbs.worker.ts     NEW: mediabunny + @mediabunny/prores, CanvasSink → WebP blobs
  features/process/thumbs-client.ts    NEW: worker + Comlink wrapper factory
  features/process/ffmpeg-engine.ts    NEW: lazy cached ffmpeg.wasm client (main thread; its own internal worker)
  features/process/run-processing.ts   MODIFY: refactor onto core runPool; chain thumbnail pass
  features/process/run-thumbnails.ts   NEW: second-pass orchestration (two queues, cascade)
  features/scan/store.ts       MODIFY: 'thumbnailing' phase + thumbStatus/thumbsById/thumbErrors/thumbDoneCount
  features/scan/scan-screen.tsx MODIFY: thumbnail strips, thumbnailing progress
  lib/engine-cache.ts          NEW: Cache API fetch → blob URL for the ffmpeg core
```

---

## Task 1: Core `runPool` (TDD) — the extracted lane pool

**Files:**
- Create: `web/packages/core/src/pool/run-pool.ts`
- Modify: `web/packages/core/src/index.ts`
- Test: `web/packages/core/test/run-pool.test.ts`

**Interfaces:**
- Consumes: nothing.
- Produces (exact, used by Tasks 3 and 6):

```ts
export interface PoolHooks<L, T, R> {
  createLane(): L | Promise<L>
  destroyLane(lane: L): void
  run(lane: L, item: T): Promise<R>
}
export interface PoolHandlers<T, R> {
  onItemStart?(item: T): void
  onItemSuccess(item: T, result: R): void
  onItemFailure(item: T, error: unknown): void
}
export interface PoolOptions {
  concurrency: number
  maxAttempts?: number // default 2 (i.e. one retry on a fresh lane)
  isCancelled?(): boolean
}
export async function runPool<L, T, R>(
  items: readonly T[],
  hooks: PoolHooks<L, T, R>,
  handlers: PoolHandlers<T, R>,
  options: PoolOptions,
): Promise<void>
```

Semantics (these ARE the test cases): lanes = `min(concurrency, items.length)`, zero items → resolves without creating lanes; shared index claimed synchronously (no double-processing); `isCancelled()` checked before each claim — cancelled loops stop claiming and destroy their lane; on `run` rejection the lane is destroyed and a FRESH lane created for the retry of the SAME item; after `maxAttempts` failures `onItemFailure` fires once and the pool moves on; every created lane is destroyed exactly once on every path (success/failure/cancel); a `createLane` rejection propagates (rejects `runPool` — callers wrap it).

- [ ] **Step 1: Write the failing test**

Create `web/packages/core/test/run-pool.test.ts`:

```ts
import { describe, expect, test } from 'bun:test'
import { runPool } from '../src/pool/run-pool'

interface Lane {
  id: number
}

function makeHooks(behavior: (lane: Lane, item: string, attemptCounts: Map<string, number>) => Promise<string>) {
  let laneSeq = 0
  const created: Lane[] = []
  const destroyed: Lane[] = []
  const attempts = new Map<string, number>()
  return {
    created,
    destroyed,
    attempts,
    hooks: {
      createLane: () => {
        const lane = { id: laneSeq++ }
        created.push(lane)
        return lane
      },
      destroyLane: (lane: Lane) => {
        destroyed.push(lane)
      },
      run: (lane: Lane, item: string) => {
        attempts.set(item, (attempts.get(item) ?? 0) + 1)
        return behavior(lane, item, attempts)
      },
    },
  }
}

describe('runPool', () => {
  test('processes every item exactly once and destroys all lanes', async () => {
    const t = makeHooks(async (_l, item) => `ok:${item}`)
    const ok: string[] = []
    await runPool(
      ['a', 'b', 'c', 'd', 'e'],
      t.hooks,
      { onItemSuccess: (_i, r) => ok.push(r), onItemFailure: () => {} },
      { concurrency: 2 },
    )
    expect(ok.sort()).toEqual(['ok:a', 'ok:b', 'ok:c', 'ok:d', 'ok:e'])
    expect(t.created.length).toBe(2)
    expect(t.destroyed.length).toBe(t.created.length)
  })

  test('zero items: resolves without creating lanes', async () => {
    const t = makeHooks(async () => 'never')
    await runPool([], t.hooks, { onItemSuccess: () => {}, onItemFailure: () => {} }, { concurrency: 4 })
    expect(t.created.length).toBe(0)
  })

  test('retries once on a FRESH lane, then succeeds', async () => {
    const t = makeHooks(async (_l, item, attempts) => {
      if (item === 'b' && attempts.get('b') === 1) throw new Error('flaky')
      return `ok:${item}`
    })
    const ok: string[] = []
    const failed: string[] = []
    await runPool(
      ['a', 'b'],
      t.hooks,
      { onItemSuccess: (_i, r) => ok.push(r), onItemFailure: (i) => failed.push(i) },
      { concurrency: 1 },
    )
    expect(ok.sort()).toEqual(['ok:a', 'ok:b'])
    expect(failed).toEqual([])
    expect(t.attempts.get('b')).toBe(2)
    // failure destroyed the lane and a fresh one was created for the retry
    expect(t.created.length).toBe(2)
    expect(t.destroyed.length).toBe(2)
  })

  test('marks item failed after maxAttempts and continues with later items', async () => {
    const t = makeHooks(async (_l, item) => {
      if (item === 'bad') throw new Error('always broken')
      return `ok:${item}`
    })
    const ok: string[] = []
    const failed: Array<[string, string]> = []
    await runPool(
      ['bad', 'good'],
      t.hooks,
      {
        onItemSuccess: (_i, r) => ok.push(r),
        onItemFailure: (i, e) => failed.push([i, e instanceof Error ? e.message : String(e)]),
      },
      { concurrency: 1, maxAttempts: 2 },
    )
    expect(failed).toEqual([['bad', 'always broken']])
    expect(ok).toEqual(['ok:good'])
    expect(t.attempts.get('bad')).toBe(2)
    expect(t.destroyed.length).toBe(t.created.length)
  })

  test('cancellation stops claiming new items', async () => {
    let cancelled = false
    const t = makeHooks(async (_l, item) => {
      if (item === 'a') cancelled = true // cancel after the first item starts
      return `ok:${item}`
    })
    const ok: string[] = []
    await runPool(
      ['a', 'b', 'c'],
      t.hooks,
      { onItemSuccess: (_i, r) => ok.push(r), onItemFailure: () => {} },
      { concurrency: 1, isCancelled: () => cancelled },
    )
    expect(ok).toEqual(['ok:a']) // b and c never claimed
    expect(t.destroyed.length).toBe(t.created.length)
  })

  test('createLane rejection propagates to the caller', async () => {
    const hooks = {
      createLane: () => {
        throw new Error('no lanes today')
      },
      destroyLane: () => {},
      run: async () => 'never',
    }
    await expect(
      runPool(['a'], hooks, { onItemSuccess: () => {}, onItemFailure: () => {} }, { concurrency: 1 }),
    ).rejects.toThrow('no lanes today')
  })
})
```

- [ ] **Step 2: Run to verify it fails**

Run: `cd web/packages/core && bun test run-pool`
Expected: FAIL — module does not exist.

- [ ] **Step 3: Implement `web/packages/core/src/pool/run-pool.ts`**

```ts
// Generic lane pool extracted from the metadata orchestration (Plan 03).
// A "lane" is a reusable resource (e.g. a worker) owned by one concurrent
// loop. Failure recycles the lane (destroy + fresh create) and retries the
// same item up to maxAttempts. Cancellation is cooperative: checked before
// each claim; in-flight work finishes and its result is delivered to the
// handlers (callers drop late writes via their own guards).
export interface PoolHooks<L, T, R> {
  createLane(): L | Promise<L>
  destroyLane(lane: L): void
  run(lane: L, item: T): Promise<R>
}

export interface PoolHandlers<T, R> {
  onItemStart?(item: T): void
  onItemSuccess(item: T, result: R): void
  onItemFailure(item: T, error: unknown): void
}

export interface PoolOptions {
  concurrency: number
  maxAttempts?: number
  isCancelled?(): boolean
}

export async function runPool<L, T, R>(
  items: readonly T[],
  hooks: PoolHooks<L, T, R>,
  handlers: PoolHandlers<T, R>,
  options: PoolOptions,
): Promise<void> {
  if (items.length === 0) return
  const maxAttempts = options.maxAttempts ?? 2
  const isCancelled = options.isCancelled ?? (() => false)
  const laneCount = Math.max(1, Math.min(options.concurrency, items.length))
  let nextIndex = 0

  async function laneLoop(): Promise<void> {
    let lane: L | null = null
    try {
      lane = await hooks.createLane()
      for (;;) {
        if (isCancelled()) return
        const index = nextIndex++
        if (index >= items.length) return
        const item = items[index] as T
        handlers.onItemStart?.(item)
        let attempt = 0
        for (;;) {
          // Lane creation stays OUTSIDE the try: a createLane failure is a
          // pool-level fault and must propagate, never masquerade as an item
          // failure. (Fixed in review — commit ac2a8c2 added the regression
          // tests: createLane-during-retry propagation + concurrency clamp.)
          if (lane === null) lane = await hooks.createLane()
          try {
            const result = await hooks.run(lane, item)
            handlers.onItemSuccess(item, result)
            break
          } catch (err) {
            hooks.destroyLane(lane)
            lane = null
            attempt += 1
            if (attempt >= maxAttempts) {
              handlers.onItemFailure(item, err)
              break
            }
          }
        }
      }
    } finally {
      if (lane !== null) hooks.destroyLane(lane)
    }
  }

  await Promise.all(Array.from({ length: laneCount }, () => laneLoop()))
}
```

Add to `web/packages/core/src/index.ts`:

```ts
export { runPool } from './pool/run-pool'
export type { PoolHandlers, PoolHooks, PoolOptions } from './pool/run-pool'
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cd web/packages/core && bun test`
Expected: PASS — all suites green (16 prior + 6 new).

- [ ] **Step 5: Gates and commit**

From `web/`: `bun run lint && bun run typecheck && bun test && bun run build` — all exit 0.

```bash
git add web/packages/core
git commit -m "feat(core): generic lane pool with cancellation and retry"
```

---

## Task 2: Core thumbnail model + router (TDD)

**Files:**
- Create: `web/packages/core/src/thumbs/model.ts`, `web/packages/core/src/thumbs/router.ts`
- Modify: `web/packages/core/src/index.ts`
- Test: `web/packages/core/test/thumbs.test.ts`

**Interfaces:**
- Consumes: `SUPPORTED_MEDIA_EXTENSIONS` conventions from Plan 02.
- Produces (exact, used by Tasks 4–6):

```ts
export type ThumbnailOutcome =
  | 'Success' | 'NoDecoder' | 'SeekFailed'
  | 'DecodeFailed' | 'ContainerOpenFailed' | 'NotAttempted'

export interface ThumbnailFrame<TImage = unknown> {
  positionRatio: number       // 0.1 | 0.5 | 0.9
  timestampSeconds: number
  image?: TImage              // app instantiates ThumbnailFrame<Blob>
  mime?: string               // 'image/webp' (mediabunny) | 'image/jpeg' (ffmpeg)
  outcome: ThumbnailOutcome
}

export const THUMBNAIL_POSITIONS: readonly number[] // [0.1, 0.5, 0.9]
export function thumbnailTimestamps(durationSeconds: number): number[]
// duration <= 0 or not finite → [0]; else positions mapped, clamped to [0, duration)

export type DecodePath = 'mediabunny' | 'ffmpeg' | 'none'
export function decodePathFor(extension: string): DecodePath
// .mp4 .m4v .mov .mkv .webm .3gp → 'mediabunny'
// .mxf .avi .mts .m2ts .wmv .flv → 'ffmpeg'
// .r3d .braw .ari and anything else → 'none'
```

- [ ] **Step 1: Write the failing test**

Create `web/packages/core/test/thumbs.test.ts`:

```ts
import { describe, expect, test } from 'bun:test'
import { THUMBNAIL_POSITIONS, thumbnailTimestamps } from '../src/thumbs/model'
import { decodePathFor } from '../src/thumbs/router'

describe('thumbnailTimestamps', () => {
  test('maps 10/50/90% of duration', () => {
    expect(THUMBNAIL_POSITIONS).toEqual([0.1, 0.5, 0.9])
    expect(thumbnailTimestamps(100)).toEqual([10, 50, 90])
  })
  test('degenerate durations collapse to a single frame at 0', () => {
    expect(thumbnailTimestamps(0)).toEqual([0])
    expect(thumbnailTimestamps(-5)).toEqual([0])
    expect(thumbnailTimestamps(Number.NaN)).toEqual([0])
  })
  test('timestamps never reach the duration itself', () => {
    for (const t of thumbnailTimestamps(1)) expect(t).toBeLessThan(1)
  })
})

describe('decodePathFor', () => {
  test('fast-path containers go to mediabunny', () => {
    for (const ext of ['.mp4', '.m4v', '.mov', '.mkv', '.webm', '.3gp']) {
      expect(decodePathFor(ext)).toBe('mediabunny')
    }
  })
  test('MXF and legacy go to ffmpeg', () => {
    for (const ext of ['.mxf', '.avi', '.mts', '.m2ts', '.wmv', '.flv']) {
      expect(decodePathFor(ext)).toBe('ffmpeg')
    }
  })
  test('RAW and unknown are never decoded', () => {
    for (const ext of ['.r3d', '.braw', '.ari', '.txt', '']) {
      expect(decodePathFor(ext)).toBe('none')
    }
  })
})
```

- [ ] **Step 2: Run to verify it fails**

Run: `cd web/packages/core && bun test thumbs`
Expected: FAIL — modules do not exist.

- [ ] **Step 3: Implement**

Create `web/packages/core/src/thumbs/model.ts`:

```ts
export type ThumbnailOutcome =
  | 'Success'
  | 'NoDecoder'
  | 'SeekFailed'
  | 'DecodeFailed'
  | 'ContainerOpenFailed'
  | 'NotAttempted'

// Generic over the image payload so core stays DOM-free; the app uses
// ThumbnailFrame<Blob>.
export interface ThumbnailFrame<TImage = unknown> {
  positionRatio: number
  timestampSeconds: number
  image?: TImage
  mime?: string
  outcome: ThumbnailOutcome
}

export const THUMBNAIL_POSITIONS: readonly number[] = [0.1, 0.5, 0.9]

export function thumbnailTimestamps(durationSeconds: number): number[] {
  if (!Number.isFinite(durationSeconds) || durationSeconds <= 0) return [0]
  return THUMBNAIL_POSITIONS.map((p) => Math.min(p * durationSeconds, durationSeconds * 0.999))
}
```

Create `web/packages/core/src/thumbs/router.ts`:

```ts
export type DecodePath = 'mediabunny' | 'ffmpeg' | 'none'

const MEDIABUNNY_EXTENSIONS = new Set(['.mp4', '.m4v', '.mov', '.mkv', '.webm', '.3gp'])
// .mts/.m2ts default to ffmpeg: AVCHD's 192-byte BDAV TS variant is
// unverified in mediabunny (spec §10.2). Promote after real-file QA.
const FFMPEG_EXTENSIONS = new Set(['.mxf', '.avi', '.mts', '.m2ts', '.wmv', '.flv'])

export function decodePathFor(extension: string): DecodePath {
  if (MEDIABUNNY_EXTENSIONS.has(extension)) return 'mediabunny'
  if (FFMPEG_EXTENSIONS.has(extension)) return 'ffmpeg'
  return 'none'
}
```

Add to `web/packages/core/src/index.ts`:

```ts
export { THUMBNAIL_POSITIONS, thumbnailTimestamps } from './thumbs/model'
export type { ThumbnailFrame, ThumbnailOutcome } from './thumbs/model'
export { decodePathFor } from './thumbs/router'
export type { DecodePath } from './thumbs/router'
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cd web/packages/core && bun test`
Expected: PASS.

- [ ] **Step 5: Gates and commit**

From `web/`: all four gates exit 0.

```bash
git add web/packages/core
git commit -m "feat(core): thumbnail model, positions, and decode-path router"
```

---

## Task 3: Refactor the metadata pass onto core `runPool`

**Files:**
- Modify: `web/app/src/features/process/run-processing.ts`

**Interfaces:**
- Consumes: core `runPool`; existing store/worker client.
- Produces: identical external behavior (`startProcessing`, `cancelProcessing`), now implemented on `runPool`; the run-token/`updateRun` guard generalized so the thumbnail pass (Task 6) can share it. Exports `POOL_CAP`, `currentRunToken()` accessor is NOT exported — instead export `isRunCurrent(run: number): boolean` and `guardedUpdate(run: number, updater: (s: ScanState) => ScanState): void` for run-thumbnails.ts to reuse.

- [ ] **Step 1: Refactor `run-processing.ts`**

Replace the hand-rolled loops with `runPool`, preserving every behavior. The resulting file:

```ts
import {
  type ClipRef,
  mapMediaInfoToClipMetadata,
  runPool,
} from '@luna-web/core'
import { type ScanState, scanStore } from '../scan/store'
import { createMetadataWorker, type MetadataWorkerHandle } from './metadata-client'
import { startThumbnails } from './run-thumbnails'

export const POOL_CAP = 4
const METADATA_TIMEOUT_MS = 5 * 60_000 // spec §10.3

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
  return Math.max(1, Math.min(POOL_CAP, navigator.hardwareConcurrency || 2, itemCount))
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
    thumbStatus: {},
    thumbsById: {},
    thumbErrors: {},
    thumbDoneCount: 0,
  }))

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
          const message = err instanceof Error ? err.message : String(err)
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
    const message = err instanceof Error ? err.message : String(err)
    scanStore.setState((s) =>
      s.phase === 'processing' ? { ...s, phase: 'error', error: message } : s,
    )
    return
  }

  if (run !== currentRun) return
  await startThumbnails(run)
}

async function analyzeClip(handle: MetadataWorkerHandle, clip: ClipRef) {
  // App-side boundary: core types getFile() as structural BlobLike; here the
  // handle came from the real File System Access API, so this is a real File
  // (structured-cloneable into the worker).
  const file = (await clip.file.getFile()) as File
  const raw = await withTimeout(handle.api.analyze(file), METADATA_TIMEOUT_MS, clip.fileName)
  return mapMediaInfoToClipMetadata(raw)
}
```

Notes for the implementer: `startThumbnails` does not exist until Task 6 — create a placeholder `run-thumbnails.ts` in THIS task exporting `export async function startThumbnails(run: number): Promise<void>` whose body transitions phase `processing → processed` via `guardedUpdate`-compatible logic (exactly: `guardedUpdate(run, (s) => ({ ...s, phase: 'processed' }))` — but note `guardedUpdate` only applies while phase is active, and 'processing' is active, so this works). Task 6 replaces the placeholder body. The store fields (`thumbStatus` etc.) are added in Task 6's store change — for THIS task, keep the store as-is and OMIT the four thumb fields from the reset block (add them in Task 6). Adjust accordingly so this task compiles green on its own.

- [ ] **Step 2: Gates**

From `web/`: all four gates exit 0. Manual sanity: metadata flow behaves exactly as before (scan → process → rows fill → processed).

- [ ] **Step 3: Commit**

```bash
git add web/app/src
git commit -m "refactor(app): metadata pass on core runPool with shared run guards"
```

---

## Task 4: mediabunny thumbnail worker + client

**Files:**
- Create: `web/app/src/workers/thumbs.worker.ts`, `web/app/src/features/process/thumbs-client.ts`
- Modify: `web/app/package.json` + `web/bun.lock` (via bun add)

**Interfaces:**
- Consumes: core `ThumbnailFrame`, `thumbnailTimestamps`.
- Produces (exact, used by Task 6):

```ts
export interface ThumbsWorkerApi {
  // Returns one frame per requested timestamp. Never rejects for per-frame
  // problems — outcomes carry them. Rejects only on container-open failure
  // (mapped by the caller to ContainerOpenFailed) or undecodable track
  // (thrown as Error with message 'NO_DECODER', mapped to NoDecoder).
  thumbnails(file: File, timestamps: number[], width: number): Promise<ThumbnailFrame<Blob>[]>
}
export interface ThumbsWorkerHandle { api: Comlink.Remote<ThumbsWorkerApi>; worker: Worker }
export function createThumbsWorker(): ThumbsWorkerHandle
```

- [ ] **Step 1: Add dependencies (latest, via bun)**

Run: `cd web/app && bun add mediabunny @mediabunny/prores`

- [ ] **Step 2: Create `web/app/src/workers/thumbs.worker.ts`**

```ts
/// <reference lib="webworker" />
import type { ThumbnailFrame } from '@luna-web/core'
import { THUMBNAIL_POSITIONS } from '@luna-web/core'
import { registerProresDecoder } from '@mediabunny/prores'
import * as Comlink from 'comlink'
import { ALL_FORMATS, BlobSource, CanvasSink, Input } from 'mediabunny'

registerProresDecoder()

const api = {
  async thumbnails(file: File, timestamps: number[], width: number): Promise<ThumbnailFrame<Blob>[]> {
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
        const canvas = wrapped.canvas as OffscreenCanvas
        const blob = await canvas.convertToBlob({ type: 'image/webp', quality: 0.85 })
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
}

Comlink.expose(api)
```

(If `canvasesAtTimestamps` yields non-null-only results in the installed mediabunny version, drop the null branch and mark missing indices `SeekFailed` after the loop; report what the actual API yielded. If `wrapped.canvas` is an `HTMLCanvasElement` type union, the `as OffscreenCanvas` narrowing in a worker context is legitimate — document it with a comment and count it in the Task 6 cast audit.)

- [ ] **Step 3: Create `web/app/src/features/process/thumbs-client.ts`**

```ts
import type { ThumbnailFrame } from '@luna-web/core'
import * as Comlink from 'comlink'

export interface ThumbsWorkerApi {
  thumbnails(file: File, timestamps: number[], width: number): Promise<ThumbnailFrame<Blob>[]>
}

export interface ThumbsWorkerHandle {
  api: Comlink.Remote<ThumbsWorkerApi>
  worker: Worker
}

export function createThumbsWorker(): ThumbsWorkerHandle {
  const worker = new Worker(new URL('../../workers/thumbs.worker.ts', import.meta.url), {
    type: 'module',
  })
  return { api: Comlink.wrap<ThumbsWorkerApi>(worker), worker }
}
```

- [ ] **Step 4: Gates**

From `web/`: all four gates exit 0 (the new files are not yet imported — that's Task 6; typecheck must still pass). Note in the report whether the build emitted the `@mediabunny/prores` wasm as a bundled asset.

- [ ] **Step 5: Commit**

```bash
git add web/app web/bun.lock
git commit -m "feat(app): mediabunny thumbnail worker with prores decoder"
```

---

## Task 5: ffmpeg.wasm fallback engine

**Files:**
- Create: `web/app/src/features/process/ffmpeg-engine.ts`, `web/app/src/lib/engine-cache.ts`
- Modify: `web/app/package.json` + `web/bun.lock` (via bun add)

**Interfaces:**
- Consumes: core `ThumbnailFrame`.
- Produces (exact, used by Task 6):

```ts
export interface FfmpegEngine {
  thumbnails(file: File, timestamps: number[], width: number): Promise<ThumbnailFrame<Blob>[]>
  dispose(): void
}
export function createFfmpegEngine(): FfmpegEngine  // lazy: engine loads on first thumbnails() call
```

- [ ] **Step 1: Add dependencies (latest, via bun)**

Run: `cd web/app && bun add @ffmpeg/ffmpeg @ffmpeg/util && bun add -d @ffmpeg/core`
(`@ffmpeg/core` is a devDependency ONLY to pin the CDN URL's version through bun/Renovate — its 31 MB payload is never bundled.)

- [ ] **Step 2: Create `web/app/src/lib/engine-cache.ts`**

```ts
// Fetch a large engine binary once, keep it in the Cache API, and hand back
// a blob URL. Later visits (and offline use) hit the cache (spec §11).
const CACHE_NAME = 'luna-engines-v1'

export async function cachedBlobUrl(url: string, mime: string): Promise<string> {
  const cache = await caches.open(CACHE_NAME)
  let response = await cache.match(url)
  if (!response) {
    await cache.add(url)
    response = await cache.match(url)
    if (!response) throw new Error(`Failed to cache engine asset: ${url}`)
  }
  const blob = await response.blob()
  return URL.createObjectURL(new Blob([blob], { type: mime }))
}
```

- [ ] **Step 3: Create `web/app/src/features/process/ffmpeg-engine.ts`**

```ts
import type { ThumbnailFrame } from '@luna-web/core'
import { THUMBNAIL_POSITIONS } from '@luna-web/core'
import { FFmpeg } from '@ffmpeg/ffmpeg'
import { cachedBlobUrl } from '@/lib/engine-cache'
// Version comes from the bun-installed devDependency, not a hand-typed pin.
// If this JSON import is blocked by the package's exports map, fall back to
// a documented constant and report it.
import ffmpegCorePkg from '@ffmpeg/core/package.json' with { type: 'json' }

const CORE_BASE = `https://cdn.jsdelivr.net/npm/@ffmpeg/core@${ffmpegCorePkg.version}/dist/esm`
const MOUNT_DIR = '/work'

export interface FfmpegEngine {
  thumbnails(file: File, timestamps: number[], width: number): Promise<ThumbnailFrame<Blob>[]>
  dispose(): void
}

export function createFfmpegEngine(): FfmpegEngine {
  const ffmpeg = new FFmpeg()
  let loaded: Promise<void> | null = null

  function ensureLoaded(): Promise<void> {
    loaded ??= (async () => {
      const [coreURL, wasmURL] = await Promise.all([
        cachedBlobUrl(`${CORE_BASE}/ffmpeg-core.js`, 'text/javascript'),
        cachedBlobUrl(`${CORE_BASE}/ffmpeg-core.wasm`, 'application/wasm'),
      ])
      await ffmpeg.load({ coreURL, wasmURL })
    })()
    return loaded
  }

  return {
    async thumbnails(file, timestamps, width) {
      await ensureLoaded()
      await ffmpeg.createDir(MOUNT_DIR)
      // WORKERFS: ffmpeg reads the File lazily by range — no whole-file copy.
      await ffmpeg.mount('WORKERFS' as never, { files: [file] }, MOUNT_DIR)
      try {
        const frames: ThumbnailFrame<Blob>[] = []
        for (let i = 0; i < timestamps.length; i++) {
          const t = timestamps[i] ?? 0
          const positionRatio = THUMBNAIL_POSITIONS[i] ?? THUMBNAIL_POSITIONS[0] ?? 0.1
          const out = `frame-${i}.jpg`
          const code = await ffmpeg.exec([
            '-ss', String(t),
            '-i', `${MOUNT_DIR}/${file.name}`,
            '-frames:v', '1',
            '-vf', `scale=${width}:-2`,
            '-q:v', '3',
            out,
          ])
          if (code !== 0) {
            frames.push({ positionRatio, timestampSeconds: t, outcome: 'SeekFailed' })
            continue
          }
          const data = await ffmpeg.readFile(out)
          await ffmpeg.deleteFile(out)
          const bytes = typeof data === 'string' ? new TextEncoder().encode(data) : data
          if (bytes.length === 0) {
            frames.push({ positionRatio, timestampSeconds: t, outcome: 'DecodeFailed' })
            continue
          }
          frames.push({
            positionRatio,
            timestampSeconds: t,
            image: new Blob([bytes as BlobPart], { type: 'image/jpeg' }),
            mime: 'image/jpeg',
            outcome: 'Success',
          })
        }
        return frames
      } finally {
        await ffmpeg.unmount(MOUNT_DIR).catch(() => {})
        await ffmpeg.deleteDir(MOUNT_DIR).catch(() => {})
      }
    },
    dispose() {
      ffmpeg.terminate()
    },
  }
}
```

Implementer notes (verify against the installed `@ffmpeg/ffmpeg` API and report deviations):
- The mount enum: prefer the properly-typed `FFFSType.WORKERFS` if the installed version exports it (from `@ffmpeg/ffmpeg` or `@ffmpeg/types`) — use the real enum and delete the `'WORKERFS' as never` placeholder. If only the string form works, document the cast and count it in Task 6's audit.
- `FFmpeg` spawns its own internal worker — this class runs on the main thread by design (no nested-worker complexity), and `exec` does not block the UI.
- If `createDir`/`deleteDir` don't exist in the installed API, mount to a fixed pre-existing path per its docs.

- [ ] **Step 4: Gates**

From `web/`: all four gates exit 0 (files not yet imported; typecheck must pass).

- [ ] **Step 5: Commit**

```bash
git add web/app web/bun.lock
git commit -m "feat(app): lazy cached ffmpeg.wasm fallback engine"
```

---

## Task 6: Thumbnail orchestration + UI

**Files:**
- Modify: `web/app/src/features/scan/store.ts`, `web/app/src/features/process/run-thumbnails.ts` (replace Task 3's placeholder), `web/app/src/features/scan/scan-screen.tsx`

**Interfaces:**
- Consumes: everything above.
- Produces: the working second pass. Store additions (exact):

```ts
export type ThumbStatus = 'queued' | 'decoding' | 'done' | 'failed'
// ScanPhase gains 'thumbnailing'
// ScanState gains: thumbStatus: Record<string, ThumbStatus>
//                  thumbsById: Record<string, ThumbnailFrame<Blob>[]>
//                  thumbErrors: Record<string, string>
//                  thumbDoneCount: number
export async function startThumbnails(run: number): Promise<void>
```

- [ ] **Step 1: Extend the store**

In `web/app/src/features/scan/store.ts`: add `'thumbnailing'` to `ScanPhase`; add the four fields above to `ScanState` + `initialScanState` (`{}`/`{}`/`{}`/`0`); import `ThumbnailFrame` type from core. Also add the four fields to Task 3's reset block in `startProcessing` now that they exist.

- [ ] **Step 2: Replace `web/app/src/features/process/run-thumbnails.ts`**

```ts
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

  guardedUpdate(run, (s) => ({
    ...s,
    phase: 'thumbnailing',
    thumbStatus: Object.fromEntries(
      clips
        .filter((c) => decodePathFor(c.extension) !== 'none')
        .map((c) => [c.id, 'queued' as const]),
    ),
  }))

  // Cascade queue: mediabunny container/codec failures retry on ffmpeg.
  const cascaded: ClipRef[] = []

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
          setThumbStatus(run, clip.id, 'queued')
        } else {
          failClip(run, clip.id, message)
        }
      },
    },
    { concurrency: poolSizeFor(mediabunnyClips.length), isCancelled: () => !isRunCurrent(run) },
  )

  await mediabunnyPass.catch((err) => {
    // Pool-level failure (e.g. worker construction): fail remaining clips
    // via the ffmpeg queue where possible; report, don't wedge.
    const message = err instanceof Error ? err.message : String(err)
    for (const clip of mediabunnyClips) {
      if (scanStore.state.thumbStatus[clip.id] === 'queued' || scanStore.state.thumbStatus[clip.id] === 'decoding') {
        failClip(run, clip.id, message)
      }
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
```

- [ ] **Step 3: UI — thumbnail strips in `scan-screen.tsx`**

Changes (keep everything else):
- The processing/processed header includes the thumbnailing state:
  `state.phase === 'thumbnailing' ? \`Generating thumbnails… ${state.thumbDoneCount}/${Object.keys(state.thumbStatus).length}\` : …`
- Each `ClipRow` grows a thumbnail strip ABOVE the metadata line (or as a leading cell): a `ThumbStrip` component that renders the clip's `ThumbnailFrame<Blob>[]`:

```tsx
import { useEffect, useState } from 'react'
import type { ThumbnailFrame } from '@luna-web/core'

function ThumbStrip({ frames }: { frames: ThumbnailFrame<Blob>[] | undefined }) {
  const [urls, setUrls] = useState<string[]>([])

  useEffect(() => {
    if (!frames) return
    const next = frames.map((f) => (f.image ? URL.createObjectURL(f.image) : ''))
    setUrls(next)
    return () => {
      for (const u of next) if (u) URL.revokeObjectURL(u)
    }
  }, [frames])

  if (!frames) return null
  return (
    <div className="flex gap-1">
      {frames.map((f, i) =>
        f.outcome === 'Success' && urls[i] ? (
          <img
            key={f.positionRatio}
            src={urls[i]}
            alt={`Frame at ${Math.round(f.positionRatio * 100)}%`}
            className="h-14 rounded object-cover"
            loading="lazy"
          />
        ) : (
          <div
            key={f.positionRatio}
            className="bg-muted text-muted-foreground flex h-14 w-24 items-center justify-center rounded text-xs"
            title={f.outcome}
          >
            {f.outcome === 'NotAttempted' ? 'RAW' : 'n/a'}
          </div>
        ),
      )}
    </div>
  )
}
```

- Wire `<ThumbStrip frames={state.thumbsById[clip.id]} />` into `ClipRow`; while `thumbStatus[clip.id]` is `'queued'`/`'decoding'`, show a subtle placeholder row of three `animate-pulse` boxes instead.
- RAW section rows keep their text-only presentation (they never enter the thumb queues).

- [ ] **Step 4: Cast audit + gates**

Run: `grep -rn " as " web/app/src --include="*.ts" --include="*.tsx" | grep -v "import type" | grep -v " as const"` — every hit must be one of the documented boundary casts (scan handle, the two `getFile() as File` sites, plus any Task 4/5 casts that were explicitly documented). Report the list.
From `web/`: all four gates exit 0.

- [ ] **Step 5: Commit**

```bash
git add web/app/src
git commit -m "feat(app): thumbnail pass with mediabunny/ffmpeg cascade and live strips"
```

---

## Definition of done (this plan)

- `bun test` green (core: prior 16 + runPool 6 + thumbs 6 ≈ 28 tests).
- All gates green from `web/`; build emits the thumbs worker chunk and the prores wasm asset; the ffmpeg core is NOT bundled (CDN + Cache API, loaded lazily).
- Commits per task (Task 3's placeholder makes each task independently green).
- **Manual QA (maintainer, non-blocking, Chromium):**
  1. MP4/H.264 card → thumbnails appear fast, strips fill live, phase reaches processed.
  2. **ProRes MOV** → thumbnails decode (TurboRes) — the headline feature; note rough per-clip speed.
  3. MXF (or AVI) → first such clip triggers the ffmpeg core download (Network tab: jsDelivr, ~31 MB, once); thumbnails arrive; second run offline-cached.
  4. A clip mediabunny can open but not decode → cascades to ffmpeg (watch status go queued→decoding→queued→decoding→done).
  5. Start over mid-thumbnailing → clean reset; no stale thumbs bleed into a re-run.
  6. RAW rows show the RAW placeholder tile, never enter decoding.
  7. Memory: a 100-clip card stays reasonable (thumbs ≈ 15–40 MB total).
  8. `.mts/.m2ts` if available: confirm ffmpeg path handles them; optionally test one via mediabunny in DevTools to inform promoting them to the fast path (spec §21).

## Self-review notes

- **Spec coverage:** §8.4 router + cascade ✓; §8.5 encoder (WebP mediabunny / JPEG ffmpeg; PDF converts at export per §10.3) ✓; §9 ThumbnailFrame/Outcome (generic image payload keeps core DOM-free) ✓; §10.2 decision table ✓; §10.3 counts/positions/width/timeouts ✓; §10.4 seeking (CanvasSink internal; ffmpeg `-ss`) ✓; §11 lazy CDN + Cache API ✓; §15 isolation/retry via runPool ✓.
- **Plan 03 carry-forwards addressed:** generic pool extracted to core with tests (cancellation + retry + error propagation) ✓; `cancelProcessing()` in the catch stops sibling lanes ✓; thumbStatus sibling record (no new global phase beyond the sequential 'thumbnailing') ✓; cooperative-cancellation abort hook remains cooperative (documented in runPool's comment — full AbortSignal plumbing deferred until profiling shows need).
- **Placeholders:** none — Task 3 creates a real compiling placeholder for `startThumbnails` that Task 6 replaces, called out explicitly.
- **Type consistency:** `ThumbsWorkerHandle`/`FfmpegEngine`/`ThumbnailFrame<Blob>`/`ThumbStatus`/`startThumbnails(run)` consistent across Tasks 4–6; `guardedUpdate`/`isRunCurrent`/`poolSizeFor`/`withTimeout` exported from Task 3 and consumed by Task 6.
- **Honest risks flagged to implementers:** mediabunny `canvasesAtTimestamps` null semantics and `WrappedCanvas.canvas` type verified-in-code rather than assumed; ffmpeg mount enum/API surface verified against the installed version; the `@ffmpeg/core/package.json` version import has a documented fallback. Each instructs report-don't-improvise.
