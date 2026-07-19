# Luna Web — Plan 12: Report Library Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A Report Library: an explicit "Save report" button persists the finished ReportModel (thumbnails included) to IndexedDB; a `/reports` route lists saved reports (newest first) with storage usage; opening one renders it read-only with working PDF/CSV export; per-entry delete. No cap/eviction — user-controlled, quota errors surfaced honestly, `navigator.storage.persist()` requested to protect saved data.

**Architecture:** Core gets the pure summary derivation + defensive list normalization (bun-tested). The app adds idb v4 (`reportSummaries` + `reportModels` stores — summaries are tiny so listing never touches frame Blobs), a persistence module, and extracts the model-driven report JSX from `report-workspace.tsx` into a shared `ReportView` so the live workspace and saved reports render through one component.

**Tech Stack:** Existing only. **Zero new dependencies.**

## Global Constraints

- **Zero new dependencies.** Gates from REPO ROOT: `bun run lint && bun run typecheck && bun test && bun run build` — green before AND after every task. Baseline at plan time: 157 tests.
- **Never touch** `apps/web/src/components/ui/` or `tools/`. **`packages/core` stays DOM-free and clock-free** (id/savedAt/byte-sizer injected by the caller).
- **Stage by EXPLICIT file paths.** `git status` first; the maintainer works the repo in parallel (deploy/docs/PDF track) — their dirty/untracked files are OFF-LIMITS; if a file YOU must modify is dirty → STOP/BLOCKED.
- Maintainer-styled files touched by this plan: `report-workspace.tsx` (Task 3 — a byte-preserving JSX MOVE into `report-view.tsx`, styling classes must survive character-for-character) and `app-shell.tsx` (Task 4 — one nav Link). Minimal diffs, stop-if-dirty.
- TypeScript 6.0.3; `.gitattributes` pins LF; Windows, Git Bash tool, `cd /e/Coding/LunaApp` first, bun 1.3.14.
- Approved decisions recorded: name = **Reports / Report Library**; explicit save (no auto-save); no cap/eviction; read-only saved view (v1); every run saves a distinct entry; entries list title/date/source/clip count/size/thumbnails badge.

---

### Task 1: Core report-library summary + defensive list normalization (TDD)

**Files:**
- Create: `packages/core/src/report/library.ts`
- Create: `packages/core/src/report/library.test.ts`
- Modify: `packages/core/src/index.ts` (exports in the report block)

**Interfaces:**
- Consumes: `ReportModel<TImage>` from `./model`.
- Produces (Tasks 2/4 rely on):
  - `interface ReportSummary { id: string; savedAt: number; title: string; sourceRoot: string; clipCount: number; otherFileCount: number; totalSizeBytes: number; storedFrameBytes: number; hasThumbnails: boolean }`
  - `summarizeReport<TImage>(model: ReportModel<TImage>, meta: { id: string; savedAt: number }, imageBytesOf: (image: TImage) => number): ReportSummary` — `title` = `cover.projectTitle` (non-empty) else `sourceRoot` (non-empty) else `'Camera report'`; counts/sizes from `model.stats`; `hasThumbnails` = any clip frame with `outcome === 'Success'` and an `image`; `storedFrameBytes` = sum of `imageBytesOf(frame.image)` over every frame carrying an image.
  - `normalizeReportSummaries(raw: unknown): ReportSummary[]` — non-array → `[]`; structurally invalid members dropped; result sorted `savedAt` DESC (ties by `id` desc).

- [ ] **Step 1: Write the failing tests**

Create `packages/core/src/report/library.test.ts`:

