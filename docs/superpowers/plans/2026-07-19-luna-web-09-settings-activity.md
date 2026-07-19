# Luna Web — Plan 09: Settings + Activity Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Milestone 8 — persisted settings (worker cap, cover defaults) with schema versioning, an activity/log view with honest per-clip failure reasons, stale-recent-source UX, and the `/settings`, `/activity`, `/credits`, not-found routes.

**Architecture:** Pure, `bun test`-covered logic lands in `packages/core` (settings normalization, capped ring log). The app wires it to the browser: idb DB v2 with a `settings` store, a TanStack `settingsStore` hydrated before first render, a `logger` singleton over a TanStack store, and three new TanStack Router routes. The thumbnail pipeline gains a preview-salvage step so "no browser decoder" never masquerades as a raw decode error.

**Tech Stack:** Existing only — TypeScript 6, TanStack Store/Router, idb, shadcn (Base UI), Tailwind 4, Lucide. **Zero new dependencies.**

## Global Constraints

- **Zero new dependencies.** No `bun add` anywhere in this plan.
- **Bun only.** Gates run from the **repo root** (post-merge layout): `bun run lint && bun run typecheck && bun test && bun run build`. All four must be green before AND after every task.
- **Never touch** `apps/web/src/components/ui/` (maintainer-owned) or `tools/` (maintainer's research tooling, read-only).
- **`packages/core` stays DOM-free** — no `Blob`, `File`, `Worker`, `navigator`, `Date.now()` in core source (generic `TImage` and caller-supplied timestamps instead). `bun test` covers core only.
- **Stage by explicit file paths, never directory adds.** Run `git status` before staging; if a file you must modify has uncommitted maintainer changes, STOP and report (do not commit around it).
- **TypeScript stays at 6.0.3** (workspace ceiling — `astro check` rejects TS 7). Never edit dependency versions by hand.
- Environment: Windows, use the Git Bash tool, `cd /e/Coding/LunaApp` first (cwd drifts). Bun 1.3.14. CRLF warnings from git are benign.
- Biome formats/organizes on `bun run lint` — if lint reports fixable style issues in files you created, run `bunx biome check --write <paths>` and re-verify.
- Maintainer decisions recorded in this plan (approved at scoping): **`theme` and `defaultExport` are dropped from the Settings v1 schema** (dark-only app; two-button export toolbar needs no default). Spec §14 is amended in Task 1. `coverDefaults` never persists `date` (always seeded to today).

---

### Task 1: Core settings model + normalization (TDD) + spec §14 amendment

**Files:**
- Create: `packages/core/src/settings/model.ts`
- Create: `packages/core/src/settings/model.test.ts`
- Modify: `packages/core/src/index.ts` (add exports, alphabetical position between `scan` and `thumbs` blocks)
- Modify: `docs/superpowers/specs/2026-07-17-luna-web-design.md` (§14 interface block)

**Interfaces:**
- Consumes: `CoverFields<TImage>` from `../report/model` (fields: `projectTitle? productionCompany? dit? director? dp? date? logo?: TImage`).
- Produces (Tasks 3/6 rely on these exact names):
  - `interface Settings<TImage = unknown> { schemaVersion: 1; workerPoolCap: number; coverDefaults: Partial<CoverFields<TImage>> }`
  - `SETTINGS_SCHEMA_VERSION = 1`, `WORKER_POOL_CAP_MIN = 1`, `WORKER_POOL_CAP_MAX = 8`, `WORKER_POOL_CAP_DEFAULT = 4`
  - `defaultSettings<TImage>(): Settings<TImage>`
  - `clampWorkerPoolCap(value: unknown): number`
  - `normalizeSettings<TImage>(raw: unknown): Settings<TImage>`

- [ ] **Step 1: Amend spec §14 (docs commit)**

In `docs/superpowers/specs/2026-07-17-luna-web-design.md`, replace the `interface Settings` code block under `## 14. Settings schema, versioning & migrations` with:

```ts
interface Settings {
  schemaVersion: number         // for migrations
  workerPoolCap: number         // default 4, clamped 1–8
  coverDefaults: Partial<CoverFields>   // logo stored as Blob; `date` is never persisted (always seeded to today)
}
```

Immediately after that code block, add this paragraph:

> _Amended at Plan 09 scoping (maintainer-approved): `theme` is dropped — Luna Web is Cinema Dark, dark-only by design (visual-design spec); `defaultExport` is dropped — with a two-button export toolbar a "default export" toggle controls nothing. Either can return as a schema v2 migration if ever needed._

Commit:

```bash
git add docs/superpowers/specs/2026-07-17-luna-web-design.md
git commit -m "docs: amend spec §14 — settings v1 drops theme and defaultExport"
```

- [ ] **Step 2: Write the failing tests**

Create `packages/core/src/settings/model.test.ts`:

```ts
import { describe, expect, test } from 'bun:test'
import {
  clampWorkerPoolCap,
  defaultSettings,
  normalizeSettings,
  SETTINGS_SCHEMA_VERSION,
  WORKER_POOL_CAP_DEFAULT,
} from './model'

describe('defaultSettings', () => {
  test('returns schema v1 with default cap and empty cover defaults', () => {
    expect(defaultSettings()).toEqual({
      schemaVersion: SETTINGS_SCHEMA_VERSION,
      workerPoolCap: WORKER_POOL_CAP_DEFAULT,
      coverDefaults: {},
    })
  })

  test('returns a fresh object each call (no shared mutable state)', () => {
    const a = defaultSettings()
    const b = defaultSettings()
    expect(a).not.toBe(b)
    expect(a.coverDefaults).not.toBe(b.coverDefaults)
  })
})

describe('clampWorkerPoolCap', () => {
  test('clamps below minimum to 1', () => {
    expect(clampWorkerPoolCap(0)).toBe(1)
    expect(clampWorkerPoolCap(-3)).toBe(1)
  })

  test('clamps above maximum to 8', () => {
    expect(clampWorkerPoolCap(99)).toBe(8)
  })

  test('rounds fractional values', () => {
    expect(clampWorkerPoolCap(3.6)).toBe(4)
    expect(clampWorkerPoolCap(2.2)).toBe(2)
  })

  test('non-numbers and non-finite values fall back to the default', () => {
    expect(clampWorkerPoolCap(Number.NaN)).toBe(WORKER_POOL_CAP_DEFAULT)
    expect(clampWorkerPoolCap(Number.POSITIVE_INFINITY)).toBe(WORKER_POOL_CAP_DEFAULT)
    expect(clampWorkerPoolCap('5')).toBe(WORKER_POOL_CAP_DEFAULT)
    expect(clampWorkerPoolCap(undefined)).toBe(WORKER_POOL_CAP_DEFAULT)
  })
})

describe('normalizeSettings', () => {
  test('non-object input yields defaults', () => {
    expect(normalizeSettings(undefined)).toEqual(defaultSettings())
    expect(normalizeSettings(null)).toEqual(defaultSettings())
    expect(normalizeSettings('junk')).toEqual(defaultSettings())
    expect(normalizeSettings(42)).toEqual(defaultSettings())
  })

  test('unknown/newer schemaVersion yields defaults (spec §14 defensive load)', () => {
    expect(normalizeSettings({ schemaVersion: 99, workerPoolCap: 2 })).toEqual(defaultSettings())
    expect(normalizeSettings({ workerPoolCap: 2 })).toEqual(defaultSettings())
  })

  test('valid record passes through with cap clamped', () => {
    const logo = { marker: 'image' }
    const result = normalizeSettings<{ marker: string }>({
      schemaVersion: 1,
      workerPoolCap: 99,
      coverDefaults: { dit: 'Shaked', projectTitle: 'Luna', logo },
    })
    expect(result).toEqual({
      schemaVersion: 1,
      workerPoolCap: 8,
      coverDefaults: { dit: 'Shaked', projectTitle: 'Luna', logo },
    })
  })

  test('cover defaults drop empty strings, non-strings, and unknown keys', () => {
    const result = normalizeSettings({
      schemaVersion: 1,
      workerPoolCap: 4,
      coverDefaults: { dit: '', director: 7, bogus: 'x', dp: 'Dana' },
    })
    expect(result.coverDefaults).toEqual({ dp: 'Dana' })
  })

  test('cover defaults never carry a persisted date', () => {
    const result = normalizeSettings({
      schemaVersion: 1,
      workerPoolCap: 4,
      coverDefaults: { date: '2020-01-01', dit: 'Shaked' },
    })
    expect(result.coverDefaults).toEqual({ dit: 'Shaked' })
  })

  test('garbage coverDefaults collapses to empty object', () => {
    const result = normalizeSettings({ schemaVersion: 1, workerPoolCap: 4, coverDefaults: 'nope' })
    expect(result.coverDefaults).toEqual({})
  })
})
```

- [ ] **Step 3: Run tests to verify they fail**

Run: `cd /e/Coding/LunaApp && bun test packages/core/src/settings/model.test.ts`
Expected: FAIL — cannot resolve `./model`.

- [ ] **Step 4: Write the implementation**

Create `packages/core/src/settings/model.ts`:

```ts
import type { CoverFields } from '../report/model'

export const SETTINGS_SCHEMA_VERSION = 1

export const WORKER_POOL_CAP_MIN = 1
export const WORKER_POOL_CAP_MAX = 8
export const WORKER_POOL_CAP_DEFAULT = 4

// Settings v1 (spec §14, as amended at Plan 09 scoping): no `theme` (the app
// is dark-only by design) and no `defaultExport` (nothing for it to control).
// Adding either later is a schemaVersion bump + migration, not a breaking edit.
export interface Settings<TImage = unknown> {
  schemaVersion: typeof SETTINGS_SCHEMA_VERSION
  workerPoolCap: number
  coverDefaults: Partial<CoverFields<TImage>>
}

export function defaultSettings<TImage = unknown>(): Settings<TImage> {
  return {
    schemaVersion: SETTINGS_SCHEMA_VERSION,
    workerPoolCap: WORKER_POOL_CAP_DEFAULT,
    coverDefaults: {},
  }
}

export function clampWorkerPoolCap(value: unknown): number {
  if (typeof value !== 'number' || !Number.isFinite(value)) return WORKER_POOL_CAP_DEFAULT
  return Math.min(WORKER_POOL_CAP_MAX, Math.max(WORKER_POOL_CAP_MIN, Math.round(value)))
}

// Cover text fields that may be persisted as defaults. `date` is deliberately
// excluded — the cover date is seeded to "today" on every launch, and a stale
// persisted date is exactly the bug that seeding exists to prevent.
const COVER_DEFAULT_TEXT_KEYS = ['projectTitle', 'productionCompany', 'dit', 'director', 'dp'] as const

// Defensive load (spec §14): any record we did not write in this exact schema
// version — including newer versions from a future deploy — collapses to
// defaults rather than crashing or half-loading.
export function normalizeSettings<TImage = unknown>(raw: unknown): Settings<TImage> {
  if (typeof raw !== 'object' || raw === null) return defaultSettings<TImage>()
  const record = raw as Record<string, unknown>
  if (record.schemaVersion !== SETTINGS_SCHEMA_VERSION) return defaultSettings<TImage>()
  return {
    schemaVersion: SETTINGS_SCHEMA_VERSION,
    workerPoolCap: clampWorkerPoolCap(record.workerPoolCap),
    coverDefaults: normalizeCoverDefaults<TImage>(record.coverDefaults),
  }
}

function normalizeCoverDefaults<TImage>(value: unknown): Partial<CoverFields<TImage>> {
  if (typeof value !== 'object' || value === null) return {}
  const source = value as Record<string, unknown>
  const out: Partial<CoverFields<TImage>> = {}
  for (const key of COVER_DEFAULT_TEXT_KEYS) {
    const field = source[key]
    if (typeof field === 'string' && field !== '') out[key] = field
  }
  // Boundary: core is DOM-free, so the logo image type is opaque here — the
  // caller (app) owns TImage (a Blob in practice) and idb round-trips it.
  if (source.logo !== undefined && source.logo !== null) out.logo = source.logo as TImage
  return out
}
```

- [ ] **Step 5: Run tests to verify they pass**

Run: `bun test packages/core/src/settings/model.test.ts`
Expected: all tests in the file PASS.

- [ ] **Step 6: Export from the core index**

In `packages/core/src/index.ts`, after the `./scan/walker` exports and before the `./thumbs/model` exports, add:

```ts
export type { Settings } from './settings/model'
export {
  clampWorkerPoolCap,
  defaultSettings,
  normalizeSettings,
  SETTINGS_SCHEMA_VERSION,
  WORKER_POOL_CAP_DEFAULT,
  WORKER_POOL_CAP_MAX,
  WORKER_POOL_CAP_MIN,
} from './settings/model'
```

- [ ] **Step 7: Run all four gates**

Run: `cd /e/Coding/LunaApp && bun run lint && bun run typecheck && bun test && bun run build`
Expected: all green; test total is 121 + the new settings tests, 0 fail.

- [ ] **Step 8: Commit**

```bash
git add packages/core/src/settings/model.ts packages/core/src/settings/model.test.ts packages/core/src/index.ts
git commit -m "feat(core): settings v1 model with defensive normalization"
```

---

### Task 2: Core activity log — capped ring + formatting (TDD)

**Files:**
- Create: `packages/core/src/activity/log.ts`
- Create: `packages/core/src/activity/log.test.ts`
- Modify: `packages/core/src/index.ts` (add exports, alphabetical position — `activity` sorts first, before the `./export/csv` block)

**Interfaces:**
- Consumes: nothing (pure module; caller supplies timestamps — core stays clock-free).
- Produces (Tasks 4/7 rely on these exact names):
  - `type LogLevel = 'debug' | 'info' | 'warn' | 'error'`
  - `interface LogEntry { seq: number; timestamp: number; level: LogLevel; message: string; detail?: string }`
  - `LOG_LEVELS: readonly LogLevel[]` (ascending severity)
  - `appendLog(entries: readonly LogEntry[], entry: LogEntry, capacity: number): LogEntry[]`
  - `logLevelAtLeast(level: LogLevel, min: LogLevel): boolean`
  - `formatLogText(entries: readonly LogEntry[]): string`

- [ ] **Step 1: Write the failing tests**

Create `packages/core/src/activity/log.test.ts`:

```ts
import { describe, expect, test } from 'bun:test'
import { appendLog, formatLogText, type LogEntry, logLevelAtLeast } from './log'

function entry(seq: number, overrides: Partial<LogEntry> = {}): LogEntry {
  return { seq, timestamp: 1_700_000_000_000 + seq, level: 'info', message: `m${seq}`, ...overrides }
}

describe('appendLog', () => {
  test('appends in order under capacity', () => {
    const a = appendLog([], entry(1), 3)
    const b = appendLog(a, entry(2), 3)
    expect(b.map((e) => e.seq)).toEqual([1, 2])
  })

  test('drops oldest entries beyond capacity (ring semantics)', () => {
    let entries: LogEntry[] = []
    for (let i = 1; i <= 5; i++) entries = appendLog(entries, entry(i), 3)
    expect(entries.map((e) => e.seq)).toEqual([3, 4, 5])
  })

  test('returns a new array, never mutating the input', () => {
    const original = [entry(1)]
    const next = appendLog(original, entry(2), 10)
    expect(original).toHaveLength(1)
    expect(next).toHaveLength(2)
    expect(next).not.toBe(original)
  })

  test('capacity below one yields an empty log', () => {
    expect(appendLog([entry(1)], entry(2), 0)).toEqual([])
  })
})

describe('logLevelAtLeast', () => {
  test('orders debug < info < warn < error', () => {
    expect(logLevelAtLeast('debug', 'info')).toBe(false)
    expect(logLevelAtLeast('info', 'info')).toBe(true)
    expect(logLevelAtLeast('warn', 'info')).toBe(true)
    expect(logLevelAtLeast('error', 'warn')).toBe(true)
    expect(logLevelAtLeast('info', 'error')).toBe(false)
    expect(logLevelAtLeast('debug', 'debug')).toBe(true)
  })
})

describe('formatLogText', () => {
  test('formats ISO timestamp, padded level, and message per line', () => {
    const text = formatLogText([
      { seq: 1, timestamp: Date.UTC(2026, 6, 19, 12, 0, 0), level: 'info', message: 'Scan started' },
      { seq: 2, timestamp: Date.UTC(2026, 6, 19, 12, 0, 5), level: 'error', message: 'Boom' },
    ])
    expect(text).toBe(
      '2026-07-19T12:00:00.000Z [INFO ] Scan started\n2026-07-19T12:00:05.000Z [ERROR] Boom',
    )
  })

  test('detail lines are indented under their entry', () => {
    const text = formatLogText([
      {
        seq: 1,
        timestamp: Date.UTC(2026, 6, 19, 12, 0, 0),
        level: 'warn',
        message: 'Thumb failed',
        detail: 'NO_DECODER: aprn',
      },
    ])
    expect(text).toBe('2026-07-19T12:00:00.000Z [WARN ] Thumb failed\n    NO_DECODER: aprn')
  })

  test('empty log formats to an empty string', () => {
    expect(formatLogText([])).toBe('')
  })
})
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `bun test packages/core/src/activity/log.test.ts`
Expected: FAIL — cannot resolve `./log`.

- [ ] **Step 3: Write the implementation**

Create `packages/core/src/activity/log.ts`:

```ts
export type LogLevel = 'debug' | 'info' | 'warn' | 'error'

// Ascending severity — index order is the comparison order.
export const LOG_LEVELS: readonly LogLevel[] = ['debug', 'info', 'warn', 'error']

export interface LogEntry {
  seq: number
  timestamp: number // epoch ms, caller-supplied (core stays clock-free)
  level: LogLevel
  message: string
  detail?: string
}

// Capped append over an immutable array — the app holds entries in a TanStack
// store, so ring semantics are "new array, oldest dropped", not index juggling.
export function appendLog(
  entries: readonly LogEntry[],
  entry: LogEntry,
  capacity: number,
): LogEntry[] {
  if (capacity < 1) return []
  const next = [...entries, entry]
  return next.length > capacity ? next.slice(next.length - capacity) : next
}

export function logLevelAtLeast(level: LogLevel, min: LogLevel): boolean {
  return LOG_LEVELS.indexOf(level) >= LOG_LEVELS.indexOf(min)
}

export function formatLogText(entries: readonly LogEntry[]): string {
  return entries
    .map((e) => {
      const head = `${new Date(e.timestamp).toISOString()} [${e.level.toUpperCase().padEnd(5)}] ${e.message}`
      return e.detail !== undefined ? `${head}\n    ${e.detail}` : head
    })
    .join('\n')
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `bun test packages/core/src/activity/log.test.ts`
Expected: all tests in the file PASS.

- [ ] **Step 5: Export from the core index**

In `packages/core/src/index.ts`, add as the FIRST export block (before `./export/csv` — alphabetical):

```ts
export type { LogEntry, LogLevel } from './activity/log'
export { appendLog, formatLogText, LOG_LEVELS, logLevelAtLeast } from './activity/log'
```

- [ ] **Step 6: Run all four gates**

Run: `cd /e/Coding/LunaApp && bun run lint && bun run typecheck && bun test && bun run build`
Expected: all green, 0 fail.

- [ ] **Step 7: Commit**

```bash
git add packages/core/src/activity/log.ts packages/core/src/activity/log.test.ts packages/core/src/index.ts
git commit -m "feat(core): capped activity log with level ordering and text formatting"
```

---

### Task 3: idb v2 + settings persistence + settings store + worker-cap wiring

**Files:**
- Modify: `apps/web/src/persistence/db.ts` (DB version 2, `settings` object store)
- Create: `apps/web/src/persistence/settings.ts`
- Create: `apps/web/src/features/settings/settings-store.ts`
- Modify: `apps/web/src/main.tsx` (hydrate before first render)
- Modify: `apps/web/src/features/process/run-processing.ts` (POOL_CAP → settings-driven)
- Modify: `apps/web/src/features/report/cover-store.ts` (stale comment only)

**Interfaces:**
- Consumes: `Settings`, `defaultSettings`, `normalizeSettings` from `@luna-web/core` (Task 1); `setCoverFields(patch)` from `@/features/report/cover-store`.
- Produces (Tasks 4/6 rely on these exact names):
  - `loadSettings(): Promise<Settings<Blob>>`, `saveSettings(settings: Settings<Blob>): Promise<void>` from `@/persistence/settings`
  - `settingsStore: Store<Settings<Blob>>`, `hydrateSettings(): Promise<void>`, `updateSettings(patch: Partial<Omit<Settings<Blob>, 'schemaVersion'>>): Promise<void>` from `@/features/settings/settings-store`
  - `poolSizeFor(itemCount)` keeps its signature but now reads `settingsStore.state.workerPoolCap`; the `POOL_CAP` export is REMOVED (verified: no other usages exist).

- [ ] **Step 1: Bump the DB to v2 with a `settings` store**

Replace the contents of `apps/web/src/persistence/db.ts` with:

```ts
import { type DBSchema, type IDBPDatabase, openDB } from 'idb'
import type { StoredRecentSource } from './recent-sources'

interface LunaDb extends DBSchema {
  recentSources: { key: number; value: StoredRecentSource }
  // Value is `unknown` on purpose: records may have been written by any past
  // or future app version — normalizeSettings() is the only trusted reader.
  settings: { key: string; value: unknown }
}

let dbPromise: Promise<IDBPDatabase<LunaDb>> | null = null

export function getDb(): Promise<IDBPDatabase<LunaDb>> {
  dbPromise ??= openDB<LunaDb>('luna-web', 2, {
    upgrade(db, oldVersion) {
      // Guard each store by the version that introduced it: fresh installs
      // enter with oldVersion 0 and must create everything.
      if (oldVersion < 1) db.createObjectStore('recentSources', { autoIncrement: true })
      if (oldVersion < 2) db.createObjectStore('settings')
    },
  })
  return dbPromise
}
```

- [ ] **Step 2: Settings persistence module**

Create `apps/web/src/persistence/settings.ts`:

```ts
import { normalizeSettings, type Settings } from '@luna-web/core'
import { getDb } from './db'

const SETTINGS_KEY = 'app'

export async function loadSettings(): Promise<Settings<Blob>> {
  const db = await getDb()
  return normalizeSettings<Blob>(await db.get('settings', SETTINGS_KEY))
}

export async function saveSettings(settings: Settings<Blob>): Promise<void> {
  const db = await getDb()
  await db.put('settings', settings, SETTINGS_KEY)
}
```

- [ ] **Step 3: Settings store with hydration**

Create `apps/web/src/features/settings/settings-store.ts`:

```ts
import { defaultSettings, type Settings } from '@luna-web/core'
import { Store } from '@tanstack/store'
import { setCoverFields } from '@/features/report/cover-store'
import { loadSettings, saveSettings } from '@/persistence/settings'

export const settingsStore = new Store<Settings<Blob>>(defaultSettings<Blob>())

// Called once before first render (main.tsx): the cover form reads its store
// once per mount, so persisted cover defaults must land before React does.
export async function hydrateSettings(): Promise<void> {
  const loaded = await loadSettings()
  settingsStore.setState(() => loaded)
  if (Object.keys(loaded.coverDefaults).length > 0) {
    // Seed cover fields from defaults. `date` is never part of coverDefaults
    // (normalizeSettings strips it) so the today-seed in coverStore survives.
    setCoverFields(loaded.coverDefaults)
  }
}

export async function updateSettings(
  patch: Partial<Omit<Settings<Blob>, 'schemaVersion'>>,
): Promise<void> {
  settingsStore.setState((s) => ({ ...s, ...patch }))
  await saveSettings(settingsStore.state)
}
```

- [ ] **Step 4: Hydrate before first render**

In `apps/web/src/main.tsx`, replace the final `createRoot(...)` call with:

```ts
// Hydrate persisted settings before the first render: the cover form is
// read-once-per-mount and pool sizing reads the store synchronously. A failed
// idb read must never block boot — defaults are already in the store.
void hydrateSettings()
  .catch(() => {})
  .then(() => {
    createRoot(rootElement).render(
      <StrictMode>
        <RouterProvider router={router} />
      </StrictMode>,
    )
  })
```

and add the import:

```ts
import { hydrateSettings } from './features/settings/settings-store'
```

- [ ] **Step 5: Worker cap reads settings**

In `apps/web/src/features/process/run-processing.ts`:

Remove the line `export const POOL_CAP = 4` and replace `poolSizeFor` with:

```ts
export function poolSizeFor(itemCount: number): number {
  // Settings-driven cap (spec §14, default 4, clamped 1–8 at the source);
  // hardwareConcurrency and the item count still bound it below that.
  const cap = settingsStore.state.workerPoolCap
  return Math.max(1, Math.min(cap, navigator.hardwareConcurrency || 2, itemCount))
}
```

Add the import:

```ts
import { settingsStore } from '../settings/settings-store'
```

(Verified at plan time: `POOL_CAP` has no consumers outside this file. If grep finds one now, STOP and report — do not improvise.)

- [ ] **Step 6: Retire the stale comment in cover-store.ts**

In `apps/web/src/features/report/cover-store.ts`, replace the comment line
`// "Start over" (resetScan) must never clobber it. Settings-persisted defaults`
and its continuation `// arrive in Plan 08.` with:

```ts
// "Start over" (resetScan) must never clobber it. Persisted cover defaults
// are seeded into this store at boot by hydrateSettings() (Plan 09).
```

- [ ] **Step 7: Run all four gates**

Run: `cd /e/Coding/LunaApp && bun run lint && bun run typecheck && bun test && bun run build`
Expected: all green (no new tests — browser-coupled code; core-only test policy, spec §17).

- [ ] **Step 8: Commit**

```bash
git add apps/web/src/persistence/db.ts apps/web/src/persistence/settings.ts apps/web/src/features/settings/settings-store.ts apps/web/src/main.tsx apps/web/src/features/process/run-processing.ts apps/web/src/features/report/cover-store.ts
git commit -m "feat(web): persisted settings (idb v2) with boot hydration and settings-driven worker cap"
```

---

### Task 4: App logger + pipeline instrumentation + honest no-decoder handling

**Files:**
- Create: `apps/web/src/lib/logger.ts`
- Modify: `apps/web/src/features/scan/run-scan.ts` (scan lifecycle logs)
- Modify: `apps/web/src/features/process/run-processing.ts` (metadata pass logs)
- Modify: `apps/web/src/features/process/run-thumbnails.ts` (logs + preview-salvage cascade + `.rtn` size cap)
- Modify: `apps/web/src/features/export/exporter.ts` (export logs)
- Modify: `apps/web/src/features/settings/settings-store.ts` (settings-saved log)

**Interfaces:**
- Consumes: `appendLog`, `LogEntry`, `LogLevel` from `@luna-web/core` (Task 2); existing pipeline internals (`guardedUpdate`, `setThumbStatus`, `failClip`, `finishClip`, `buildPreviewFrame`, `cascadedIds`).
- Produces (Task 7 relies on these exact names):
  - `activityStore: Store<LogEntry[]>`, `logger.{debug,info,warn,error}(message: string, detail?: string)`, `clearActivity(): void` from `@/lib/logger`

- [ ] **Step 1: The logger singleton**

Create `apps/web/src/lib/logger.ts`:

```ts
import { appendLog, type LogEntry, type LogLevel } from '@luna-web/core'
import { Store } from '@tanstack/store'

// Spec §8.14: in-memory ring buffer + console mirror. No remote sink, ever.
const CAPACITY = 500

let seq = 0

export const activityStore = new Store<LogEntry[]>([])

function write(level: LogLevel, message: string, detail?: string): void {
  seq += 1
  const entry: LogEntry = {
    seq,
    timestamp: Date.now(),
    level,
    message,
    ...(detail !== undefined ? { detail } : {}),
  }
  activityStore.setState((entries) => appendLog(entries, entry, CAPACITY))
  const line = detail !== undefined ? `${message} — ${detail}` : message
  if (level === 'error') console.error(`[luna] ${line}`)
  else if (level === 'warn') console.warn(`[luna] ${line}`)
  else console.log(`[luna] ${line}`)
}

export const logger = {
  debug: (message: string, detail?: string) => write('debug', message, detail),
  info: (message: string, detail?: string) => write('info', message, detail),
  warn: (message: string, detail?: string) => write('warn', message, detail),
  error: (message: string, detail?: string) => write('error', message, detail),
}

export function clearActivity(): void {
  activityStore.setState(() => [])
}
```

- [ ] **Step 2: Instrument the scan lifecycle**

In `apps/web/src/features/scan/run-scan.ts` add `import { logger } from '@/lib/logger'` and:

- In `scanFrom`, in the permission-denied branch, before `scanStore.setState`: `logger.warn(`Read permission denied for "${handle.name}"`)`
- Right after the `scanStore.setState(() => ({ ...initialScanState, phase: 'scanning', ... }))` line: `logger.info(`Scanning "${handle.name}"…`)`
- In the success `setState` block's statement position (immediately after that `setState` call): `logger.info(`Scan of "${handle.name}" complete`, `${result.clips.length} clips, ${result.raw.length} RAW notices`)`
- In the catch block, before `scanStore.setState`: `logger.error('Scan failed', err instanceof Error ? err.message : String(err))`
- In `resetScan`, first line: `logger.debug('Start over — scan state reset')`

- [ ] **Step 3: Instrument the metadata pass**

In `apps/web/src/features/process/run-processing.ts` add `import { logger } from '@/lib/logger'` and:

- In `startProcessing`, immediately after the initial `scanStore.setState`: `logger.info('Metadata pass started', `${clips.length} clips, ${poolSizeFor(clips.length)} workers`)`
- In `onItemFailure`, after computing `message`: `logger.warn(`Metadata failed for ${clip.fileName}`, message)`
- After the `runPool` await succeeds (immediately before `if (run !== currentRun) return`): `logger.info('Metadata pass complete')`
- In BOTH catch blocks, after computing `message`: `logger.error('Processing failed', message)`

- [ ] **Step 4: Instrument thumbnails + honest no-decoder path**

In `apps/web/src/features/process/run-thumbnails.ts` add `import { logger } from '@/lib/logger'` and make these changes:

**4a — route-count log.** After the routing `for` loop (before the all-empty early return):

```ts
logger.info(
  'Thumbnail pass started',
  `${mediabunnyClips.length} via WebCodecs, ${ffmpegClips.length} via ffmpeg, ${previewClips.length} via embedded preview, ${noPreviewClips.length} placeholders`,
)
```

**4b — extract the decoder-failure predicate.** Above `startThumbnails`, add:

```ts
// A decoder/container failure is a routing fact, not a clip defect — it feeds
// the cascade (mediabunny → ffmpeg) and the preview salvage (ffmpeg → embedded
// preview). Timeouts and I/O errors are real failures and must NOT match.
function isDecoderFailure(message: string): boolean {
  return message.includes('NO_DECODER') || /format|recognized|container/i.test(message)
}
```

and change the mediabunny `onItemFailure` condition from
`if (message.includes('NO_DECODER') || /format|recognized|container/i.test(message)) {`
to `if (isDecoderFailure(message)) {`, adding inside that branch:
`logger.debug(`Cascading ${clip.fileName} to ffmpeg`, message)`
and in the else branch: `logger.warn(`Thumbnails failed for ${clip.fileName}`, message)`.

**4c — preview salvage after ffmpeg.** Above the `const ffmpegQueue = ...` line, add:

```ts
// Clips that BOTH decoders rejected (e.g. ProRes RAW whose metadata pass
// failed, so codec routing never sent them to the preview path) get one last
// chance: the embedded tail preview. Only decoder failures qualify — real
// errors stay failed.
const salvage: ClipRef[] = []
```

Replace the ffmpeg pool's `onItemFailure` with:

```ts
onItemFailure: (clip, err) => {
  const message = err instanceof Error ? err.message : String(err)
  if (cascadedIds.has(clip.id) && isDecoderFailure(message)) {
    salvage.push(clip)
    logger.info(`No browser decoder for ${clip.fileName} — trying embedded preview`)
    setThumbStatus(run, clip.id, 'queued')
  } else {
    logger.warn(`Thumbnails failed for ${clip.fileName}`, message)
    failClip(run, clip.id, message)
  }
},
```

**4d — preview queue includes salvage.** Replace the preview-pool block's gate and item source: change

```ts
if (previewClips.length > 0 && isRunCurrent(run)) {
  await runPool<object, ClipRef, ThumbnailFrame<Blob>>(
    previewClips,
```

to

```ts
const previewQueue = [...previewClips, ...salvage]
if (previewQueue.length > 0 && isRunCurrent(run)) {
  await runPool<object, ClipRef, ThumbnailFrame<Blob>>(
    previewQueue,
```

and in that pool's `.catch` sweep, change `for (const clip of previewClips) {` to `for (const clip of previewQueue) {`.

**4e — honest placeholder + `.rtn` cap.** Replace `buildPreviewFrame` with:

```ts
// A `.rtn` is a ~50 KB thumbnail sidecar; anything huge is not a .rtn and must
// not be slurped into memory whole (P8 final-review carry-forward).
const RTN_MAX_BYTES = 8 * 1024 * 1024

async function buildPreviewFrame(clip: ClipRef): Promise<ThumbnailFrame<Blob>> {
  let preview: EmbeddedPreview | null
  if (clip.extension === '.crm') {
    preview = await extractCrmPreview(await clip.file.getFile())
  } else if (clip.extension === '.r3d') {
    if (clip.previewSidecar) {
      const sidecar = await clip.previewSidecar.getFile()
      if (sidecar.size > RTN_MAX_BYTES) {
        logger.warn(`${clip.fileName}: .rtn sidecar is ${sidecar.size} bytes — too large, skipping`)
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
    logger.info(`No embedded preview in ${clip.fileName} — placeholder frame used`)
    return noDecoderFrame()
  }
  return await previewToFrame(preview)
}
```

**4f — aggregate placeholder log.** Replace the `for (const clip of noPreviewClips)` loop with:

```ts
if (noPreviewClips.length > 0) {
  logger.info(
    `${noPreviewClips.length} clip(s) have no browser decode path (BRAW) — placeholder frames used`,
  )
  for (const clip of noPreviewClips) {
    finishClip(run, clip.id, [noDecoderFrame()])
  }
}
```

**4g — pass-complete log.** Immediately before the final `guardedUpdate(run, (s) => ({ ...s, phase: 'processed' }))`: `logger.info('Thumbnail pass complete')`

- [ ] **Step 5: Instrument exports and settings saves**

In `apps/web/src/features/export/exporter.ts`, add `import { logger } from '@/lib/logger'` and replace `runExport` with:

```ts
export async function runExport(exporter: Exporter, report: ReportModel<Blob>): Promise<void> {
  logger.info(`Export started: ${exporter.label}`)
  try {
    const blob = await exporter.generate(report)
    await saveBlob(
      blob,
      reportFileName(report.cover.projectTitle, report.cover.date, exporter.extension),
      exporter.mime,
    )
    logger.info(`Export finished: ${exporter.label}`)
  } catch (err) {
    logger.error(
      `Export failed: ${exporter.label}`,
      err instanceof Error ? err.message : String(err),
    )
    throw err
  }
}
```

In `apps/web/src/features/settings/settings-store.ts`, add `import { logger } from '@/lib/logger'` and in `updateSettings`, after `await saveSettings(...)`: `logger.debug('Settings saved')`

- [ ] **Step 6: Run all four gates**

Run: `cd /e/Coding/LunaApp && bun run lint && bun run typecheck && bun test && bun run build`
Expected: all green, 0 fail.

- [ ] **Step 7: Commit**

```bash
git add apps/web/src/lib/logger.ts apps/web/src/features/scan/run-scan.ts apps/web/src/features/process/run-processing.ts apps/web/src/features/process/run-thumbnails.ts apps/web/src/features/export/exporter.ts apps/web/src/features/settings/settings-store.ts
git commit -m "feat(web): activity logger, pipeline instrumentation, preview salvage for undecodable clips"
```

---

### Task 5: Stale recent-source UX

**Files:**
- Modify: `apps/web/src/persistence/recent-sources.ts` (`stale` flag + `markSourceStale`)
- Modify: `apps/web/src/features/scan/run-scan.ts` (`recentKey` param, NotFoundError mapping, friendlier permission copy)
- Modify: `apps/web/src/features/scan/recent-list.tsx` (stale rendering, key pass-through, re-pick)

**Interfaces:**
- Consumes: `logger` (Task 4); existing `scanFrom`, `pickAndScan`, `listRecentSources`, `forgetSource`.
- Produces: `StoredRecentSource` gains `stale?: boolean`; `markSourceStale(key: number): Promise<void>`; `scanFrom(handle, recentKey?: number)`.

- [ ] **Step 1: Persist the stale flag**

In `apps/web/src/persistence/recent-sources.ts`, add `stale?: boolean` to `StoredRecentSource`:

```ts
export interface StoredRecentSource {
  name: string
  handle: FileSystemDirectoryHandle
  lastUsedAt: number
  stale?: boolean
}
```

and append at the end of the file:

```ts
// Spec §15: a handle that turned out to be dead (folder moved/renamed/removed)
// is marked, not deleted — the user should see what broke and choose to re-pick
// or remove it. A successful re-pick of the same folder self-heals via the
// isSameEntry dedupe in rememberSource (fresh entry replaces the stale one).
export async function markSourceStale(key: number): Promise<void> {
  const db = await getDb()
  const existing = await db.get('recentSources', key)
  if (existing) await db.put('recentSources', { ...existing, stale: true }, key)
}
```

- [ ] **Step 2: Map handle failures to human copy in scanFrom**

In `apps/web/src/features/scan/run-scan.ts`:

Add `markSourceStale` to the recent-sources import. Change the signature to:

```ts
export async function scanFrom(handle: FileSystemDirectoryHandle, recentKey?: number): Promise<void> {
```

Replace the permission-denied error string with:

```ts
      error: `Luna needs read permission for "${handle.name}". Pick the folder again to re-authorize.`,
```

Replace the catch block with:

```ts
  } catch (err) {
    // A recent-source handle whose folder was moved/renamed/removed throws
    // NotFoundError (possibly mid-walk). Spec §15: mark the entry stale and
    // say what happened — never surface a raw DOMException message.
    if (err instanceof DOMException && err.name === 'NotFoundError') {
      if (recentKey !== undefined) markSourceStale(recentKey).catch(() => {})
      logger.warn(`Recent folder "${handle.name}" is no longer accessible`)
      scanStore.setState((s) => ({
        ...s,
        phase: 'error',
        error: `Luna can't find "${handle.name}" anymore — it may have been moved, renamed, or unplugged. Pick the folder again to continue.`,
      }))
      return
    }
    logger.error('Scan failed', err instanceof Error ? err.message : String(err))
    scanStore.setState((s) => ({
      ...s,
      phase: 'error',
      error: err instanceof Error ? err.message : String(err),
    }))
    return
  }
