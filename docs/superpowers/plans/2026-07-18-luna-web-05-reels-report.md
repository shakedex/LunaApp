# Luna Web — Plan 05: Reels & Report Workspace Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Processed clips group into camera reels; a report workspace shows hero stats, reel sections, and an editable cover/branding form — producing the complete `ReportModel` that Plan 06's exporters (PDF/CSV) will consume unchanged.

**Architecture:** Reel detection and the `ReportModel` builder are pure, generic core logic (`bun test`-ed): group by embedded `reelName` from metadata, fall back to the clip's top-level folder, else "Ungrouped". Cover fields live in a **separate** `coverStore` so *Start over* never clobbers a DIT's typed-once branding. The processed phase becomes a report workspace (hero stats → cover form → reel sections → RAW section) reusing the existing row components. The logged render-thrash carry-forward is fixed with per-row store selectors + `memo`.

**Tech Stack (additions):** `@tanstack/react-form` — via `bun add` at latest. Nothing else.

**Spec:** `docs/superpowers/specs/2026-07-17-luna-web-design.md` §8.7 (ReelDetector), §8.8 (ReportModel + cardCount), §8.10 (results UI — the sortable TanStack Table grid + virtualization are deferred to Plan 06 alongside exports polish), §9 (model shapes), §12 (exports consume `ReportModel`).

## Global Constraints

- **Never hand-write a dependency version.** New dep via `bun add` (latest).
- **`packages/core` stays DOM-free.** `CoverFields`/`ReportModel` are generic over the image payload (`TImage`) like `ThumbnailFrame`; core never names `Blob`.
- **Tests:** `bun test`, core only (reel detection, cardCount, report builder). UI is maintainer-QA.
- **Gates green after every task** (from `web/`): `bun run lint && bun run typecheck && bun test && bun run build`.
- **Cover fields survive `resetScan`** (separate store) — a DIT types them once per day.
- **No new casts.** The existing documented boundary casts are the complete set.
- **Fidelity:** stats sum only what exists (missing durations contribute 0, never fabricated); reel names come from data, never invented beyond the documented "Ungrouped" fallback.
- **Known divergence (documented here, echoed in code):** the desktop `ReelDetectionService` gates folder-name fallback on camera-roll patterns (`A001`, `REEL_01`); Luna Web groups by reelName → top-level folder (any name) → "Ungrouped". Simpler, deterministic, and the top folder *is* the card layout. Revisit only if QA shows bad groupings.
- **No interactive-CLI handoffs.** The .NET app at the repo root is never touched.

---

## File Structure

```
web/packages/core/src/
  reels/detect.ts              NEW: detectReels, ReelInput, DetectedReel, UNGROUPED_REEL
  report/model.ts              NEW: CoverFields, ReportClip, Reel, ReportStats, ReportModel,
                                    cardCountFrom, buildReportModel
  index.ts                     MODIFY: export new modules
web/packages/core/test/
  reels.test.ts                NEW
  report-model.test.ts         NEW
web/app/src/
  features/report/cover-store.ts   NEW: coverStore (separate from scanStore)
  features/report/cover-form.tsx   NEW: TanStack Form cover fields + logo picker
  features/report/report-workspace.tsx  NEW: hero stats + reel sections (processed phase)
  features/scan/scan-screen.tsx    MODIFY: processed phase renders <ReportWorkspace/>;
                                   per-row selectors + memo (perf carry-forward)
```

---

## Task 1: Core reel detection (TDD)

**Files:**
- Create: `web/packages/core/src/reels/detect.ts`
- Modify: `web/packages/core/src/index.ts`
- Test: `web/packages/core/test/reels.test.ts`

**Interfaces:**
- Produces (exact, used by Task 2):

```ts
export interface ReelInput { relativePath: string; reelName?: string }
export interface DetectedReel<T extends ReelInput> { name: string; clips: T[] }
export const UNGROUPED_REEL = 'Ungrouped'
export function detectReels<T extends ReelInput>(clips: readonly T[]): DetectedReel<T>[]
```