```ts
import { describe, expect, test } from 'bun:test'
import type { ReportModel } from './model'
import { normalizeReportSummaries, type ReportSummary, summarizeReport } from './library'

type Img = { bytes: number }

function model(overrides: Partial<ReportModel<Img>> = {}): ReportModel<Img> {
  return {
    cover: { projectTitle: 'Luna Feature' },
    sourceRoot: 'CARD_A',
    stats: {
      cardCount: 1,
      clipCount: 2,
      otherFileCount: 1,
      otherFileSizeBytes: 10,
      totalDurationSeconds: 120,
      totalSizeBytes: 5_000,
    },
    reels: [
      {
        name: 'A001',
        clips: [
          {
            id: 'A001/a.mov',
            fileName: 'a.mov',
            relativePath: 'A001/a.mov',
            extension: '.mov',
            sizeBytes: 2_000,
            metadata: {},
            thumbnails: [
              { positionRatio: 0.5, timestampSeconds: 1, outcome: 'Success', image: { bytes: 300 } },
              { positionRatio: 0.9, timestampSeconds: 2, outcome: 'SeekFailed' },
            ],
          },
          {
            id: 'A001/b.mov',
            fileName: 'b.mov',
            relativePath: 'A001/b.mov',
            extension: '.mov',
            sizeBytes: 2_990,
            metadata: {},
            thumbnails: [],
          },
        ],
        otherFiles: [],
        stats: {
          clipCount: 2,
          otherFileCount: 0,
          otherFileSizeBytes: 0,
          totalSizeBytes: 4_990,
          totalDurationSeconds: 120,
        },
      },
    ],
    ...overrides,
  }
}

const META = { id: 'r-1', savedAt: 1_700_000_000_000 }
const bytesOf = (image: Img) => image.bytes

describe('summarizeReport', () => {
  test('derives title, counts, sizes, thumbnail bytes and flag', () => {
    expect(summarizeReport(model(), META, bytesOf)).toEqual({
      id: 'r-1',
      savedAt: 1_700_000_000_000,
      title: 'Luna Feature',
      sourceRoot: 'CARD_A',
      clipCount: 2,
      otherFileCount: 1,
      totalSizeBytes: 5_000,
      storedFrameBytes: 300,
      hasThumbnails: true,
    })
  })

  test('title falls back to sourceRoot, then to "Camera report"', () => {
    expect(summarizeReport(model({ cover: {} }), META, bytesOf).title).toBe('CARD_A')
    expect(summarizeReport(model({ cover: {}, sourceRoot: '' }), META, bytesOf).title).toBe(
      'Camera report',
    )
    expect(summarizeReport(model({ cover: { projectTitle: '  ' } }), META, bytesOf).title).toBe(
      'CARD_A',
    )
  })

  test('metadata-only report: no thumbnails, zero stored bytes', () => {
    const m = model()
    for (const reel of m.reels) for (const clip of reel.clips) clip.thumbnails = []
    const summary = summarizeReport(m, META, bytesOf)
    expect(summary.hasThumbnails).toBe(false)
    expect(summary.storedFrameBytes).toBe(0)
  })

  test('non-Success frames with images still count toward stored bytes', () => {
    const m = model()
    m.reels[0]?.clips[0]?.thumbnails.push({
      positionRatio: 0.1,
      timestampSeconds: 0,
      outcome: 'DecodeFailed',
      image: { bytes: 50 },
    })
    expect(summarizeReport(m, META, bytesOf).storedFrameBytes).toBe(350)
  })
})

describe('normalizeReportSummaries', () => {
  const valid: ReportSummary = {
    id: 'r-1',
    savedAt: 100,
    title: 't',
    sourceRoot: 's',
    clipCount: 1,
    otherFileCount: 0,
    totalSizeBytes: 10,
    storedFrameBytes: 0,
    hasThumbnails: false,
  }

  test('non-array yields empty', () => {
    expect(normalizeReportSummaries(undefined)).toEqual([])
    expect(normalizeReportSummaries('junk')).toEqual([])
  })

  test('invalid members dropped, valid kept, sorted newest first', () => {
    const newer = { ...valid, id: 'r-2', savedAt: 200 }
    const result = normalizeReportSummaries([valid, { id: 7 }, null, newer, { ...valid, savedAt: 'x' }])
    expect(result.map((s) => s.id)).toEqual(['r-2', 'r-1'])
  })
})
```