```

(Note: this replaces Task 4's catch-block `logger.error` placement — the error log now lives on the generic path only; the NotFoundError path logs a warn. If Task 4 already landed its version, adjust to this final shape.)

- [ ] **Step 3: Render stale entries and pass keys**

Replace the `<li>` body in `apps/web/src/features/scan/recent-list.tsx` so stale entries are visually distinct, re-pick via the picker, and fresh entries pass their key to `scanFrom`:

```tsx
        {entries.map((e) => (
          <li key={e.key}>
            <div className="bg-card hover:border-input flex items-center justify-between rounded-lg border px-3 py-2 transition-colors">
              <button
                type="button"
                className="flex min-w-0 items-center gap-2 text-left text-sm"
                onClick={() => void (e.stale ? pickAndScan() : scanFrom(e.handle, e.key))}
              >
                {e.stale ? (
                  <TriangleAlert className="size-4 shrink-0 text-amber-500" />
                ) : (
                  <FolderOpen className="text-muted-foreground size-4 shrink-0" />
                )}
                <span className="min-w-0">
                  <span className={e.stale ? 'text-muted-foreground truncate block' : 'truncate block'}>
                    {e.name}
                  </span>
                  {e.stale && (
                    <span className="text-muted-foreground block text-xs">
                      Folder unavailable — click to pick again
                    </span>
                  )}
                </span>
              </button>
              <Button
                variant="ghost"
                size="icon-sm"
                aria-label={`Remove ${e.name} from recent folders`}
                onClick={() => {
                  void forgetSource(e.key).then(() => listRecentSources().then(setEntries))
                }}
              >
                <X />
              </Button>
            </div>
          </li>
        ))}