Rules (these ARE the test cases): group key = trimmed non-empty `reelName`; else the first `/`-segment of `relativePath` when the path is nested; else `UNGROUPED_REEL`. Reels sorted by name with numeric-aware compare (`A002` after `A001`, `REEL_2` before `REEL_10`); clips within a reel sorted by `relativePath`. Deterministic for identical input.

- [ ] **Step 1: Write the failing test**

Create `web/packages/core/test/reels.test.ts`:

```ts
import { describe, expect, test } from 'bun:test'
import { detectReels, UNGROUPED_REEL } from '../src/reels/detect'

const clip = (relativePath: string, reelName?: string) => ({ relativePath, reelName })

describe('detectReels', () => {
  test('groups by embedded reelName first, regardless of folders', () => {
    const reels = detectReels([
      clip('X/one.mov', 'A001R2B'),
      clip('Y/two.mov', 'A001R2B'),
      clip('X/three.mov', 'B001R1A'),
    ])
    expect(reels.map((r) => r.name)).toEqual(['A001R2B', 'B001R1A'])
    expect(reels[0]?.clips.map((c) => c.relativePath)).toEqual(['X/one.mov', 'Y/two.mov'])
  })

  test('falls back to the top-level folder when reelName is absent', () => {
    const reels = detectReels([
      clip('A001/c2.mov'),
      clip('A001/c1.mov'),
      clip('B002/c3.mov'),
    ])
    expect(reels.map((r) => r.name)).toEqual(['A001', 'B002'])
    expect(reels[0]?.clips.map((c) => c.relativePath)).toEqual(['A001/c1.mov', 'A001/c2.mov'])
  })

  test('root-level clips without reelName land in Ungrouped', () => {
    const reels = detectReels([clip('loose.mov'), clip('A001/x.mov')])
    expect(reels.map((r) => r.name)).toEqual(['A001', UNGROUPED_REEL])
  })

  test('whitespace-only reelName is treated as absent', () => {
    const reels = detectReels([clip('A001/x.mov', '   ')])
    expect(reels.map((r) => r.name)).toEqual(['A001'])
  })

  test('numeric-aware reel ordering', () => {
    const reels = detectReels([clip('REEL_10/a.mov'), clip('REEL_2/b.mov'), clip('A002/c.mov'), clip('A001/d.mov')])
    expect(reels.map((r) => r.name)).toEqual(['A001', 'A002', 'REEL_2', 'REEL_10'])
  })

  test('empty input yields no reels', () => {
    expect(detectReels([])).toEqual([])
  })
})
```

- [ ] **Step 2: Run to verify it fails**

Run: `cd web/packages/core && bun test reels`
Expected: FAIL — module does not exist.

- [ ] **Step 3: Implement `web/packages/core/src/reels/detect.ts`**

```ts
// Reel (camera roll) grouping. Divergence from the desktop ReelDetectionService
// is deliberate and documented in the plan: desktop pattern-gates the folder
// fallback (A001 / REEL_01); here ANY top-level folder groups, because the top
// folder is the card layout. reelName from container metadata always wins.
export interface ReelInput {
  relativePath: string
  reelName?: string
}

export interface DetectedReel<T extends ReelInput> {
  name: string
  clips: T[]
}

export const UNGROUPED_REEL = 'Ungrouped'

function reelKeyFor(clip: ReelInput): string {
  const fromMetadata = clip.reelName?.trim()
  if (fromMetadata) return fromMetadata
  const slash = clip.relativePath.indexOf('/')
  if (slash > 0) return clip.relativePath.slice(0, slash)
  return UNGROUPED_REEL
}

export function detectReels<T extends ReelInput>(clips: readonly T[]): DetectedReel<T>[] {
  const byName = new Map<string, T[]>()
  for (const clip of clips) {
    const key = reelKeyFor(clip)
    const group = byName.get(key)
    if (group) group.push(clip)
    else byName.set(key, [clip])
  }
  const reels = [...byName.entries()].map(([name, group]) => ({
    name,
    clips: [...group].sort((a, b) => a.relativePath.localeCompare(b.relativePath)),
  }))
  return reels.sort((a, b) => a.name.localeCompare(b.name, undefined, { numeric: true }))
}
```