- [ ] **Step 2: Run to verify failure** — `bun test packages/core/src/report/library.test.ts` → FAIL (module missing).

- [ ] **Step 3: Implement**

Create `packages/core/src/report/library.ts`:

```ts
import type { ReportModel } from './model'

// What the /reports list renders — deliberately tiny and Blob-free so listing
// the library never loads a single stored frame.
export interface ReportSummary {
  id: string
  savedAt: number // epoch ms, caller-supplied (core stays clock-free)
  title: string
  sourceRoot: string
  clipCount: number
  otherFileCount: number
  totalSizeBytes: number // the card's bytes (model.stats), not the stored record's
  storedFrameBytes: number // bytes of thumbnail images persisted with the report
  hasThumbnails: boolean
}

export function summarizeReport<TImage>(
  model: ReportModel<TImage>,
  meta: { id: string; savedAt: number },
  imageBytesOf: (image: TImage) => number,
): ReportSummary {
  let storedFrameBytes = 0
  let hasThumbnails = false
  for (const reel of model.reels) {
    for (const clip of reel.clips) {
      for (const frame of clip.thumbnails) {
        if (frame.image !== undefined) storedFrameBytes += imageBytesOf(frame.image)
        if (frame.outcome === 'Success' && frame.image !== undefined) hasThumbnails = true
      }
    }
  }
  const projectTitle = model.cover.projectTitle
  const title =
    (typeof projectTitle === 'string' && projectTitle.trim() !== '' && projectTitle) ||
    (model.sourceRoot !== '' && model.sourceRoot) ||
    'Camera report'
  return {
    id: meta.id,
    savedAt: meta.savedAt,
    title,
    sourceRoot: model.sourceRoot,
    clipCount: model.stats.clipCount,
    otherFileCount: model.stats.otherFileCount,
    totalSizeBytes: model.stats.totalSizeBytes,
    storedFrameBytes,
    hasThumbnails,
  }
}

// Defensive read of persisted summaries — any past/future version may have
// written them. Invalid members are dropped, never repaired.
export function normalizeReportSummaries(raw: unknown): ReportSummary[] {
  if (!Array.isArray(raw)) return []
  return raw
    .filter(isReportSummary)
    .sort((a, b) => b.savedAt - a.savedAt || (a.id < b.id ? 1 : -1))
}

function isReportSummary(value: unknown): value is ReportSummary {
  if (typeof value !== 'object' || value === null) return false
  const s = value as Record<string, unknown>
  return (
    typeof s.id === 'string' &&
    typeof s.savedAt === 'number' &&
    Number.isFinite(s.savedAt) &&
    typeof s.title === 'string' &&
    typeof s.sourceRoot === 'string' &&
    typeof s.clipCount === 'number' &&
    typeof s.otherFileCount === 'number' &&
    typeof s.totalSizeBytes === 'number' &&
    typeof s.storedFrameBytes === 'number' &&
    typeof s.hasThumbnails === 'boolean'
  )
}
```

- [ ] **Step 4: Exports** — in `packages/core/src/index.ts`, extend the `./report/` area (alphabetical: `library` before `model`):

```ts
export type { ReportSummary } from './report/library'
export { normalizeReportSummaries, summarizeReport } from './report/library'
```

- [ ] **Step 5: Tests green + all four gates** — expected: 157 + new, 0 fail.

- [ ] **Step 6: Commit**

```bash
git add packages/core/src/report/library.ts packages/core/src/report/library.test.ts packages/core/src/index.ts
git commit -m "feat(core): report library summary derivation and defensive list normalization"
```

---

### Task 2: idb v4 + report-library persistence module

**Files:**
- Modify: `apps/web/src/persistence/db.ts` (v4, two stores)
- Create: `apps/web/src/persistence/report-library.ts`

**Interfaces:**
- Consumes: `summarizeReport`, `normalizeReportSummaries`, `ReportSummary`, `ReportModel` from core; `getDb`.
- Produces (Tasks 3/4/5 rely on):
  - `saveReport(model: ReportModel<Blob>): Promise<ReportSummary>` (throws on quota — caller maps the error)
  - `listReportSummaries(): Promise<ReportSummary[]>` (newest first)
  - `loadReportModel(id: string): Promise<ReportModel<Blob> | null>`
  - `deleteReport(id: string): Promise<void>`