```

Update the imports: add `TriangleAlert` to the lucide import and `pickAndScan` to the `./run-scan` import.

COORDINATION NOTE: `recent-list.tsx` was restyled by the maintainer (landing hero rework, commits c4365e1/67cb497). Run `git status` first; preserve the current styling classes exactly as they exist on disk — the snippet above is based on the file at plan time; if the maintainer's version has drifted, port ONLY the behavioral deltas (stale icon/copy, `pickAndScan` branch, `scanFrom(e.handle, e.key)`).

- [ ] **Step 4: Run all four gates**

Run: `cd /e/Coding/LunaApp && bun run lint && bun run typecheck && bun test && bun run build`
Expected: all green, 0 fail.

- [ ] **Step 5: Commit**

```bash
git add apps/web/src/persistence/recent-sources.ts apps/web/src/features/scan/run-scan.ts apps/web/src/features/scan/recent-list.tsx
git commit -m "feat(web): stale recent-source detection with friendly re-pick flow"
```

---

### Task 6: `/settings` route + header navigation

**Files:**
- Create: `apps/web/src/features/settings/settings-screen.tsx`
- Create: `apps/web/src/routes/settings.tsx`
- Modify: `apps/web/src/components/app-shell.tsx` (nav links)

**Interfaces:**
- Consumes: `settingsStore`, `updateSettings` (Task 3); `clampWorkerPoolCap`, `WORKER_POOL_CAP_MIN/MAX`, `CoverFields` from `@luna-web/core`; `coverStore` from `@/features/report/cover-store`; shadcn `Card`, `Input`, `Label`, `Button`, `Separator` (existing `components/ui` — read their exports before use; do NOT modify them).
- Produces: route `/settings`; `SettingsScreen` component.

- [ ] **Step 1: The settings screen**

Create `apps/web/src/features/settings/settings-screen.tsx`:

```tsx
import { clampWorkerPoolCap, type CoverFields, WORKER_POOL_CAP_MAX, WORKER_POOL_CAP_MIN } from '@luna-web/core'
import { useSelector } from '@tanstack/react-store'
import { Button } from '@/components/ui/button'
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '@/components/ui/card'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import { coverStore } from '@/features/report/cover-store'
import { settingsStore, updateSettings } from './settings-store'