Add to `web/packages/core/src/index.ts`:

```ts
export { detectReels, UNGROUPED_REEL } from './reels/detect'
export type { DetectedReel, ReelInput } from './reels/detect'
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cd web/packages/core && bun test`
Expected: PASS (31 prior + 6 new).

- [ ] **Step 5: Gates and commit**

From `web/`: all four gates exit 0.

```bash
git add web/packages/core
git commit -m "feat(core): reel detection with metadata-first grouping"
```

---

## Task 2: Core report model + builder (TDD)

**Files:**
- Create: `web/packages/core/src/report/model.ts`
- Modify: `web/packages/core/src/index.ts`
- Test: `web/packages/core/test/report-model.test.ts`

**Interfaces:**
- Produces (exact — this is Plan 06's exporter input, treat as frozen):

```ts
export interface CoverFields<TImage = unknown> {
  projectTitle?: string
  productionCompany?: string
  dit?: string
  director?: string
  dp?: string
  date?: string        // display string, app decides format
  logo?: TImage
}
export interface ReportClip<TImage = unknown> {
  id: string
  fileName: string
  relativePath: string
  extension: string
  sizeBytes: number
  metadata: ClipMetadata          // {} when the metadata pass failed
  thumbnails: ThumbnailFrame<TImage>[]  // [] when none
}
export interface Reel<TImage = unknown> { name: string; clips: ReportClip<TImage>[] }
export interface ReportStats {
  cardCount: number
  clipCount: number
  rawCount: number
  totalDurationSeconds: number
  totalSizeBytes: number
}
export interface ReportModel<TImage = unknown> {
  cover: CoverFields<TImage>
  stats: ReportStats
  reels: Reel<TImage>[]
  raw: RawNotice[]
}
export function cardCountFrom(relativePaths: readonly string[]): number
// spec §8.8: distinct top-level segments; media-at-root-only → 1; no clips → 0
export interface BuildReportInput<TImage = unknown> {
  clips: readonly ClipRef[]
  raw: readonly RawNotice[]
  metadataById: Readonly<Record<string, ClipMetadata>>
  thumbsById: Readonly<Record<string, ThumbnailFrame<TImage>[]>>
  cover: CoverFields<TImage>
}
export function buildReportModel<TImage = unknown>(input: BuildReportInput<TImage>): ReportModel<TImage>
```

- [ ] **Step 1: Write the failing test**

Create `web/packages/core/test/report-model.test.ts`:

```ts
import { describe, expect, test } from 'bun:test'
import type { BlobLike, FileHandleLike } from '../src/scan/handles'
import type { ClipRef } from '../src/scan/model'
import { buildReportModel, cardCountFrom } from '../src/report/model'

const fakeBlob: BlobLike = { size: 0, slice: () => fakeBlob, arrayBuffer: async () => new ArrayBuffer(0) }
const fakeFile: FileHandleLike = { kind: 'file', name: 'x', getFile: async () => fakeBlob }
const ref = (relativePath: string, sizeBytes: number): ClipRef => ({
  id: relativePath,
  fileName: relativePath.split('/').pop() ?? relativePath,
  relativePath,
  extension: `.${relativePath.split('.').pop() ?? ''}`,
  sizeBytes,
  file: fakeFile,
})

describe('cardCountFrom', () => {
  test('distinct top-level folders', () => {
    expect(cardCountFrom(['A001/a.mov', 'A001/b.mov', 'B002/c.mov'])).toBe(2)
  })
  test('media at root only counts as one card', () => {
    expect(cardCountFrom(['a.mov', 'b.mov'])).toBe(1)
  })
  test('mixed root + folders counts folders only (spec §8.8)', () => {
    expect(cardCountFrom(['a.mov', 'A001/b.mov'])).toBe(1)
  })
  test('no clips, no cards', () => {
    expect(cardCountFrom([])).toBe(0)
  })
})

describe('buildReportModel', () => {
  test('merges metadata + thumbnails, groups reels, sums stats', () => {
    const clips = [ref('A001/one.mov', 100), ref('A001/two.mov', 50), ref('B002/three.mxf', 25)]
    const model = buildReportModel({
      clips,
      raw: [],
      metadataById: {
        'A001/one.mov': { durationSeconds: 10, reelName: 'CUSTOM' },
        'A001/two.mov': { durationSeconds: 5 },
      },
      thumbsById: {
        'A001/one.mov': [{ positionRatio: 0.1, timestampSeconds: 1, outcome: 'Success' }],
      },
      cover: { projectTitle: 'Test' },
    })
    expect(model.stats).toEqual({
      cardCount: 2,
      clipCount: 3,
      rawCount: 0,
      totalDurationSeconds: 15, // missing duration contributes 0, never fabricated
      totalSizeBytes: 175,
    })
    expect(model.reels.map((r) => r.name)).toEqual(['A001', 'B002', 'CUSTOM'])
    const custom = model.reels.find((r) => r.name === 'CUSTOM')
    expect(custom?.clips[0]?.thumbnails.length).toBe(1)
    const three = model.reels.find((r) => r.name === 'B002')?.clips[0]
    expect(three?.metadata).toEqual({}) // failed metadata → empty, present
    expect(three?.thumbnails).toEqual([])
    expect(model.cover.projectTitle).toBe('Test')
  })

  test('empty input produces an empty, zeroed model', () => {
    const model = buildReportModel({ clips: [], raw: [], metadataById: {}, thumbsById: {}, cover: {} })
    expect(model.stats).toEqual({ cardCount: 0, clipCount: 0, rawCount: 0, totalDurationSeconds: 0, totalSizeBytes: 0 })
    expect(model.reels).toEqual([])
  })
})
```

- [ ] **Step 2: Run to verify it fails**

Run: `cd web/packages/core && bun test report-model`
Expected: FAIL — module does not exist.

- [ ] **Step 3: Implement `web/packages/core/src/report/model.ts`**

```ts
import type { ClipMetadata } from '../metadata/model'
import { detectReels } from '../reels/detect'
import type { ClipRef, RawNotice } from '../scan/model'
import type { ThumbnailFrame } from '../thumbs/model'

export interface CoverFields<TImage = unknown> {
  projectTitle?: string
  productionCompany?: string
  dit?: string
  director?: string
  dp?: string
  date?: string
  logo?: TImage
}

export interface ReportClip<TImage = unknown> {
  id: string
  fileName: string
  relativePath: string
  extension: string
  sizeBytes: number
  metadata: ClipMetadata
  thumbnails: ThumbnailFrame<TImage>[]
}

export interface Reel<TImage = unknown> {
  name: string
  clips: ReportClip<TImage>[]
}

export interface ReportStats {
  cardCount: number
  clipCount: number
  rawCount: number
  totalDurationSeconds: number
  totalSizeBytes: number
}

export interface ReportModel<TImage = unknown> {
  cover: CoverFields<TImage>
  stats: ReportStats
  reels: Reel<TImage>[]
  raw: RawNotice[]
}

// spec §8.8: a "card" is a top-level media subfolder; media at the root with
// no subfolders counts as one card; an empty scan has zero.
export function cardCountFrom(relativePaths: readonly string[]): number {
  if (relativePaths.length === 0) return 0
  const folders = new Set<string>()
  for (const path of relativePaths) {
    const slash = path.indexOf('/')
    if (slash > 0) folders.add(path.slice(0, slash))
  }
  return folders.size > 0 ? folders.size : 1
}

export interface BuildReportInput<TImage = unknown> {
  clips: readonly ClipRef[]
  raw: readonly RawNotice[]
  metadataById: Readonly<Record<string, ClipMetadata>>
  thumbsById: Readonly<Record<string, ThumbnailFrame<TImage>[]>>
  cover: CoverFields<TImage>
}

export function buildReportModel<TImage = unknown>(
  input: BuildReportInput<TImage>,
): ReportModel<TImage> {
  const reportClips = input.clips.map((clip) => {
    const metadata = input.metadataById[clip.id] ?? {}
    return {
      id: clip.id,
      fileName: clip.fileName,
      relativePath: clip.relativePath,
      extension: clip.extension,
      sizeBytes: clip.sizeBytes,
      metadata,
      thumbnails: input.thumbsById[clip.id] ?? [],
      reelName: metadata.reelName,
    }
  })

  let totalDurationSeconds = 0
  let totalSizeBytes = 0
  for (const clip of reportClips) {
    totalDurationSeconds += clip.metadata.durationSeconds ?? 0
    totalSizeBytes += clip.sizeBytes
  }

  const reels = detectReels(reportClips).map((reel) => ({
    name: reel.name,
    clips: reel.clips.map(({ reelName: _drop, ...clip }) => clip),
  }))

  return {
    cover: input.cover,
    stats: {
      cardCount: cardCountFrom(input.clips.map((c) => c.relativePath)),
      clipCount: input.clips.length,
      rawCount: input.raw.length,
      totalDurationSeconds,
      totalSizeBytes,
    },
    reels,
    raw: [...input.raw],
  }
}
```

Add to `web/packages/core/src/index.ts`:

```ts
export { buildReportModel, cardCountFrom } from './report/model'
export type {
  BuildReportInput,
  CoverFields,
  Reel,
  ReportClip,
  ReportModel,
  ReportStats,
} from './report/model'
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cd web/packages/core && bun test`
Expected: PASS (43 total).

- [ ] **Step 5: Gates and commit**

From `web/`: all four gates exit 0.

```bash
git add web/packages/core
git commit -m "feat(core): report model with reel grouping and hero stats"
```

---

## Task 3: Cover store + cover form (app)

**Files:**
- Create: `web/app/src/features/report/cover-store.ts`, `web/app/src/features/report/cover-form.tsx`
- Modify: `web/app/package.json` + `web/bun.lock` (via bun add)

**Interfaces:**
- Produces (exact, used by Task 4 and Plan 06):

```ts
export const coverStore: Store<CoverFields<Blob>>   // SEPARATE from scanStore; survives resetScan
export function setCoverFields(patch: Partial<CoverFields<Blob>>): void
export function CoverForm(): JSX.Element
```

- [ ] **Step 1: Add the dependency (latest, via bun)**

Run: `cd web/app && bun add @tanstack/react-form`

- [ ] **Step 2: Create `web/app/src/features/report/cover-store.ts`**

```ts
import type { CoverFields } from '@luna-web/core'
import { Store } from '@tanstack/store'

// Deliberately separate from scanStore: a DIT types the cover once per day;
// "Start over" (resetScan) must never clobber it. Settings-persisted defaults
// arrive in Plan 08.
export const coverStore = new Store<CoverFields<Blob>>({})

export function setCoverFields(patch: Partial<CoverFields<Blob>>): void {
  coverStore.setState((s) => ({ ...s, ...patch }))
}
```

- [ ] **Step 3: Create `web/app/src/features/report/cover-form.tsx`**

Text fields sync to the store on blur (no save button, no render-loop hazards); the
logo is a plain file input writing the Blob directly, with an object-URL preview.

```tsx
import { useForm } from '@tanstack/react-form'
import { useEffect, useState } from 'react'
import { useStore } from '@tanstack/react-store'
import { coverStore, setCoverFields } from './cover-store'

const TEXT_FIELDS = [
  ['projectTitle', 'Project title'],
  ['productionCompany', 'Production company'],
  ['dit', 'DIT'],
  ['director', 'Director'],
  ['dp', 'Director of photography'],
  ['date', 'Date'],
] as const

type TextFieldName = (typeof TEXT_FIELDS)[number][0]

export function CoverForm() {
  const cover = coverStore.state

  const form = useForm({
    defaultValues: {
      projectTitle: cover.projectTitle ?? '',
      productionCompany: cover.productionCompany ?? '',
      dit: cover.dit ?? '',
      director: cover.director ?? '',
      dp: cover.dp ?? '',
      date: cover.date ?? new Date().toISOString().slice(0, 10),
    },
  })

  return (
    <section className="w-full rounded-lg border p-6">
      <h3 className="mb-4 text-lg font-medium">Report details</h3>
      <div className="grid grid-cols-2 gap-4">
        {TEXT_FIELDS.map(([name, label]) => (
          <form.Field key={name} name={name satisfies TextFieldName}>
            {(field) => (
              <label className="flex flex-col gap-1 text-sm">
                <span className="text-muted-foreground">{label}</span>
                <input
                  className="bg-background rounded-md border px-3 py-2"
                  name={field.name}
                  value={field.state.value}
                  onChange={(e) => field.handleChange(e.target.value)}
                  onBlur={() => {
                    field.handleBlur()
                    setCoverFields({ [field.name]: field.state.value })
                  }}
                />
              </label>
            )}
          </form.Field>
        ))}
        <LogoPicker />
      </div>
    </section>
  )
}

function LogoPicker() {
  const logo = useStore(coverStore, (s) => s.logo)
  const [previewUrl, setPreviewUrl] = useState<string | null>(null)

  useEffect(() => {
    if (!logo) {
      setPreviewUrl(null)
      return
    }
    const url = URL.createObjectURL(logo)
    setPreviewUrl(url)
    return () => URL.revokeObjectURL(url)
  }, [logo])

  return (
    <label className="flex flex-col gap-1 text-sm">
      <span className="text-muted-foreground">Logo</span>
      <input
        type="file"
        accept="image/*"
        className="text-sm"
        onChange={(e) => {
          const file = e.target.files?.[0]
          if (file) setCoverFields({ logo: file })
        }}
      />
      {previewUrl && <img src={previewUrl} alt="Report logo preview" className="mt-1 h-10 w-auto object-contain" />}
    </label>
  )
}
```

Implementer notes: `defaultValues` reads `coverStore.state` once at mount — the form is
the editor, the store is the source of truth between mounts. If the installed
`@tanstack/react-form`'s `form.Field` children/render-prop API differs from this shape,
adapt minimally per its types and report. If the `satisfies` in the `name` prop fights
the generic, drop it — the `TEXT_FIELDS` tuple already constrains names.

- [ ] **Step 4: Gates**

From `web/`: all four gates exit 0 (components not yet rendered — Task 4 wires; typecheck must pass).

- [ ] **Step 5: Commit**

```bash
git add web/app web/bun.lock
git commit -m "feat(app): cover store and report-details form"
```

---

## Task 4: Report workspace (processed phase)

**Files:**
- Create: `web/app/src/features/report/report-workspace.tsx`
- Modify: `web/app/src/features/scan/scan-screen.tsx`

**Interfaces:**
- Consumes: `buildReportModel`, `coverStore`, `CoverForm`, the existing `ClipRow`/`ThumbStrip`/RAW-section pieces, `formatBytes`/`formatDuration`.
- Produces: `<ReportWorkspace />` rendered by the processed phase; scan-screen's processed branch shrinks to it (processing/thumbnailing keep the current flat live list).

- [ ] **Step 1: Create `web/app/src/features/report/report-workspace.tsx`**

```tsx
import { buildReportModel } from '@luna-web/core'
import { useStore } from '@tanstack/react-store'
import { useMemo } from 'react'
import { Button } from '@/components/ui/button'
import { ClipRow, RawSection } from '@/features/scan/scan-screen'
import { resetScan } from '@/features/scan/run-scan'
import { scanStore } from '@/features/scan/store'
import { formatBytes, formatDuration } from '@/lib/format'
import { CoverForm } from './cover-form'
import { coverStore } from './cover-store'

export function ReportWorkspace() {
  const clips = useStore(scanStore, (s) => s.clips)
  const raw = useStore(scanStore, (s) => s.raw)
  const metadataById = useStore(scanStore, (s) => s.metadataById)
  const thumbsById = useStore(scanStore, (s) => s.thumbsById)
  const cover = useStore(coverStore)

  const model = useMemo(
    () => buildReportModel({ clips, raw, metadataById, thumbsById, cover }),
    [clips, raw, metadataById, thumbsById, cover],
  )

  return (
    <section className="w-full">
      <div className="mb-4 flex items-center justify-between">
        <h2 className="text-xl font-medium">{model.cover.projectTitle || 'Camera report'}</h2>
        <Button variant="outline" onClick={resetScan}>
          Start over
        </Button>
      </div>

      <dl className="mb-6 grid grid-cols-5 gap-4 text-center">
        <Stat label="Cards" value={String(model.stats.cardCount)} />
        <Stat label="Clips" value={String(model.stats.clipCount)} />
        <Stat label="Duration" value={formatDuration(model.stats.totalDurationSeconds)} />
        <Stat label="Size" value={formatBytes(model.stats.totalSizeBytes)} />
        <Stat label="RAW" value={String(model.stats.rawCount)} />
      </dl>

      <div className="mb-6">
        <CoverForm />
      </div>

      {model.reels.map((reel) => (
        <section key={reel.name} className="mb-6">
          <h3 className="mb-2 flex items-baseline gap-3 text-lg font-medium">
            {reel.name}
            <span className="text-muted-foreground text-sm">
              {reel.clips.length} clips ·{' '}
              {formatBytes(reel.clips.reduce((n, c) => n + c.sizeBytes, 0))}
            </span>
          </h3>
          <ul className="divide-y rounded-lg border">
            {reel.clips.map((clip) => (
              <ClipRow key={clip.id} clipId={clip.id} />
            ))}
          </ul>
        </section>
      ))}

      <RawSection raw={model.raw} />
      <p className="text-muted-foreground mt-3 text-sm">Exports (PDF/CSV) arrive in the next milestone.</p>
    </section>
  )
}

function Stat({ label, value }: { label: string; value: string }) {
  return (
    <div>
      <dt className="text-muted-foreground text-sm">{label}</dt>
      <dd className="text-2xl">{value}</dd>
    </div>
  )
}
```

- [ ] **Step 2: Rewire `scan-screen.tsx`**

- Split the current processing/processed branch: `state.phase === 'processing' || state.phase === 'thumbnailing'` keeps the existing live flat list; `state.phase === 'processed'` renders `<ReportWorkspace />` instead.
- Export the pieces the workspace reuses: `ClipRow` (see Task 5's new signature) and extract the RAW list into an exported `RawSection({ raw })` component (same markup as today).
- Delete the old processed-only bits that moved into the workspace (header/Start-over for that phase).

- [ ] **Step 3: Gates and commit**

From `web/`: all four gates exit 0. Manual sanity: full run reaches the workspace with stats/reels/cover form.

```bash
git add web/app/src
git commit -m "feat(app): reel-grouped report workspace with hero stats"
```

---

## Task 5: Per-row selectors + memoization (perf carry-forward)

**Files:**
- Modify: `web/app/src/features/scan/scan-screen.tsx`

**Interfaces:**
- Produces: `ClipRow` re-signed as `memo(function ClipRow({ clipId }: { clipId: string }))` — it receives ONLY the id and subscribes narrowly itself:

```tsx
const status = useStore(scanStore, (s) => s.clipStatus[clipId] ?? 'queued')
const metadata = useStore(scanStore, (s) => s.metadataById[clipId])
const thumbStatus = useStore(scanStore, (s) => s.thumbStatus[clipId])
const frames = useStore(scanStore, (s) => s.thumbsById[clipId])
const error = useStore(scanStore, (s) => s.clipErrors[clipId])
const clip = useStore(scanStore, (s) => s.clips.find((c) => c.id === clipId))
```

- [ ] **Step 1: Refactor**

- `ClipRow` takes `{ clipId }`, subscribes per-field as above (each `useStore` selector returns a stable primitive/reference, so unrelated clip updates no longer re-render this row), wrapped in `React.memo`. Guard `if (!clip) return null`.
- The list containers (live list + reel sections) map ids only: `clips.map((c) => <ClipRow key={c.id} clipId={c.id} />)`.
- The top-level `ScanScreen` stops subscribing to the whole store: replace `useStore(scanStore)` with narrow selectors for exactly what the shell renders (`phase`, `sourceName`, `progress`, `summary`, `processedCount`, `thumbDoneCount`, `clips` (for count/ids), `thumbStatus` denominator via `Object.keys(s.thumbStatus).length`, `error`, `raw`).
- `ThumbStrip` keeps its `frames` prop (passed from the row's narrow subscription).

- [ ] **Step 2: Gates and commit**

From `web/`: all four gates exit 0. Manual sanity: during processing, React DevTools highlight shows only the updating row re-rendering (spot-check, non-blocking).

```bash
git add web/app/src
git commit -m "perf(app): per-row store selectors and row memoization"
```

---

## Definition of done (this plan)

- `bun test` green (43 core tests: prior 31 + reels 6 + report-model 6).
- All gates green from `web/`.
- **Manual QA (maintainer, non-blocking, Chromium):**
  1. Full run on a multi-folder card → workspace shows correct Cards/Clips/Duration/Size/RAW; reels match the folder/reelName layout; clips sorted within reels.
  2. Clips with embedded reel names (MXF) group by reelName even across folders.
  3. Fill cover fields, pick a logo → survive **Start over** + a fresh scan (separate store).
  4. Date defaults to today; edits stick after blur.
  5. Large card during processing: rows fill without whole-list flicker (per-row selectors).
  6. RAW section renders in the workspace.

## Self-review notes

- **Spec coverage:** §8.7 reel detection (documented divergence: no pattern gating) ✓; §8.8 ReportModel + cardCount exactly per spec ✓; §9 shapes (generic TImage keeps core DOM-free; `metadata: {}` / `thumbnails: []` for failures — never fabricated) ✓; §8.10 partial by design — the sortable/filterable TanStack Table grid + virtualization are explicitly Plan 06's; cover form per §7 (TanStack Form) with the store-survives-reset behavior chosen deliberately.
- **Plan 04 carry-forwards addressed:** per-row selectors + memoization (Task 5) ✓; variable frame counts flow through untouched (`thumbnails` array as-is) ✓; the mime-branching export rule is *recorded for Plan 06* (ReportModel preserves per-frame `mime`).
- **Placeholders:** none. Task 4 imports `ClipRow`/`RawSection` in their Task-5 signatures — the plan orders Task 4 before Task 5 but Task 4's Step 2 explicitly exports both pieces; implementers of Task 4 create `RawSection` and re-export the CURRENT ClipRow signature adapted per Task 5's `{ clipId }` contract... **Correction for implementers:** to keep every task independently green, Task 4 performs the `ClipRow({ clipId })` re-signature as part of its Step 2 (the workspace needs it), and Task 5 then completes the narrowing of `ScanScreen`'s own subscriptions + memo audit. The two commits stay separately meaningful.
- **Type consistency:** `CoverFields<Blob>`/`coverStore`/`setCoverFields` across Tasks 3–4; `buildReportModel` input field names match the store's exact field names (`clips/raw/metadataById/thumbsById`); `ClipRow({ clipId })`/`RawSection({ raw })` across Tasks 4–5.
- **Honest risks:** TanStack Form API drift → verify-and-adapt note in Task 3; the `reelName` temporary field in `buildReportModel`'s intermediate objects is dropped via destructuring before reels are returned (keeps `ReportClip` exact).