- [ ] **Step 1: DB v4**

In `apps/web/src/persistence/db.ts`, add to the schema interface:

```ts
  // Report Library: summaries are tiny and listed often; models carry the
  // frame Blobs and are only read when a saved report is opened.
  reportSummaries: { key: string; value: unknown }
  reportModels: { key: string; value: unknown }
```

bump to `openDB<LunaDb>('luna-web', 4, ...)` and add to `upgrade`:

```ts
      if (oldVersion < 4) {
        db.createObjectStore('reportSummaries')
        db.createObjectStore('reportModels')
      }
```

- [ ] **Step 2: Persistence module**

Create `apps/web/src/persistence/report-library.ts`:

```ts
import {
  normalizeReportSummaries,
  type ReportModel,
  type ReportSummary,
  summarizeReport,
} from '@luna-web/core'
import { getDb } from './db'

let persistenceRequested = false

// Saved reports are deliberate user data: ask the browser to protect this
// origin from storage-pressure eviction. Best-effort, once per session —
// Chromium grants silently based on engagement, no prompt.
function requestPersistence(): void {
  if (persistenceRequested) return
  persistenceRequested = true
  navigator.storage?.persist?.().catch(() => {})
}

export async function saveReport(model: ReportModel<Blob>): Promise<ReportSummary> {
  requestPersistence()
  const summary = summarizeReport(
    model,
    { id: crypto.randomUUID(), savedAt: Date.now() },
    (image) => image.size,
  )
  const db = await getDb()
  // One transaction over both stores: a summary must never exist without its
  // model (or vice versa) — quota failures abort both writes together.
  const tx = db.transaction(['reportSummaries', 'reportModels'], 'readwrite')
  void tx.objectStore('reportSummaries').put(summary, summary.id)
  void tx.objectStore('reportModels').put(model, summary.id)
  await tx.done
  return summary
}

export async function listReportSummaries(): Promise<ReportSummary[]> {
  const db = await getDb()
  return normalizeReportSummaries(await db.getAll('reportSummaries'))
}

export async function loadReportModel(id: string): Promise<ReportModel<Blob> | null> {
  const db = await getDb()
  const raw = await db.get('reportModels', id)
  // Boundary: this record is a structured clone of a ReportModel<Blob> WE
  // wrote in saveReport — validate the coarse shape, then trust it.
  if (typeof raw !== 'object' || raw === null) return null
  const model = raw as ReportModel<Blob>
  if (!Array.isArray(model.reels) || typeof model.stats !== 'object') return null
  return model
}

export async function deleteReport(id: string): Promise<void> {
  const db = await getDb()
  const tx = db.transaction(['reportSummaries', 'reportModels'], 'readwrite')
  void tx.objectStore('reportSummaries').delete(id)
  void tx.objectStore('reportModels').delete(id)
  await tx.done
}
```

(Note: "Clear local data" already deletes the whole `luna-web` DB — saved reports are covered with zero changes.)

- [ ] **Step 3: All four gates, commit**

```bash
git add apps/web/src/persistence/db.ts apps/web/src/persistence/report-library.ts
git commit -m "feat(web): report library persistence (idb v4, summary/model split)"
```

---

### Task 3: Extract shared ReportView + Save-report button

**Files:**
- Create: `apps/web/src/features/report/report-view.tsx` (JSX moved from report-workspace — styling preserved character-for-character)
- Create: `apps/web/src/features/report/save-report-button.tsx`
- Modify: `apps/web/src/features/report/report-workspace.tsx` (becomes a thin wrapper)