// Persistable subset of the current cover: text fields that are non-empty,
// plus the logo. `date` is deliberately never saved (today-seeded each boot).
function coverDefaultsFrom(cover: CoverFields<Blob>): Partial<CoverFields<Blob>> {
  const out: Partial<CoverFields<Blob>> = {}
  for (const key of ['projectTitle', 'productionCompany', 'dit', 'director', 'dp'] as const) {
    const value = cover[key]
    if (typeof value === 'string' && value.trim() !== '') out[key] = value
  }
  if (cover.logo) out.logo = cover.logo
  return out
}

const DEFAULT_LABELS: ReadonlyArray<[keyof CoverFields<Blob> & string, string]> = [
  ['projectTitle', 'Project'],
  ['productionCompany', 'Production'],
  ['dit', 'DIT'],
  ['director', 'Director'],
  ['dp', 'DP'],
]

export function SettingsScreen() {
  const workerPoolCap = useSelector(settingsStore, (s) => s.workerPoolCap)
  const coverDefaults = useSelector(settingsStore, (s) => s.coverDefaults)
  const cores = navigator.hardwareConcurrency || 2

  const savedLines = DEFAULT_LABELS.filter(([key]) => typeof coverDefaults[key] === 'string')
  const hasDefaults = savedLines.length > 0 || coverDefaults.logo !== undefined

  return (
    <div className="mx-auto flex max-w-2xl flex-col gap-6">
      <h1 className="text-2xl font-semibold tracking-tight">Settings</h1>

      <Card>
        <CardHeader>
          <CardTitle>Processing</CardTitle>
          <CardDescription>
            Parallel workers for metadata and thumbnail decoding. Bounded by your CPU ({cores}{' '}
            cores detected) — higher is not always faster.
          </CardDescription>
        </CardHeader>
        <CardContent>
          <div className="flex items-center gap-3">
            <Label htmlFor="worker-cap" className="shrink-0">
              Worker cap
            </Label>
            <Input
              id="worker-cap"
              type="number"
              min={WORKER_POOL_CAP_MIN}
              max={WORKER_POOL_CAP_MAX}
              defaultValue={workerPoolCap}
              className="w-24"
              onBlur={(event) => {
                const next = clampWorkerPoolCap(event.currentTarget.valueAsNumber)
                event.currentTarget.value = String(next)
                if (next !== settingsStore.state.workerPoolCap)
                  void updateSettings({ workerPoolCap: next })
              }}
            />
            <span className="text-muted-foreground text-sm">
              {WORKER_POOL_CAP_MIN}–{WORKER_POOL_CAP_MAX}, applies to the next run
            </span>
          </div>
        </CardContent>
      </Card>

      <Card>
        <CardHeader>
          <CardTitle>Report defaults</CardTitle>
          <CardDescription>
            Cover fields pre-filled on every launch. The report date always defaults to today and
            is never saved.
          </CardDescription>
        </CardHeader>
        <CardContent className="flex flex-col gap-4">
          {hasDefaults ? (
            <dl className="grid grid-cols-[auto_1fr] gap-x-4 gap-y-1 text-sm">
              {savedLines.map(([key, label]) => (
                <div key={key} className="contents">
                  <dt className="text-muted-foreground">{label}</dt>
                  <dd className="truncate font-mono">{coverDefaults[key] as string}</dd>
                </div>
              ))}
              {coverDefaults.logo !== undefined && (
                <div className="contents">
                  <dt className="text-muted-foreground">Logo</dt>
                  <dd className="font-mono">saved</dd>
                </div>
              )}
            </dl>
          ) : (
            <p className="text-muted-foreground text-sm">No defaults saved yet.</p>
          )}
          <div className="flex items-center gap-2">
            <Button
              variant="outline"
              onClick={() => void updateSettings({ coverDefaults: coverDefaultsFrom(coverStore.state) })}
            >
              Save current cover as defaults
            </Button>
            <Button
              variant="ghost"
              disabled={!hasDefaults}
              onClick={() => void updateSettings({ coverDefaults: {} })}
            >
              Clear defaults
            </Button>
          </div>
        </CardContent>
      </Card>
    </div>
  )
}
```

VERIFY-DON'T-ASSUME: before writing the screen, read `apps/web/src/components/ui/card.tsx` and confirm the exported subcomponent names (`Card`, `CardHeader`, `CardTitle`, `CardDescription`, `CardContent`). If base-nova names differ, adapt the JSX to the real exports — never edit the ui component.

- [ ] **Step 2: The route file**

Create `apps/web/src/routes/settings.tsx`:

```tsx
import { createFileRoute } from '@tanstack/react-router'
import { SettingsScreen } from '@/features/settings/settings-screen'

export const Route = createFileRoute('/settings')({
  component: SettingsScreen,
})
```

(`tsr generate` runs inside the typecheck/build scripts — no manual routeTree step needed.)

- [ ] **Step 3: Header navigation**

In `apps/web/src/components/app-shell.tsx`, add `import { Link } from '@tanstack/react-router'` and replace the `<nav>` block with the version below. This task adds ONLY the Settings link — the router type-checks `Link` targets against existing route files, so the `/activity` and `/credits` links must wait for Task 7 (which finalizes this nav). `/docs` stays a plain `<a>`: it is edge-routed to the docs Worker, never a SPA route (spec §8.13).

```tsx
          <nav className="text-muted-foreground flex items-center gap-2 text-sm">
            <Link to="/settings" className={cn(buttonVariants({ variant: 'ghost', size: 'sm' }))}>
              Settings
            </Link>
            <a href="/docs" className={cn(buttonVariants({ variant: 'ghost', size: 'sm' }))}>
              Docs
            </a>
            <span className="tabular-nums">v{__APP_VERSION__}</span>
          </nav>
```

COORDINATION NOTE: `app-shell.tsx` is maintainer-styled. `git status` first; if it has uncommitted changes, STOP and report. Keep the diff to exactly the import line + the nav block.

- [ ] **Step 4: Run all four gates**

Run: `cd /e/Coding/LunaApp && bun run lint && bun run typecheck && bun test && bun run build`
Expected: all green, 0 fail.

- [ ] **Step 5: Commit**

```bash
git add apps/web/src/features/settings/settings-screen.tsx apps/web/src/routes/settings.tsx apps/web/src/components/app-shell.tsx
git commit -m "feat(web): settings route — worker cap and persisted report defaults"
```

---

### Task 7: `/activity` + `/credits` routes, not-found page, nav completion

**Files:**
- Create: `apps/web/src/features/activity/activity-screen.tsx`
- Create: `apps/web/src/routes/activity.tsx`
- Create: `apps/web/src/routes/credits.tsx`
- Modify: `apps/web/src/routes/__root.tsx` (`notFoundComponent`)
- Modify: `apps/web/src/components/app-shell.tsx` (add Activity link + version→credits link)

**Interfaces:**
- Consumes: `activityStore`, `logger`, `clearActivity` (Task 4); `formatLogText`, `logLevelAtLeast`, `LogLevel` from `@luna-web/core` (Task 2); `saveBlob(blob, fileName, mime)` from `@/features/export/save`; shadcn `Button`; `Logo` from `@/components/logo`; `__APP_VERSION__` global.
- Produces: routes `/activity`, `/credits`, root `notFoundComponent`.

- [ ] **Step 1: The activity screen**

Create `apps/web/src/features/activity/activity-screen.tsx`:

```tsx
import { formatLogText, type LogLevel, logLevelAtLeast } from '@luna-web/core'
import { useSelector } from '@tanstack/react-store'
import { Download, Trash2 } from 'lucide-react'
import { useState } from 'react'
import { Button } from '@/components/ui/button'
import { activityStore, clearActivity } from '@/lib/logger'
import { saveBlob } from '@/features/export/save'