**Interfaces:**
- Consumes: `ReportModel<Blob>`; `ClipCard`, `StatTile`, `formatBytes/formatDuration`; `saveReport` (Task 2); `logger`.
- Produces (Task 5 relies on): `ReportView({ model, eyebrow?, actions, children? })` — `eyebrow` defaults to `'Camera Report'`; `actions` renders in the header row (right side); `children` renders between the stats card and the reel nav (the live workspace passes `<CoverForm />` there; the saved view passes nothing → read-only for free).

- [ ] **Step 1: MOVE the model-driven JSX (maintainer-styled — byte-preserving)**

`git status` first — if `report-workspace.tsx` is dirty → BLOCKED.

Create `apps/web/src/features/report/report-view.tsx` containing, moved VERBATIM from the current `report-workspace.tsx`: the `slug` helper, the `metaLine` derivation, the `multiReel` flag, the entire returned JSX (header block, stats Card, reel nav, reel sections incl. other-files table), and the `CoverLogo` component. The only permitted edits during the move:

- The component signature becomes:

```tsx
export function ReportView({
  model,
  eyebrow = 'Camera Report',
  actions,
  children,
}: {
  model: ReportModel<Blob>
  eyebrow?: string
  actions: ReactNode
  children?: ReactNode
}) {
```

- The hardcoded `Camera Report` eyebrow text becomes `{eyebrow}`.
- The header's action cluster `<CoverLogo …/><ExportButtons …/><Button Start over…/>` becomes `<CoverLogo logo={model.cover.logo} />{actions}` (CoverLogo stays inside ReportView; ExportButtons/Start-over move OUT to the callers via `actions`).
- The `<div className="mb-6"><CoverForm /></div>` block becomes `{children}`.
- Imports adjusted accordingly (ReportView must NOT import CoverForm, ExportButtons, scanStore, coverStore, or resetScan — it is pure model → JSX).

Every Tailwind class string, element structure, and text node otherwise survives unchanged — diff the moved JSX against the original to prove it.

- [ ] **Step 2: The Save-report button**

Create `apps/web/src/features/report/save-report-button.tsx`:

```tsx
import type { ReportModel } from '@luna-web/core'
import { useEffect, useState } from 'react'
import { Button } from '@/components/ui/button'
import { logger } from '@/lib/logger'
import { saveReport } from '@/persistence/report-library'

export function SaveReportButton({ model }: { model: ReportModel<Blob> }) {
  const [state, setState] = useState<'idle' | 'saving' | 'saved'>('idle')
  const [error, setError] = useState<string | null>(null)

  // A new/changed model (fresh run, cover edit) re-arms the button.
  useEffect(() => {
    setState('idle')
    setError(null)
  }, [model])

  return (
    <div className="flex items-center gap-2">
      <Button
        variant="outline"
        disabled={state !== 'idle'}
        onClick={() => {
          setState('saving')
          setError(null)
          saveReport(model)
            .then((summary) => {
              setState('saved')
              logger.info('Report saved to library', summary.title)
            })
            .catch((err) => {
              setState('idle')
              const message =
                err instanceof DOMException && err.name === 'QuotaExceededError'
                  ? 'Not enough storage space — free disk space or delete saved reports, then try again.'
                  : err instanceof Error
                    ? err.message
                    : String(err)
              setError(message)
              logger.error('Saving report failed', message)
            })
        }}
      >
        {state === 'saving' ? 'Saving…' : state === 'saved' ? 'Saved ✓' : 'Save report'}
      </Button>
      {error && <span className="text-destructive text-sm">{error}</span>}
    </div>
  )
}
```

- [ ] **Step 3: Thin workspace wrapper**

`report-workspace.tsx` keeps its selectors and `useMemo` model exactly as-is; the return becomes:

```tsx
  return (
    <ReportView
      model={model}
      actions={
        <>
          <ExportButtons report={model} />
          <SaveReportButton model={model} />
          <Button variant="outline" onClick={resetScan}>
            Start over
          </Button>
        </>
      }
    >
      <div className="mb-6">
        <CoverForm />
      </div>
    </ReportView>
  )
```