const FILTERS: ReadonlyArray<{ min: LogLevel; label: string }> = [
  { min: 'debug', label: 'All' },
  { min: 'info', label: 'Info' },
  { min: 'warn', label: 'Warnings' },
  { min: 'error', label: 'Errors' },
]

const LEVEL_CLASS: Record<LogLevel, string> = {
  debug: 'text-muted-foreground',
  info: 'text-foreground',
  warn: 'text-amber-500',
  error: 'text-destructive',
}

function timeOf(timestamp: number): string {
  return new Date(timestamp).toLocaleTimeString(undefined, { hour12: false })
}

export function ActivityScreen() {
  const entries = useSelector(activityStore, (s) => s)
  const [minLevel, setMinLevel] = useState<LogLevel>('info')
  const visible = entries.filter((e) => logLevelAtLeast(e.level, minLevel))

  return (
    <div className="mx-auto flex max-w-3xl flex-col gap-4">
      <div className="flex flex-wrap items-center justify-between gap-3">
        <h1 className="text-2xl font-semibold tracking-tight">Activity</h1>
        <div className="flex items-center gap-2">
          {FILTERS.map((f) => (
            <Button
              key={f.min}
              variant={minLevel === f.min ? 'secondary' : 'ghost'}
              size="sm"
              onClick={() => setMinLevel(f.min)}
            >
              {f.label}
            </Button>
          ))}
          <Button
            variant="outline"
            size="sm"
            disabled={entries.length === 0}
            onClick={() =>
              void saveBlob(
                new Blob([formatLogText(entries)], { type: 'text/plain' }),
                `luna-activity-${new Date().toISOString().slice(0, 10)}.txt`,
                'text/plain',
              )
            }
          >
            <Download /> Download
          </Button>
          <Button
            variant="ghost"
            size="sm"
            disabled={entries.length === 0}
            onClick={clearActivity}
            aria-label="Clear activity log"
          >
            <Trash2 />
          </Button>
        </div>
      </div>

      {visible.length === 0 ? (
        <p className="text-muted-foreground py-12 text-center text-sm">
          {entries.length === 0
            ? 'Nothing logged yet — scan a folder and activity will show up here.'
            : 'No entries at this level.'}
        </p>
      ) : (
        <ol className="flex flex-col-reverse gap-1 font-mono text-xs">
          {visible.map((e) => (
            <li key={e.seq} className="rounded px-2 py-1 leading-relaxed">
              <span className="text-muted-foreground tabular-nums">{timeOf(e.timestamp)}</span>{' '}
              <span className={`${LEVEL_CLASS[e.level]} uppercase`}>{e.level}</span>{' '}
              <span>{e.message}</span>
              {e.detail !== undefined && (
                <div className="text-muted-foreground truncate pl-16" title={e.detail}>
                  {e.detail}
                </div>
              )}
            </li>
          ))}
        </ol>
      )}
    </div>
  )
}
```

(`flex-col-reverse` renders newest entries at the top while keeping the array append-ordered.)

- [ ] **Step 2: Route files**

Create `apps/web/src/routes/activity.tsx`:

```tsx
import { createFileRoute } from '@tanstack/react-router'
import { ActivityScreen } from '@/features/activity/activity-screen'

export const Route = createFileRoute('/activity')({
  component: ActivityScreen,
})
```

Create `apps/web/src/routes/credits.tsx` (static screen, kept route-inline):

```tsx
import { createFileRoute } from '@tanstack/react-router'
import { Logo } from '@/components/logo'

const STACK: ReadonlyArray<[string, string]> = [
  ['mediainfo.js', 'container + camera metadata (WASM)'],
  ['mediabunny + @mediabunny/prores', 'WebCodecs decode, ProRes via TurboRes'],
  ['ffmpeg.wasm', 'fallback decode engine (CDN-cached)'],
  ['React 19 + TanStack Router / Store / Form', 'application framework'],
  ['Tailwind CSS 4 + shadcn/ui (Base UI)', 'interface'],
  ['react-pdf', 'PDF report rendering'],
  ['idb', 'IndexedDB persistence'],
  ['comlink', 'worker RPC'],
  ['Lucide + Geist', 'icons + type'],
  ['Bun + Vite + Biome', 'build tooling'],
]

function CreditsScreen() {
  return (
    <div className="mx-auto flex max-w-2xl flex-col items-center gap-8 text-center">
      <Logo className="h-16 w-auto" />
      <div>
        <h1 className="text-2xl font-semibold tracking-tight">Luna Web</h1>
        <p className="text-muted-foreground mt-1 tabular-nums">v{__APP_VERSION__}</p>
      </div>
      <p className="text-muted-foreground max-w-prose text-sm leading-relaxed">
        A camera report tool for DITs. Your footage never leaves your device — Luna scans, reads
        metadata, and renders thumbnails entirely in your browser. No uploads, no analytics, no
        telemetry.
      </p>
      <dl className="grid w-full grid-cols-[auto_1fr] gap-x-6 gap-y-2 text-left text-sm">
        {STACK.map(([name, role]) => (
          <div key={name} className="contents">
            <dt className="font-mono whitespace-nowrap">{name}</dt>
            <dd className="text-muted-foreground">{role}</dd>
          </div>
        ))}
      </dl>
    </div>
  )
}