with imports pruned to what remains (buildReportModel, selectors, Button, ExportButtons, SaveReportButton, CoverForm, ReportView, resetScan, stores) — `StatTile`, `Card`, `ClipCard`, `formatBytes`, `formatDuration`, `slug`, `CoverLogo` all now live only in report-view.tsx.

- [ ] **Step 4: All four gates** (build proves the move compiles; visually the workspace must be UNCHANGED — the diff-against-original from Step 1 is the evidence). Commit:

```bash
git add apps/web/src/features/report/report-view.tsx apps/web/src/features/report/save-report-button.tsx apps/web/src/features/report/report-workspace.tsx
git commit -m "feat(web): shared ReportView extraction + explicit Save report button"
```

---

### Task 4: `/reports` list route + nav link

**Files:**
- Create: `apps/web/src/features/reports/report-library-screen.tsx`
- Create: `apps/web/src/routes/reports.index.tsx`
- Modify: `apps/web/src/components/app-shell.tsx` (one nav Link — stop-if-dirty)

**Interfaces:**
- Consumes: `listReportSummaries`, `deleteReport` (Task 2); `ReportSummary`; `formatBytes`; shadcn Card/Button/Badge; `Link` from TanStack Router.
- Produces: route `/reports` (list). NOTE: entry rows `Link` to `/reports/$reportId`, and TanStack type-checks Link targets against existing route files — so **Task 4 creates BOTH route files**: `reports.index.tsx` (real) and a minimal `reports.$reportId.tsx` stub whose component Task 5 replaces. The stub:

```tsx
import { createFileRoute } from '@tanstack/react-router'

export const Route = createFileRoute('/reports/$reportId')({
  component: () => null, // Task 5 replaces this with SavedReportScreen
})
```

- [ ] **Step 1: The library screen**

Create `apps/web/src/features/reports/report-library-screen.tsx`:

```tsx
import type { ReportSummary } from '@luna-web/core'
import { Link } from '@tanstack/react-router'
import { Trash2 } from 'lucide-react'
import { useEffect, useState } from 'react'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { formatBytes } from '@/lib/format'
import { deleteReport, listReportSummaries } from '@/persistence/report-library'

function savedAtLabel(savedAt: number): string {
  return new Date(savedAt).toLocaleString(undefined, { hour12: false })
}

export function ReportLibraryScreen() {
  const [summaries, setSummaries] = useState<ReportSummary[] | null>(null)
  const [usage, setUsage] = useState<string | null>(null)
  const [confirmingId, setConfirmingId] = useState<string | null>(null)

  useEffect(() => {
    void listReportSummaries().then(setSummaries)
    void navigator.storage?.estimate?.().then((estimate) => {
      if (estimate?.usage !== undefined && estimate.quota !== undefined) {
        setUsage(`${formatBytes(estimate.usage)} used of ${formatBytes(estimate.quota)} available`)
      }
    })
  }, [])

  return (
    <div className="mx-auto flex max-w-3xl flex-col gap-4">
      <div className="flex flex-wrap items-baseline justify-between gap-3">
        <h1 className="text-2xl font-semibold tracking-tight">Report Library</h1>
        {usage && (
          <span className="text-muted-foreground font-mono text-xs tabular-nums">{usage}</span>
        )}
      </div>

      {summaries === null ? null : summaries.length === 0 ? (
        <p className="text-muted-foreground py-12 text-center text-sm">
          No saved reports yet — finish a run and use "Save report" to keep it here.
        </p>
      ) : (
        <ul className="flex flex-col gap-2">
          {summaries.map((s) => (
            <li key={s.id}>
              <div className="bg-card hover:border-input flex items-center justify-between gap-3 rounded-lg border px-4 py-3 transition-colors">
                <Link
                  to="/reports/$reportId"
                  params={{ reportId: s.id }}
                  className="min-w-0 flex-1"
                >
                  <span className="block truncate font-medium">{s.title}</span>
                  <span className="text-muted-foreground block truncate font-mono text-xs tabular-nums">
                    {s.sourceRoot || '—'} · {s.clipCount} clips · {formatBytes(s.totalSizeBytes)} ·
                    saved {savedAtLabel(s.savedAt)}
                  </span>
                </Link>
                <div className="flex shrink-0 items-center gap-2">
                  <Badge variant={s.hasThumbnails ? 'secondary' : 'outline'}>
                    {s.hasThumbnails ? `Thumbnails · ${formatBytes(s.storedFrameBytes)}` : 'Data only'}
                  </Badge>
                  {confirmingId === s.id ? (
                    <>
                      <Button
                        variant="destructive"
                        size="sm"
                        onClick={() => {
                          void deleteReport(s.id).then(() =>
                            listReportSummaries().then((next) => {
                              setSummaries(next)
                              setConfirmingId(null)
                            }),
                          )
                        }}
                      >
                        Delete
                      </Button>
                      <Button variant="ghost" size="sm" onClick={() => setConfirmingId(null)}>
                        Cancel
                      </Button>
                    </>
                  ) : (
                    <Button
                      variant="ghost"
                      size="icon-sm"
                      aria-label={`Delete saved report ${s.title}`}
                      onClick={() => setConfirmingId(s.id)}
                    >
                      <Trash2 />
                    </Button>
                  )}
                </div>
              </div>
            </li>
          ))}
        </ul>
      )}
    </div>
  )
}
```

- [ ] **Step 2: Route files**

Create `apps/web/src/routes/reports.index.tsx`:

```tsx
import { createFileRoute } from '@tanstack/react-router'
import { ReportLibraryScreen } from '@/features/reports/report-library-screen'

export const Route = createFileRoute('/reports/')({
  component: ReportLibraryScreen,
})
```

Create the Task-5 stub `apps/web/src/routes/reports.$reportId.tsx` exactly as shown in the Interfaces note above.

- [ ] **Step 3: Nav link** (`app-shell.tsx`, stop-if-dirty, minimal diff): insert as the FIRST nav item, before Settings:

```tsx
            <Link to="/reports" className={cn(buttonVariants({ variant: 'ghost', size: 'sm' }))}>
              Reports
            </Link>
```

- [ ] **Step 4: All four gates, commit**

```bash
git add apps/web/src/features/reports/report-library-screen.tsx apps/web/src/routes/reports.index.tsx apps/web/src/routes/reports.\$reportId.tsx apps/web/src/components/app-shell.tsx
git commit -m "feat(web): Report Library route with storage usage and per-entry delete"
```

---

### Task 5: Saved-report view (read-only)

**Files:**
- Create: `apps/web/src/features/reports/saved-report-screen.tsx`
- Modify: `apps/web/src/routes/reports.$reportId.tsx` (replace the stub component)

**Interfaces:**
- Consumes: `loadReportModel` (Task 2); `ReportView` (Task 3); `ExportButtons`; `Link`; shadcn Button.
- Produces: route `/reports/$reportId` rendering the saved model read-only with working exports.

- [ ] **Step 1: The screen**

Create `apps/web/src/features/reports/saved-report-screen.tsx`:

```tsx
import type { ReportModel } from '@luna-web/core'
import { Link } from '@tanstack/react-router'
import { useEffect, useState } from 'react'
import { buttonVariants } from '@/components/ui/button'
import { ExportButtons } from '@/features/export/export-buttons'
import { ReportView } from '@/features/report/report-view'
import { cn } from '@/lib/utils'
import { loadReportModel } from '@/persistence/report-library'

export function SavedReportScreen({ reportId }: { reportId: string }) {
  const [state, setState] = useState<
    { status: 'loading' } | { status: 'missing' } | { status: 'loaded'; model: ReportModel<Blob> }
  >({ status: 'loading' })

  useEffect(() => {
    let cancelled = false
    setState({ status: 'loading' })
    void loadReportModel(reportId).then((model) => {
      if (cancelled) return
      setState(model ? { status: 'loaded', model } : { status: 'missing' })
    })
    return () => {
      cancelled = true
    }
  }, [reportId])

  if (state.status === 'loading') return null

  if (state.status === 'missing') {
    return (
      <div className="flex flex-col items-center gap-4 py-24 text-center">
        <h1 className="text-2xl font-semibold tracking-tight">Report not found</h1>
        <p className="text-muted-foreground max-w-sm text-sm">
          This saved report doesn't exist anymore — it may have been deleted.
        </p>
        <Link to="/reports" className={cn(buttonVariants({ variant: 'outline' }))}>
          Back to Reports
        </Link>
      </div>
    )
  }

  return (
    <ReportView
      model={state.model}
      eyebrow="Saved Report"
      actions={
        <>
          <ExportButtons report={state.model} />
          <Link to="/reports" className={cn(buttonVariants({ variant: 'outline' }))}>
            Back to Reports
          </Link>
        </>
      }
    />
  )
}
```