export const Route = createFileRoute('/credits')({
  component: CreditsScreen,
})
```

- [ ] **Step 3: Not-found page on the root route**

Replace `apps/web/src/routes/__root.tsx` with:

```tsx
import { createRootRoute, Link, Outlet } from '@tanstack/react-router'
import { AppShell } from '@/components/app-shell'
import { CapabilityGate } from '@/components/capability-gate'
import { buttonVariants } from '@/components/ui/button'
import { cn } from '@/lib/utils'

export const Route = createRootRoute({
  component: () => (
    <CapabilityGate>
      <AppShell>
        <Outlet />
      </AppShell>
    </CapabilityGate>
  ),
  notFoundComponent: () => (
    <div className="flex flex-col items-center gap-4 py-24 text-center">
      <p className="text-muted-foreground font-mono text-sm">404</p>
      <h1 className="text-2xl font-semibold tracking-tight">This page doesn't exist</h1>
      <Link to="/" className={cn(buttonVariants({ variant: 'outline' }))}>
        Back to Luna
      </Link>
    </div>
  ),
})
```

- [ ] **Step 4: Complete the header nav**

In `apps/web/src/components/app-shell.tsx` (same coordination rules as Task 6), update the nav to its final shape:

```tsx
          <nav className="text-muted-foreground flex items-center gap-2 text-sm">
            <Link to="/settings" className={cn(buttonVariants({ variant: 'ghost', size: 'sm' }))}>
              Settings
            </Link>
            <Link to="/activity" className={cn(buttonVariants({ variant: 'ghost', size: 'sm' }))}>
              Activity
            </Link>
            <a href="/docs" className={cn(buttonVariants({ variant: 'ghost', size: 'sm' }))}>
              Docs
            </a>
            <Link to="/credits" className="hover:text-foreground tabular-nums transition-colors">
              v{__APP_VERSION__}
            </Link>
          </nav>
```

- [ ] **Step 5: Run all four gates**

Run: `cd /e/Coding/LunaApp && bun run lint && bun run typecheck && bun test && bun run build`
Expected: all green, 0 fail.

- [ ] **Step 6: Commit**

```bash
git add apps/web/src/features/activity/activity-screen.tsx apps/web/src/routes/activity.tsx apps/web/src/routes/credits.tsx apps/web/src/routes/__root.tsx apps/web/src/components/app-shell.tsx
git commit -m "feat(web): activity and credits routes, not-found page, full header nav"
```

---

## Definition of done

- All four gates green from the repo root: `bun run lint && bun run typecheck && bun test && bun run build` (121 pre-existing tests + Tasks 1–2 additions, 0 fail).
- No changes under `apps/web/src/components/ui/` or `tools/`.
- Final whole-plan review (opus) after all tasks, fixes + re-review, then close in the ledger.

### Maintainer QA checklist (manual, Chromium)

1. **Settings persist:** open `/settings`, set worker cap to 2, reload — still 2. Set 0 / 99 / blur — clamps to 1 / 8.
2. **DB upgrade is lossless:** your existing recent-folders list survives the first launch after this plan (idb v1→v2).
3. **Cover defaults:** fill the cover form (incl. a logo), open `/settings` → "Save current cover as defaults" → the summary shows the saved fields. Start over, reload the app: cover form is pre-filled, date = today (not the day you saved). "Clear defaults" empties it.
4. **Worker cap has effect:** on a large card, cap 1 vs 4 visibly changes metadata-pass parallelism (optional sanity check).
5. **Stale source:** scan a folder so it lands in recents, then rename the folder on disk. Click the recent entry → friendly "can't find" message (no raw DOMException), and after returning to the landing screen the entry shows the amber "Folder unavailable — click to pick again" state; clicking it opens the picker. Re-picking the folder heals the list.
6. **Permission denial:** deny the permission prompt on a recent → friendly re-authorize message.
7. **Activity:** scan + process a card → `/activity` shows scan/metadata/thumbnail entries; filters work (Warnings hides infos); Download saves a readable `.txt`; Clear empties; BRAW cards log one aggregate placeholder line, not per-clip spam.
8. **ProRes RAW regression:** a ProRes RAW clip still gets its embedded-preview thumbnail via the normal route; the report renders it.
9. **Routes:** `/credits` shows logo, version, privacy line, stack list; a garbage URL (e.g. `/nope`) shows the 404 page with a working "Back to Luna" link; header links navigate without full page reloads; Docs link still plain-anchors to `/docs`.
10. **Exports still work** and log start/finish lines to `/activity`.

## Self-review notes (run after writing, per writing-plans skill)

- **Spec coverage:** §8.13 routes (Tasks 6–7, incl. not-found; no `/docs` route — nav keeps plain anchor), §8.14 logger + activity view (Tasks 2, 4, 7, incl. download), §14 schema/versioning/defensive load (Tasks 1, 3; amendment recorded), §15 stale-source + honest per-clip failures (Tasks 4–5), §16 privacy line restated in credits, §17 core-only tests respected, §19 version surfaced in credits. Milestone 8 complete.
- **Type consistency check:** `Settings<Blob>` everywhere in app code; `logger.{level}(message, detail?)` matches all call sites; `scanFrom(handle, recentKey?)` matches recent-list call; `saveBlob(blob, fileName, mime)` matches save.ts; `LogEntry.seq` used as React key.
- **Known deliberate choices:** salvage only for `cascadedIds` members (direct-ffmpeg formats like MXF have no ISO-BMFF tail preview to salvage); `activityStore` identity selector re-renders the activity screen per log write (bounded at 500 entries, fine); Task 6/7 both touch `app-shell.tsx` nav (6 adds Settings, 7 completes it) to keep each task's typecheck green with only the routes that exist.