(No `children` → no CoverForm → read-only by construction; cover data still shows via ReportView's header/meta line and exports carry the saved cover.)

- [ ] **Step 2: Wire the route**

Replace `apps/web/src/routes/reports.$reportId.tsx` with:

```tsx
import { createFileRoute } from '@tanstack/react-router'
import { SavedReportScreen } from '@/features/reports/saved-report-screen'

export const Route = createFileRoute('/reports/$reportId')({
  component: RouteComponent,
})

function RouteComponent() {
  const { reportId } = Route.useParams()
  return <SavedReportScreen reportId={reportId} />
}
```

- [ ] **Step 3: All four gates, commit**

```bash
git add apps/web/src/features/reports/saved-report-screen.tsx apps/web/src/routes/reports.\$reportId.tsx
git commit -m "feat(web): read-only saved report view with re-export"
```

---

## Definition of done

- All four gates green from repo root; no changes under `apps/web/src/components/ui/` or `tools/`; the live workspace renders pixel-identical after the ReportView extraction.
- Final whole-plan review (opus), fixes + re-review, ledger close.

### Maintainer QA checklist (manual, Chromium)

1. **Save**: finish a run → "Save report" → "Saved ✓"; a log line appears under the run's operation in `/activity`. Editing the cover re-arms the button (a second save creates a second entry — expected, every run/save is unique).
2. **Library**: `/reports` lists entries newest-first with title, source, clip count, card size, saved time, and a Thumbnails/Data-only badge (thumbnail entries show their stored size). Storage usage line renders.
3. **Open**: a saved report renders read-only (no cover form), identical layout to the live workspace; PDF and CSV export from it work WITHOUT the source folder present (unplug/rename the card first — exports must still succeed, everything comes from idb).
4. **Refresh persistence**: save → hard refresh → `/reports` still lists it; open still works.
5. **Delete**: two-step per-entry delete removes it; opening its stale URL shows "Report not found".
6. **Metadata-only run** saved → tiny entry ("Data only" badge), opens with "No preview frames" placeholders, exports work.
7. **Live workspace unchanged**: the processed screen looks exactly as before (extraction is invisible), Start over still works.
8. **Clear local data** wipes the library too.
9. **Quota honesty** (optional, hard to trigger): with a nearly-full disk, a failed save shows the friendly storage message, and no orphan entry appears in `/reports` (single-transaction save).

## Self-review notes

- Decisions honored: explicit save only; no cap/eviction; `navigator.storage.persist()` on first save; read-only v1; every save a distinct entry; list fields per approval; name = Reports/Report Library.
- Type consistency: `ReportSummary` field names identical core↔app↔UI; `saveReport→summarizeReport(imageBytesOf: (img) => img.size)` matches core's injected-sizer signature; `ReportView` props match both call sites; route ids `/reports/` + `/reports/$reportId` match TanStack flat-route file names.
- Coordination: report-workspace extraction is a byte-preserving move (diff-proof step); app-shell single-Link diff; no PDF-file touches.
- Known deliberate choices: `loadReportModel` trusts the coarse shape of records we wrote (structured-clone round-trip, documented boundary); Task 4 ships a null-component stub for the detail route so each task stays independently green; storage usage uses `navigator.storage.estimate` (approximate by design).
