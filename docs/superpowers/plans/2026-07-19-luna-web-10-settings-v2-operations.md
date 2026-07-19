# Luna Web — Plan 10: Settings v2, Operations & Feedback Round Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Execute the maintainer's feedback round: editable cover defaults in `/settings`, operation-grouped + persisted activity log, an opt-out of thumbnail generation (setting + per-run override), full captured paths in the report/CSV, a clear-local-data control, and a proper 404. (Reportbook explicitly deferred to a later plan.)

**Architecture:** Pure logic lands in `packages/core` with bun tests (settings v2 migration chain, operation grouping, activity-snapshot normalization, report `sourceRoot` + CSV `path` column). The app wires idb v3 (activity store), an operations-aware logger with write-through persistence, a `generateThumbnails` pipeline branch, and the settings/activity/404 UI.

**Tech Stack:** Existing only — TypeScript 6, TanStack Store/Router, idb, shadcn (Base UI), Tailwind 4, Lucide. **Zero new dependencies.**

## Global Constraints

- **Zero new dependencies.** No `bun add` anywhere.
- **Bun only.** Gates from the REPO ROOT: `bun run lint && bun run typecheck && bun test && bun run build` — green before AND after every task. Baseline at plan time: 141 tests.
- **Never touch** `apps/web/src/components/ui/` or `tools/`.
- **`packages/core` stays DOM-free and clock-free** in source (caller-supplied timestamps, generic `TImage`).
- **Stage by EXPLICIT file paths.** `git status` first; if a file you must modify has uncommitted maintainer changes, STOP and report. Untracked maintainer files (`docs/superpowers/backlog/`, `docs/superpowers/plans/2026-07-19-pdf-report-redesign.md`, any `docs/superpowers/specs/*pdf*`) — never touch or stage.
- **Maintainer's active PDF redesign**: `pdf-document.tsx` / `pdf-prepare.ts` are being reworked in parallel. Tasks touch them ONLY for the zero-frame guard (Task 5) with a minimal diff; the `sourceRoot`-in-ReportModel addition is data-only — do NOT refactor their separate `sourceRoot` parameter.
- TypeScript stays 6.0.3. `.gitattributes` pins LF. Environment: Windows, Git Bash tool, `cd /e/Coding/LunaApp` first, bun 1.3.14.
- Approved decisions recorded here: paths render as `<sourceRoot>/<relativePath>` (File System Access API cannot expose absolute OS paths); Reportbook deferred; 404 rebuilt by us (maintainer restyles at will).

---

### Task 1: Core settings v2 — `generateThumbnails` + real migration chain (TDD)

**Files:**
- Modify: `packages/core/src/settings/model.ts`
- Modify: `packages/core/src/settings/model.test.ts`
- (Exports unchanged — same symbol names, `SETTINGS_SCHEMA_VERSION` value changes to 2.)

**Interfaces:**
- Consumes: existing v1 model.
- Produces (Tasks 4/5/6 rely on): `Settings<TImage>` gains `generateThumbnails: boolean`; `SETTINGS_SCHEMA_VERSION = 2`; `defaultSettings()` includes `generateThumbnails: true`; `normalizeSettings` now MIGRATES v1 records (adds `generateThumbnails: true`) instead of discarding them, still collapses unknown/newer versions to defaults.

- [ ] **Step 1: Extend the tests (red)**

In `packages/core/src/settings/model.test.ts`:

1. In the `defaultSettings` describe, update the first test's expectation to include the new field:

```ts
  test('returns schema v2 with default cap, thumbnails on, empty cover defaults', () => {
    expect(defaultSettings()).toEqual({
      schemaVersion: SETTINGS_SCHEMA_VERSION,
      workerPoolCap: WORKER_POOL_CAP_DEFAULT,
      generateThumbnails: true,
      coverDefaults: {},
    })
  })
```

2. In the `normalizeSettings` describe, REPLACE the test `'unknown/newer schemaVersion yields defaults (spec §14 defensive load)'` with:

```ts
  test('newer/unknown schemaVersion yields defaults (spec §14 defensive load)', () => {
    expect(normalizeSettings({ schemaVersion: 99, workerPoolCap: 2 })).toEqual(defaultSettings())
    expect(normalizeSettings({ workerPoolCap: 2 })).toEqual(defaultSettings())
  })

  test('v1 records migrate: fields preserved, generateThumbnails defaults on', () => {
    const migrated = normalizeSettings({
      schemaVersion: 1,
      workerPoolCap: 6,
      coverDefaults: { dit: 'Shaked' },
    })
    expect(migrated).toEqual({
      schemaVersion: 2,
      workerPoolCap: 6,
      generateThumbnails: true,
      coverDefaults: { dit: 'Shaked' },
    })
  })

  test('generateThumbnails must be a real boolean; junk falls back to true', () => {
    expect(
      normalizeSettings({ schemaVersion: 2, workerPoolCap: 4, generateThumbnails: false })
        .generateThumbnails,
    ).toBe(false)
    expect(
      normalizeSettings({ schemaVersion: 2, workerPoolCap: 4, generateThumbnails: 'no' })
        .generateThumbnails,
    ).toBe(true)
    expect(
      normalizeSettings({ schemaVersion: 2, workerPoolCap: 4 }).generateThumbnails,
    ).toBe(true)
  })
```

3. Every other existing test that builds a `{ schemaVersion: 1, ... }` literal must be updated to `schemaVersion: 2` EXCEPT the migration test above (they assert normalization behavior, not migration). The `'valid record passes through with cap clamped'` test's expected object gains `generateThumbnails: true`.

- [ ] **Step 2: Run tests to verify failures**

Run: `cd /e/Coding/LunaApp && bun test packages/core/src/settings/model.test.ts`
Expected: FAIL (missing field / version mismatch).

- [ ] **Step 3: Implement v2 + migration**

In `packages/core/src/settings/model.ts`:

1. Change the version constant and interface:

```ts
export const SETTINGS_SCHEMA_VERSION = 2
```

```ts
export interface Settings<TImage = unknown> {
  schemaVersion: typeof SETTINGS_SCHEMA_VERSION
  workerPoolCap: number
  generateThumbnails: boolean
  coverDefaults: Partial<CoverFields<TImage>>
}
```

2. `defaultSettings` returns `generateThumbnails: true` (between `workerPoolCap` and `coverDefaults`).

3. Replace `normalizeSettings` with the migration chain:

```ts
// Migration chain (spec §14): each migration is a pure transform vN → vN+1 on
// the RAW record; normalization of field values happens once, at the end.
// Records newer than the current version (or unversioned junk) collapse to
// defaults — we cannot downgrade what a future deploy wrote.
function migrateV1toV2(record: Record<string, unknown>): Record<string, unknown> {
  return { ...record, schemaVersion: 2, generateThumbnails: true }
}

export function normalizeSettings<TImage = unknown>(raw: unknown): Settings<TImage> {
  if (typeof raw !== 'object' || raw === null) return defaultSettings<TImage>()
  let record = raw as Record<string, unknown>
  if (record.schemaVersion === 1) record = migrateV1toV2(record)
  if (record.schemaVersion !== SETTINGS_SCHEMA_VERSION) return defaultSettings<TImage>()
  return {
    schemaVersion: SETTINGS_SCHEMA_VERSION,
    workerPoolCap: clampWorkerPoolCap(record.workerPoolCap),
    generateThumbnails: record.generateThumbnails === false ? false : true,
    coverDefaults: normalizeCoverDefaults<TImage>(record.coverDefaults),
  }
}
```

(Note the boolean rule: only a literal `false` disables thumbnails; anything else — including junk — lands on the safe default `true`. The doc comment above `Settings` gains one line: `generateThumbnails` — master switch for the thumbnail pass, overridable per run.)

- [ ] **Step 4: Run tests to verify pass, then all four gates**

Run: `bun test packages/core/src/settings/model.test.ts` → PASS.
Run: `bun run lint && bun run typecheck && bun test && bun run build` → all green (the app compiles unchanged: no app code reads `generateThumbnails` yet).

- [ ] **Step 5: Commit**

```bash
git add packages/core/src/settings/model.ts packages/core/src/settings/model.test.ts
git commit -m "feat(core): settings v2 - generateThumbnails with v1 migration chain"
```

---

### Task 2: Core operations — log grouping + activity snapshot normalization (TDD)

**Files:**
- Create: `packages/core/src/activity/operations.ts`
- Create: `packages/core/src/activity/operations.test.ts`
- Modify: `packages/core/src/activity/log.ts` (additive `operationId` on `LogEntry`)
- Modify: `packages/core/src/index.ts` (new exports into the existing activity block)

**Interfaces:**
- Consumes: `LogEntry`, `LogLevel` from `./log`.
- Produces (Tasks 4/7 rely on):
  - `LogEntry` gains `operationId?: number`
  - `type OperationKind = 'app' | 'scan' | 'process' | 'export'`
  - `interface Operation { id: number; kind: OperationKind; label: string; startedAt: number }`
  - `interface OperationGroup { operation: Operation; entries: LogEntry[] }`
  - `groupLogByOperation(entries: readonly LogEntry[], operations: readonly Operation[]): OperationGroup[]` — newest operation first; entries keep append order inside a group; entries whose `operationId` matches no operation (or is absent) collect under a synthetic `{ id: 0, kind: 'app', label: 'General', startedAt: 0 }` group appended LAST (oldest position); operations with zero entries are omitted.
  - `interface ActivitySnapshot { operations: Operation[]; entries: LogEntry[] }`
  - `normalizeActivitySnapshot(raw: unknown): ActivitySnapshot` — defensive: non-object → empty; arrays filtered to structurally valid members (numbers are finite, strings are strings, level ∈ LOG_LEVELS, kind ∈ the four kinds).
  - `capActivitySnapshot(snapshot: ActivitySnapshot, maxOperations: number, maxEntries: number): ActivitySnapshot` — keeps the NEWEST maxOperations operations (by startedAt, ties by id) and the NEWEST maxEntries entries, then drops entries whose operationId is no longer present UNLESS operationId is undefined/0 (general entries survive the entry cap only).

- [ ] **Step 1: Write the failing tests**

Create `packages/core/src/activity/operations.test.ts`:

```ts
import { describe, expect, test } from 'bun:test'
import type { LogEntry } from './log'
import {
  capActivitySnapshot,
  GENERAL_OPERATION,
  groupLogByOperation,
  normalizeActivitySnapshot,
  type Operation,
} from './operations'

function op(id: number, startedAt = id * 1000): Operation {
  return { id, kind: 'scan', label: `Op ${id}`, startedAt }
}

function entry(seq: number, operationId?: number): LogEntry {
  return {
    seq,
    timestamp: 1_700_000_000_000 + seq,
    level: 'info',
    message: `m${seq}`,
    ...(operationId !== undefined ? { operationId } : {}),
  }
}

describe('groupLogByOperation', () => {
  test('groups by operation, newest operation first, entry order preserved', () => {
    const groups = groupLogByOperation(
      [entry(1, 1), entry(2, 2), entry(3, 1), entry(4, 2)],
      [op(1), op(2)],
    )
    expect(groups.map((g) => g.operation.id)).toEqual([2, 1])
    expect(groups[0]?.entries.map((e) => e.seq)).toEqual([2, 4])
    expect(groups[1]?.entries.map((e) => e.seq)).toEqual([1, 3])
  })

  test('unmatched and untagged entries fall into the trailing General group', () => {
    const groups = groupLogByOperation([entry(1), entry(2, 99), entry(3, 1)], [op(1)])
    expect(groups.map((g) => g.operation.id)).toEqual([1, GENERAL_OPERATION.id])
    expect(groups[1]?.entries.map((e) => e.seq)).toEqual([1, 2])
  })

  test('operations with zero entries are omitted', () => {
    const groups = groupLogByOperation([entry(1, 1)], [op(1), op(2)])
    expect(groups.map((g) => g.operation.id)).toEqual([1])
  })

  test('empty input yields no groups', () => {
    expect(groupLogByOperation([], [])).toEqual([])
  })
})

describe('normalizeActivitySnapshot', () => {
  test('non-object input yields an empty snapshot', () => {
    expect(normalizeActivitySnapshot(undefined)).toEqual({ operations: [], entries: [] })
    expect(normalizeActivitySnapshot('junk')).toEqual({ operations: [], entries: [] })
  })

  test('structurally invalid members are filtered, valid ones survive', () => {
    const snapshot = normalizeActivitySnapshot({
      operations: [op(1), { id: 'x' }, null, { id: 2, kind: 'bogus', label: 'l', startedAt: 1 }],
      entries: [entry(1, 1), { seq: 2 }, { ...entry(3), level: 'loud' }],
    })
    expect(snapshot.operations).toEqual([op(1)])
    expect(snapshot.entries).toEqual([entry(1, 1)])
  })
})

describe('capActivitySnapshot', () => {
  test('keeps newest operations and entries, drops entries of evicted operations', () => {
    const snapshot = {
      operations: [op(1), op(2), op(3)],
      entries: [entry(1, 1), entry(2, 2), entry(3, 3), entry(4), entry(5, 3)],
    }
    const capped = capActivitySnapshot(snapshot, 2, 10)
    expect(capped.operations.map((o) => o.id)).toEqual([2, 3])
    expect(capped.entries.map((e) => e.seq)).toEqual([2, 3, 4, 5])
  })

  test('entry cap keeps the newest entries', () => {
    const snapshot = { operations: [op(1)], entries: [entry(1, 1), entry(2, 1), entry(3, 1)] }
    expect(capActivitySnapshot(snapshot, 5, 2).entries.map((e) => e.seq)).toEqual([2, 3])
  })
})
```

- [ ] **Step 2: Run to verify failure**

Run: `bun test packages/core/src/activity/operations.test.ts` → FAIL (module missing).

- [ ] **Step 3: Implement**

1. In `packages/core/src/activity/log.ts`, add to `LogEntry` (after `seq`):

```ts
  operationId?: number // groups entries under a user operation (0/absent = general)
```

2. Create `packages/core/src/activity/operations.ts`:

```ts
import { type LogEntry, LOG_LEVELS, type LogLevel } from './log'

export type OperationKind = 'app' | 'scan' | 'process' | 'export'

const OPERATION_KINDS: readonly OperationKind[] = ['app', 'scan', 'process', 'export']

export interface Operation {
  id: number
  kind: OperationKind
  label: string
  startedAt: number // epoch ms, caller-supplied
}

export interface OperationGroup {
  operation: Operation
  entries: LogEntry[]
}

// Bucket for entries written outside any user operation (or whose operation
// was evicted by the cap): always rendered last.
export const GENERAL_OPERATION: Operation = { id: 0, kind: 'app', label: 'General', startedAt: 0 }

export function groupLogByOperation(
  entries: readonly LogEntry[],
  operations: readonly Operation[],
): OperationGroup[] {
  const known = new Map<number, LogEntry[]>()
  for (const operation of operations) known.set(operation.id, [])
  const general: LogEntry[] = []
  for (const entry of entries) {
    const bucket = entry.operationId !== undefined ? known.get(entry.operationId) : undefined
    if (bucket) bucket.push(entry)
    else general.push(entry)
  }
  const groups: OperationGroup[] = []
  const byNewest = [...operations].sort((a, b) => b.startedAt - a.startedAt || b.id - a.id)
  for (const operation of byNewest) {
    const bucket = known.get(operation.id)
    if (bucket && bucket.length > 0) groups.push({ operation, entries: bucket })
  }
  if (general.length > 0) groups.push({ operation: GENERAL_OPERATION, entries: general })
  return groups
}

export interface ActivitySnapshot {
  operations: Operation[]
  entries: LogEntry[]
}

// Defensive read of a persisted snapshot: any past or future app version may
// have written it. Structurally invalid members are dropped, never repaired.
export function normalizeActivitySnapshot(raw: unknown): ActivitySnapshot {
  if (typeof raw !== 'object' || raw === null) return { operations: [], entries: [] }
  const record = raw as { operations?: unknown; entries?: unknown }
  const operations = Array.isArray(record.operations)
    ? record.operations.filter(isValidOperation)
    : []
  const entries = Array.isArray(record.entries) ? record.entries.filter(isValidEntry) : []
  return { operations, entries }
}

function isValidOperation(value: unknown): value is Operation {
  if (typeof value !== 'object' || value === null) return false
  const op = value as Record<string, unknown>
  return (
    typeof op.id === 'number' &&
    Number.isFinite(op.id) &&
    OPERATION_KINDS.includes(op.kind as OperationKind) &&
    typeof op.label === 'string' &&
    typeof op.startedAt === 'number' &&
    Number.isFinite(op.startedAt)
  )
}

function isValidEntry(value: unknown): value is LogEntry {
  if (typeof value !== 'object' || value === null) return false
  const entry = value as Record<string, unknown>
  const operationIdOk =
    entry.operationId === undefined ||
    (typeof entry.operationId === 'number' && Number.isFinite(entry.operationId))
  return (
    typeof entry.seq === 'number' &&
    Number.isFinite(entry.seq) &&
    typeof entry.timestamp === 'number' &&
    Number.isFinite(entry.timestamp) &&
    LOG_LEVELS.includes(entry.level as LogLevel) &&
    typeof entry.message === 'string' &&
    (entry.detail === undefined || typeof entry.detail === 'string') &&
    operationIdOk
  )
}

export function capActivitySnapshot(
  snapshot: ActivitySnapshot,
  maxOperations: number,
  maxEntries: number,
): ActivitySnapshot {
  const operations = [...snapshot.operations]
    .sort((a, b) => a.startedAt - b.startedAt || a.id - b.id)
    .slice(-Math.max(0, maxOperations))
  const kept = new Set(operations.map((o) => o.id))
  const entries = snapshot.entries
    .slice(-Math.max(0, maxEntries))
    .filter((e) => e.operationId === undefined || e.operationId === 0 || kept.has(e.operationId))
  return { operations, entries }
}
```

3. In `packages/core/src/index.ts`, extend the activity block:

```ts
export type {
  ActivitySnapshot,
  Operation,
  OperationGroup,
  OperationKind,
} from './activity/operations'
export {
  capActivitySnapshot,
  GENERAL_OPERATION,
  groupLogByOperation,
  normalizeActivitySnapshot,
} from './activity/operations'
```

- [ ] **Step 4: Run tests, then all four gates**

`bun test packages/core/src/activity/operations.test.ts` → PASS. Then all four gates → green.

- [ ] **Step 5: Commit**

```bash
git add packages/core/src/activity/operations.ts packages/core/src/activity/operations.test.ts packages/core/src/activity/log.ts packages/core/src/index.ts
git commit -m "feat(core): operation model, log grouping, activity snapshot normalization"
```

---

### Task 3: Core report `sourceRoot` + CSV `path` column (TDD)

**Files:**
- Modify: `packages/core/src/report/model.ts` (additive `sourceRoot`)
- Modify: `packages/core/src/export/csv.ts` (`path` column)
- Modify: the existing report-model and CSV test files (locate with `ls packages/core/src/report/*.test.ts packages/core/src/export/*.test.ts`)

**Interfaces:**
- Consumes: current `BuildReportInput` / `ReportModel` / `CSV_COLUMNS` shapes (post other-files rework).
- Produces (Tasks 5/7 + maintainer's PDF may rely on):
  - `BuildReportInput.sourceRoot?: string`; `ReportModel.sourceRoot: string` (defaults to `''` when absent).
  - `CSV_COLUMNS` gains `'path'` immediately after `'relativePath'`; the cell renders `sourceRoot ? `${sourceRoot}/${relativePath}` : relativePath`.
- Constraint honored: the maintainer's `pdf-prepare.ts` keeps its own separate `sourceRoot` parameter — do NOT touch pdf files in this task.

- [ ] **Step 1: Extend tests (red)**

In the report-model test file, add:

```ts
  test('sourceRoot flows through; absent input defaults to empty string', () => {
    const base = { clips: [], otherFiles: [], metadataById: {}, thumbsById: {}, cover: {} }
    expect(buildReportModel({ ...base, sourceRoot: 'CARD_A' }).sourceRoot).toBe('CARD_A')
    expect(buildReportModel(base).sourceRoot).toBe('')
  })
```

In the CSV test file, add (adapt the fixture helper the file already uses — read it first; the assertion targets are exact):

```ts
  test('path column renders sourceRoot-prefixed captured path', () => {
    // build a one-clip report via the file's existing fixture helper, with
    // sourceRoot 'CARD_A' and a clip whose relativePath is 'A001/clip.mov'
    // then:
    const csv = generateReportCsv(report)
    const header = csv.split('\r\n')[0]
    expect(header).toContain('relativePath,path,')
    expect(csv).toContain('A001/clip.mov,CARD_A/A001/clip.mov')
  })

  test('path column falls back to relativePath when sourceRoot is empty', () => {
    // same fixture with sourceRoot '' (or absent):
    const line = generateReportCsv(report).split('\r\n')[1] ?? ''
    expect(line).toContain('A001/clip.mov,A001/clip.mov')
  })
```

(If the CSV file's line terminator constant differs from `\r\n`, match the file's existing tests — RFC-4180 uses CRLF; the existing tests are the source of truth.)

- [ ] **Step 2: Run to verify failure**

Run both test files → FAIL.

- [ ] **Step 3: Implement**

1. `packages/core/src/report/model.ts`:
   - `BuildReportInput` gains `sourceRoot?: string` (after `cover`).
   - `ReportModel` gains `sourceRoot: string` (after `cover`), with the comment: `// Name of the picked folder — the deepest "absolute" path a browser can capture.`
   - `buildReportModel` returns `sourceRoot: input.sourceRoot ?? ''` in the result object.
2. `packages/core/src/export/csv.ts`:
   - Insert `'path',` into `CSV_COLUMNS` directly after `'relativePath',`.
   - In the row-building array, directly after the `clip.relativePath` element, insert:

```ts
          report.sourceRoot ? `${report.sourceRoot}/${clip.relativePath}` : clip.relativePath,
```

- [ ] **Step 4: Tests green, all four gates**

Both test files PASS; all four gates green. NOTE: `bun run typecheck` will surface every app-side `buildReportModel` call site — `sourceRoot` is optional, so existing calls compile; do not modify app files in this task.

- [ ] **Step 5: Commit**

```bash
git add packages/core/src/report/model.ts packages/core/src/export/csv.ts <the two test files>
git commit -m "feat(core): report sourceRoot and CSV captured-path column"
```

---

### Task 4: App — idb v3, operations-aware logger with persistence, boot hydration

**Files:**
- Modify: `apps/web/src/persistence/db.ts` (v3 + `activity` store)
- Create: `apps/web/src/persistence/activity.ts`
- Modify: `apps/web/src/lib/logger.ts` (operations + persistence)
- Modify: `apps/web/src/main.tsx` (hydrate activity alongside settings)
- Modify: `apps/web/src/features/scan/run-scan.ts`, `apps/web/src/features/process/run-processing.ts`, `apps/web/src/features/export/exporter.ts` (beginOperation calls)
- Modify: `apps/web/src/features/activity/activity-screen.tsx` (minimal adaptation to the new store shape — the grouped UI is Task 7)

**Interfaces:**
- Consumes: Task 2's core exports; existing logger call sites (signatures unchanged).
- Produces (Task 7 relies on): `activityStore: Store<ActivitySnapshot>`; `logger.{debug,info,warn,error}` unchanged; `beginOperation(kind: OperationKind, label: string): void`; `clearActivity(): void` (also clears persistence); `hydrateActivity(): Promise<void>`.

- [ ] **Step 1: DB v3**

In `apps/web/src/persistence/db.ts`: add to the schema interface

```ts
  // One record ('log') holding the persisted ActivitySnapshot; `unknown` for
  // the same reason as settings — normalizeActivitySnapshot is the only reader.
  activity: { key: string; value: unknown }
```

bump `openDB<LunaDb>('luna-web', 3, ...)` and add to `upgrade`:

```ts
      if (oldVersion < 3) db.createObjectStore('activity')
```

- [ ] **Step 2: Activity persistence module**

Create `apps/web/src/persistence/activity.ts`:

```ts
import { type ActivitySnapshot, normalizeActivitySnapshot } from '@luna-web/core'
import { getDb } from './db'

const ACTIVITY_KEY = 'log'

export async function loadActivity(): Promise<ActivitySnapshot> {
  const db = await getDb()
  return normalizeActivitySnapshot(await db.get('activity', ACTIVITY_KEY))
}

export async function saveActivity(snapshot: ActivitySnapshot): Promise<void> {
  const db = await getDb()
  await db.put('activity', snapshot, ACTIVITY_KEY)
}

export async function clearPersistedActivity(): Promise<void> {
  const db = await getDb()
  await db.delete('activity', ACTIVITY_KEY)
}
```

- [ ] **Step 3: Rework the logger**

Replace `apps/web/src/lib/logger.ts` with:

```ts
import {
  type ActivitySnapshot,
  appendLog,
  capActivitySnapshot,
  type LogEntry,
  type LogLevel,
  type Operation,
  type OperationKind,
} from '@luna-web/core'
import { Store } from '@tanstack/store'
import { clearPersistedActivity, loadActivity, saveActivity } from '@/persistence/activity'

// Spec §8.14 + Plan 10: operation-grouped ring, persisted to idb so a refresh
// keeps past operations readable. Caps keep the store and the record bounded.
const ENTRY_CAPACITY = 1000
const OPERATION_CAPACITY = 25
const PERSIST_DEBOUNCE_MS = 400

let seq = 0
let operationSeq = 0
let currentOperationId: number | undefined
let persistTimer: ReturnType<typeof setTimeout> | null = null

export const activityStore = new Store<ActivitySnapshot>({ operations: [], entries: [] })

// Restore the persisted snapshot BEFORE the first render (main.tsx) so new
// entries append after history and counters never collide with restored ids.
export async function hydrateActivity(): Promise<void> {
  const restored = await loadActivity()
  activityStore.setState(() => restored)
  seq = restored.entries.reduce((max, e) => Math.max(max, e.seq), 0)
  operationSeq = restored.operations.reduce((max, o) => Math.max(max, o.id), 0)
}

function schedulePersist(): void {
  if (persistTimer !== null) clearTimeout(persistTimer)
  persistTimer = setTimeout(() => {
    persistTimer = null
    saveActivity(activityStore.state).catch(() => {
      // Best-effort: a failed idb write must never affect the app.
    })
  }, PERSIST_DEBOUNCE_MS)
}

export function beginOperation(kind: OperationKind, label: string): void {
  operationSeq += 1
  currentOperationId = operationSeq
  const operation: Operation = { id: operationSeq, kind, label, startedAt: Date.now() }
  activityStore.setState((s) =>
    capActivitySnapshot(
      { operations: [...s.operations, operation], entries: s.entries },
      OPERATION_CAPACITY,
      ENTRY_CAPACITY,
    ),
  )
  schedulePersist()
}

function write(level: LogLevel, message: string, detail?: string): void {
  seq += 1
  const entry: LogEntry = {
    seq,
    timestamp: Date.now(),
    level,
    message,
    ...(detail !== undefined ? { detail } : {}),
    ...(currentOperationId !== undefined ? { operationId: currentOperationId } : {}),
  }
  activityStore.setState((s) =>
    capActivitySnapshot(
      { operations: s.operations, entries: appendLog(s.entries, entry, ENTRY_CAPACITY) },
      OPERATION_CAPACITY,
      ENTRY_CAPACITY,
    ),
  )
  const line = detail !== undefined ? `${message} — ${detail}` : message
  if (level === 'error') console.error(`[luna] ${line}`)
  else if (level === 'warn') console.warn(`[luna] ${line}`)
  else console.log(`[luna] ${line}`)
  schedulePersist()
}

export const logger = {
  debug: (message: string, detail?: string) => write('debug', message, detail),
  info: (message: string, detail?: string) => write('info', message, detail),
  warn: (message: string, detail?: string) => write('warn', message, detail),
  error: (message: string, detail?: string) => write('error', message, detail),
}

export function clearActivity(): void {
  currentOperationId = undefined
  activityStore.setState(() => ({ operations: [], entries: [] }))
  clearPersistedActivity().catch(() => {})
}
```

- [ ] **Step 4: Begin operations at the user-action boundaries**

1. `apps/web/src/features/scan/run-scan.ts`: import `beginOperation` from `@/lib/logger`. In `scanFrom`, immediately BEFORE the `scanStore.setState(() => ({ ...initialScanState, phase: 'scanning', ... }))` line (i.e., after the permission gate passed):

```ts
  beginOperation('scan', `Scan: ${handle.name}`)
```

2. `apps/web/src/features/process/run-processing.ts`: import `beginOperation`. In `startProcessing`, immediately after the `if (state.phase !== 'summary') return` guard:

```ts
  beginOperation('process', `Process: ${state.sourceName ?? 'card'} (${clips.length} clips)`)
```

(Note: `clips` is declared right after — move the `const clips = state.clips` line ABOVE the beginOperation call.)

3. `apps/web/src/features/export/exporter.ts`: import `beginOperation`. In `runExport`, replace `logger.info(`Export started: ${exporter.label}`)` with:

```ts
  beginOperation('export', `Export ${exporter.label}: ${report.cover.projectTitle ?? 'report'}`)
  logger.info(`Export started: ${exporter.label}`)
```

- [ ] **Step 5: Hydrate at boot**

In `apps/web/src/main.tsx`, change the bootstrap chain to hydrate both stores:

```ts
void Promise.allSettled([hydrateSettings(), hydrateActivity()]).then(() => {
  createRoot(rootElement).render(
    <StrictMode>
      <RouterProvider router={router} />
    </StrictMode>,
  )
})
```

with the added import `import { hydrateActivity } from './lib/logger'`. (`allSettled` keeps the render-once-on-any-outcome property; drop the previous `.catch(() => {})` form.)

- [ ] **Step 6: Minimal activity-screen adaptation (grouped UI is Task 7)**

In `apps/web/src/features/activity/activity-screen.tsx`, change the selector line to read entries from the snapshot:

```ts
  const entries = useSelector(activityStore, (s) => s.entries)
```

Everything else in the file stays as-is (it operates on `entries`).

- [ ] **Step 7: All four gates, commit**

Gates green (141+ tests — core counts grew in Tasks 1–3), then:

```bash
git add apps/web/src/persistence/db.ts apps/web/src/persistence/activity.ts apps/web/src/lib/logger.ts apps/web/src/main.tsx apps/web/src/features/scan/run-scan.ts apps/web/src/features/process/run-processing.ts apps/web/src/features/export/exporter.ts apps/web/src/features/activity/activity-screen.tsx
git commit -m "feat(web): persisted operation-grouped activity log (idb v3)"
```

---

### Task 5: App — no-thumbnails mode (setting + per-run override + zero-frame guards)

**Files:**
- Modify: `apps/web/src/features/scan/store.ts` (`generateThumbnails` run flag)
- Modify: `apps/web/src/features/scan/run-scan.ts` (seed the flag from settings)
- Modify: `apps/web/src/features/scan/scan-screen.tsx` (summary checkbox — MAINTAINER-STYLED, minimal diff)
- Modify: `apps/web/src/features/process/run-processing.ts` (skip branch)
- Modify: `apps/web/src/features/report/report-workspace.tsx` (thread `sourceRoot` into buildReportModel — piggybacked here because this task already verifies the workspace)
- Verify (minimal guard ONLY if broken): `apps/web/src/features/export/pdf-document.tsx` zero-frame rendering; `apps/web/src/features/report/clip-card.tsx` zero-frame rendering

**Interfaces:**
- Consumes: `settingsStore.state.generateThumbnails` (Task 1 field, hydrated Task 3/Plan 09); `startThumbnails(run)`; `guardedUpdate`.
- Produces: `ScanState.generateThumbnails: boolean` (initial `true`); summary checkbox toggles it; when `false`, the pipeline goes metadata → `processed` with NO thumbStatus entries (empty `thumbsById` — every clip renders `NotAttempted` in CSV via the existing `aggregateThumbnailOutcome([])`).

- [ ] **Step 1: Run flag in the scan store**

In `apps/web/src/features/scan/store.ts`, add to `ScanState` (after `sourceName`):

```ts
  // Per-run thumbnail switch: seeded from settings when a scan completes,
  // overridable on the summary screen before processing starts.
  generateThumbnails: boolean
```

and `generateThumbnails: true,` in `initialScanState`.

- [ ] **Step 2: Seed from settings**

In `apps/web/src/features/scan/run-scan.ts`, import `settingsStore` from `@/features/settings/settings-store`. In the success `setState` (the one setting `phase: 'summary'`), add:

```ts
      generateThumbnails: settingsStore.state.generateThumbnails,
```

- [ ] **Step 3: Summary checkbox (maintainer-styled file — minimal diff)**

In `apps/web/src/features/scan/scan-screen.tsx` (git status first; STOP if uncommitted changes): add selectors + a labeled native checkbox between the stat grid and the buttons row of the `summary` branch:

```tsx
  const generateThumbnails = useSelector(scanStore, (s) => s.generateThumbnails)
```

```tsx
            <label className="text-muted-foreground flex w-fit cursor-pointer items-center gap-2 text-sm">
              <input
                type="checkbox"
                className="accent-primary size-4"
                checked={generateThumbnails}
                onChange={(e) => {
                  const checked = e.currentTarget.checked
                  scanStore.setState((s) => ({ ...s, generateThumbnails: checked }))
                }}
              />
              Generate thumbnails (uncheck for a tech-data-only report)
            </label>
```

Do not alter any other markup, classes, or the glow style on the Process button.

- [ ] **Step 4: Pipeline skip branch**

In `apps/web/src/features/process/run-processing.ts`, replace the `await startThumbnails(run)` call inside its try block with:

```ts
    if (scanStore.state.generateThumbnails) {
      await startThumbnails(run)
    } else {
      logger.info('Thumbnails skipped (disabled for this run) — tech-data-only report')
      guardedUpdate(run, (s) => ({ ...s, phase: 'processed' }))
    }
```

(`scanStore` needs importing if not already imported in that scope — it is, via `../scan/store`.)

- [ ] **Step 5: Thread sourceRoot into the workspace model**

In `apps/web/src/features/report/report-workspace.tsx` (maintainer-restyled — minimal diff): add a `sourceName` selector and pass it to `buildReportModel`:

```ts
  const sourceName = useSelector(scanStore, (s) => s.sourceName)
```

and in the `useMemo` input object add `sourceRoot: sourceName ?? ''` (and `sourceName` to the dependency array).

- [ ] **Step 6: Zero-frame verification (guards only if actually broken)**

With `bun run build` green, verify by READING (do not run a browser):
1. `apps/web/src/features/report/clip-card.tsx` — what renders when `clip.thumbnails` is `[]`? Acceptable: placeholder/no-strip. If it crashes (e.g., indexes `[0]` unconditionally), add the smallest possible guard.
2. `apps/web/src/features/export/pdf-document.tsx` — same question for the thumbnail strip band. This file is mid-redesign by the maintainer: if a guard is genuinely required, keep it to one conditional; if the file already tolerates empty arrays, TOUCH NOTHING and record that in your report.
3. CSV: `aggregateThumbnailOutcome([])` returns `'NotAttempted'` (already core-tested) — nothing to do.

- [ ] **Step 7: All four gates, commit**

```bash
git add apps/web/src/features/scan/store.ts apps/web/src/features/scan/run-scan.ts apps/web/src/features/scan/scan-screen.tsx apps/web/src/features/process/run-processing.ts apps/web/src/features/report/report-workspace.tsx
# plus clip-card.tsx / pdf-document.tsx ONLY if a guard was genuinely required
git commit -m "feat(web): opt-out thumbnail generation with per-run override; sourceRoot into report model"
```

---

### Task 6: App — settings screen v2: editable defaults, thumbnails toggle, clear local data

**Files:**
- Modify: `apps/web/src/features/settings/settings-screen.tsx` (rework)
- Modify: `apps/web/src/lib/engine-cache.ts` (export the cache name)
- Create: `apps/web/src/persistence/clear.ts`
- Modify: `apps/web/src/persistence/db.ts` (add `closeDb`)

**Interfaces:**
- Consumes: `settingsStore`/`updateSettings`; `coverStore`/`setCoverFields`; core `clampWorkerPoolCap`, `WORKER_POOL_CAP_MIN/MAX`, `CoverFields`; `deleteDB` from `idb` (already a dependency); `logger`.
- Produces: `ENGINE_CACHE_NAME` export; `clearLocalData(): Promise<void>`; `closeDb(): Promise<void>`.

- [ ] **Step 1: Export the engine cache name**

In `apps/web/src/lib/engine-cache.ts`, change the private const to an export:

```ts
export const ENGINE_CACHE_NAME = 'luna-engines-v1'
```

(and use `ENGINE_CACHE_NAME` in `caches.open(...)`).

- [ ] **Step 2: closeDb + clearLocalData**

In `apps/web/src/persistence/db.ts` append:

```ts
// Required before deleteDB: an open connection would block deletion forever.
export async function closeDb(): Promise<void> {
  if (!dbPromise) return
  const db = await dbPromise.catch(() => null)
  dbPromise = null
  db?.close()
}
```

Create `apps/web/src/persistence/clear.ts`:

```ts
import { deleteDB } from 'idb'
import { ENGINE_CACHE_NAME } from '@/lib/engine-cache'
import { closeDb } from './db'

// Settings "Clear local data": everything Luna keeps on this machine —
// settings, cover defaults, recent folders, the activity log, and the cached
// ffmpeg engine. Footage is never stored, so there is nothing else to clear.
export async function clearLocalData(): Promise<void> {
  await closeDb()
  await deleteDB('luna-web')
  await caches.delete(ENGINE_CACHE_NAME).catch(() => false)
  window.location.reload()
}
```

- [ ] **Step 3: Rework the settings screen**

Replace the body of `apps/web/src/features/settings/settings-screen.tsx` with three cards. Keep `coverDefaultsFrom` as-is. Structure (complete component):

```tsx
import {
  type CoverFields,
  clampWorkerPoolCap,
  WORKER_POOL_CAP_MAX,
  WORKER_POOL_CAP_MIN,
} from '@luna-web/core'
import { useSelector } from '@tanstack/react-store'
import { useState } from 'react'
import { Button } from '@/components/ui/button'
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '@/components/ui/card'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import { coverStore } from '@/features/report/cover-store'
import { clearLocalData } from '@/persistence/clear'
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

const DEFAULT_FIELDS: ReadonlyArray<{
  key: 'projectTitle' | 'productionCompany' | 'dit' | 'director' | 'dp'
  label: string
}> = [
  { key: 'projectTitle', label: 'Project' },
  { key: 'productionCompany', label: 'Production company' },
  { key: 'dit', label: 'DIT' },
  { key: 'director', label: 'Director' },
  { key: 'dp', label: 'DP' },
]

export function SettingsScreen() {
  const workerPoolCap = useSelector(settingsStore, (s) => s.workerPoolCap)
  const generateThumbnails = useSelector(settingsStore, (s) => s.generateThumbnails)
  const coverDefaults = useSelector(settingsStore, (s) => s.coverDefaults)
  const [confirmingClear, setConfirmingClear] = useState(false)
  const cores = navigator.hardwareConcurrency || 2

  const patchDefault = (key: (typeof DEFAULT_FIELDS)[number]['key'], value: string) => {
    const next = { ...settingsStore.state.coverDefaults }
    if (value.trim() === '') delete next[key]
    else next[key] = value
    void updateSettings({ coverDefaults: next })
  }

  return (
    <div className="mx-auto flex max-w-2xl flex-col gap-6">
      <h1 className="text-2xl font-semibold tracking-tight">Settings</h1>

      <Card>
        <CardHeader>
          <CardTitle>Processing</CardTitle>
          <CardDescription>
            Applies to the next run. Worker count is bounded by your CPU ({cores} cores detected).
          </CardDescription>
        </CardHeader>
        <CardContent className="flex flex-col gap-4">
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
              {WORKER_POOL_CAP_MIN}–{WORKER_POOL_CAP_MAX}
            </span>
          </div>
          <label className="text-muted-foreground flex w-fit cursor-pointer items-center gap-2 text-sm">
            <input
              type="checkbox"
              className="accent-primary size-4"
              checked={generateThumbnails}
              onChange={(e) => void updateSettings({ generateThumbnails: e.currentTarget.checked })}
            />
            Generate thumbnails by default (override per run on the scan summary)
          </label>
        </CardContent>
      </Card>

      <Card>
        <CardHeader>
          <CardTitle>Report defaults</CardTitle>
          <CardDescription>
            Pre-filled into the cover on every launch. The report date always defaults to today and
            is never saved. Edits save as you leave each field.
          </CardDescription>
        </CardHeader>
        <CardContent className="flex flex-col gap-4">
          <div className="grid grid-cols-1 gap-3 sm:grid-cols-2">
            {DEFAULT_FIELDS.map(({ key, label }) => (
              <div key={key} className="flex flex-col gap-1.5">
                <Label htmlFor={`default-${key}`}>{label}</Label>
                <Input
                  id={`default-${key}`}
                  defaultValue={coverDefaults[key] ?? ''}
                  onBlur={(event) => {
                    if ((coverDefaults[key] ?? '') !== event.currentTarget.value)
                      patchDefault(key, event.currentTarget.value)
                  }}
                />
              </div>
            ))}
            <div className="flex flex-col gap-1.5">
              <Label htmlFor="default-logo">Logo</Label>
              <div className="flex items-center gap-2">
                <Input
                  id="default-logo"
                  type="file"
                  accept="image/*"
                  className="max-w-56"
                  onChange={(event) => {
                    const file = event.currentTarget.files?.[0]
                    if (file)
                      void updateSettings({
                        coverDefaults: { ...settingsStore.state.coverDefaults, logo: file },
                      })
                  }}
                />
                {coverDefaults.logo !== undefined && (
                  <Button
                    variant="ghost"
                    size="sm"
                    onClick={() => {
                      const { logo: _drop, ...rest } = settingsStore.state.coverDefaults
                      void updateSettings({ coverDefaults: rest })
                    }}
                  >
                    Remove
                  </Button>
                )}
              </div>
            </div>
          </div>
          <div className="flex items-center gap-2">
            <Button
              variant="outline"
              size="sm"
              onClick={() => void updateSettings({ coverDefaults: coverDefaultsFrom(coverStore.state) })}
            >
              Copy from current cover
            </Button>
            <Button
              variant="ghost"
              size="sm"
              disabled={Object.keys(coverDefaults).length === 0}
              onClick={() => void updateSettings({ coverDefaults: {} })}
            >
              Clear defaults
            </Button>
          </div>
        </CardContent>
      </Card>

      <Card>
        <CardHeader>
          <CardTitle>Local data</CardTitle>
          <CardDescription>
            Luna stores settings, report defaults, recent folders, the activity log, and the cached
            decode engine on this device. Footage is never stored.
          </CardDescription>
        </CardHeader>
        <CardContent>
          {confirmingClear ? (
            <div className="flex flex-wrap items-center gap-2">
              <span className="text-destructive text-sm">
                Delete all locally stored Luna data and reload?
              </span>
              <Button variant="destructive" size="sm" onClick={() => void clearLocalData()}>
                Delete everything
              </Button>
              <Button variant="ghost" size="sm" onClick={() => setConfirmingClear(false)}>
                Cancel
              </Button>
            </div>
          ) : (
            <Button variant="outline" size="sm" onClick={() => setConfirmingClear(true)}>
              Clear local data…
            </Button>
          )}
        </CardContent>
      </Card>
    </div>
  )
}
```

VERIFY-DON'T-ASSUME: check `button.tsx` actually exposes a `destructive` variant (it's a shadcn default, but confirm); confirm `Input` passes through `type="file"` props (it's a plain input wrapper). Note the editable-defaults inputs use `defaultValue` + key-less mounts: values hydrate pre-render (Plan 09) and this screen remounts on navigation, so stale-display risk is nil; the change-gate on blur prevents no-op writes. The uncontrolled logo file input intentionally shows no filename after reload — the "Remove" button's presence is the saved indicator.

- [ ] **Step 4: All four gates, commit**

```bash
git add apps/web/src/features/settings/settings-screen.tsx apps/web/src/lib/engine-cache.ts apps/web/src/persistence/clear.ts apps/web/src/persistence/db.ts
git commit -m "feat(web): settings v2 UI - editable report defaults, thumbnail toggle, clear local data"
```

---

### Task 7: App — operation-grouped activity UI, captured paths in workspace, 404 rebuild

**Files:**
- Modify: `apps/web/src/features/activity/activity-screen.tsx` (grouped rework)
- Modify: `apps/web/src/features/report/clip-card.tsx` (path line — MAINTAINER-STYLED, minimal diff)
- Modify: `apps/web/src/routes/__root.tsx` (404 rebuild)

**Interfaces:**
- Consumes: `groupLogByOperation`, `GENERAL_OPERATION`, `formatLogText`, `logLevelAtLeast`, `LogLevel`, `OperationGroup` from core; `activityStore` snapshot shape (Task 4); `saveBlob`; `ReportModel.sourceRoot` via the workspace (Task 5 threaded it).
- Produces: grouped `/activity`; clip rows showing `sourceRoot/relativePath`; a proper 404.

- [ ] **Step 1: Grouped activity screen**

Rework `apps/web/src/features/activity/activity-screen.tsx`:

```tsx
import {
  formatLogText,
  GENERAL_OPERATION,
  groupLogByOperation,
  type LogLevel,
  logLevelAtLeast,
  type OperationGroup,
} from '@luna-web/core'
import { useSelector } from '@tanstack/react-store'
import { ChevronDown, Download, Trash2 } from 'lucide-react'
import { useState } from 'react'
import { Button } from '@/components/ui/button'
import { saveBlob } from '@/features/export/save'
import { activityStore, clearActivity } from '@/lib/logger'

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

function operationTitle(group: OperationGroup): string {
  if (group.operation.id === GENERAL_OPERATION.id) return GENERAL_OPERATION.label
  return `${group.operation.label} — ${new Date(group.operation.startedAt).toLocaleString(undefined, { hour12: false })}`
}

// Download text: operation headings above their entries, newest operation first.
function downloadText(groups: OperationGroup[]): string {
  return groups
    .map((g) => `${'='.repeat(4)} ${operationTitle(g)} ${'='.repeat(4)}\n${formatLogText(g.entries)}`)
    .join('\n\n')
}

export function ActivityScreen() {
  const snapshot = useSelector(activityStore, (s) => s)
  const [minLevel, setMinLevel] = useState<LogLevel>('info')
  const [collapsed, setCollapsed] = useState<ReadonlySet<number>>(new Set())

  const allGroups = groupLogByOperation(snapshot.entries, snapshot.operations)
  const groups = allGroups
    .map((g) => ({ ...g, entries: g.entries.filter((e) => logLevelAtLeast(e.level, minLevel)) }))
    .filter((g) => g.entries.length > 0)

  const toggle = (id: number) =>
    setCollapsed((prev) => {
      const next = new Set(prev)
      if (next.has(id)) next.delete(id)
      else next.add(id)
      return next
    })

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
            disabled={snapshot.entries.length === 0}
            onClick={() =>
              void saveBlob(
                new Blob([downloadText(allGroups)], { type: 'text/plain' }),
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
            disabled={snapshot.entries.length === 0}
            onClick={clearActivity}
            aria-label="Clear activity log"
          >
            <Trash2 />
          </Button>
        </div>
      </div>

      {groups.length === 0 ? (
        <p className="text-muted-foreground py-12 text-center text-sm">
          {snapshot.entries.length === 0
            ? 'Nothing logged yet — scan a folder and activity will show up here.'
            : 'No entries at this level.'}
        </p>
      ) : (
        <div className="flex flex-col gap-3">
          {groups.map((group) => {
            const isCollapsed = collapsed.has(group.operation.id)
            return (
              <section key={group.operation.id} className="bg-card rounded-lg border">
                <button
                  type="button"
                  className="flex w-full items-center justify-between gap-2 px-3 py-2 text-left text-sm font-medium"
                  onClick={() => toggle(group.operation.id)}
                  aria-expanded={!isCollapsed}
                >
                  <span className="truncate">{operationTitle(group)}</span>
                  <span className="text-muted-foreground flex shrink-0 items-center gap-2 text-xs">
                    {group.entries.length} entries
                    <ChevronDown
                      className={`size-4 transition-transform ${isCollapsed ? '-rotate-90' : ''}`}
                    />
                  </span>
                </button>
                {!isCollapsed && (
                  <ol className="flex flex-col gap-1 border-t px-2 py-2 font-mono text-xs">
                    {group.entries.map((e) => (
                      <li key={e.seq} className="rounded px-2 py-0.5 leading-relaxed">
                        <span className="text-muted-foreground tabular-nums">
                          {timeOf(e.timestamp)}
                        </span>{' '}
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
              </section>
            )
          })}
        </div>
      )}
    </div>
  )
}
```

(Newest-operation-first comes from `groupLogByOperation`; the old `flex-col-reverse` trick is gone — entries render top-down inside each group.)

- [ ] **Step 2: Captured path on clip rows (maintainer-styled file — minimal diff)**

In `apps/web/src/features/report/clip-card.tsx` (git status first; STOP if uncommitted): locate where the clip's `fileName` renders. Add — matching the file's existing typography classes for secondary mono text — one path line under it (or extend the existing secondary line), showing the full captured path. The component receives its data from the report model; the exact wiring depends on the file's current props (read it first). Required outcome, verbatim content:

```
{report.sourceRoot ? `${report.sourceRoot}/${clip.relativePath}` : clip.relativePath}
```

rendered with `title` set to the same string and `truncate` so long paths ellipsize. If `clip-card.tsx` doesn't receive the report/sourceRoot today, thread `sourceRoot: string` down as a prop from `report-workspace.tsx` (model already carries it after Task 5). Keep every existing class; add nothing else.

- [ ] **Step 3: 404 rebuild**

In `apps/web/src/routes/__root.tsx`, replace the `notFoundComponent` with:

```tsx
  notFoundComponent: () => (
    <div className="flex min-h-[50dvh] flex-col items-center justify-center gap-6 text-center">
      <Logo className="h-14 w-auto opacity-80" />
      <div className="space-y-2">
        <p className="text-primary font-mono text-sm tracking-[0.3em] uppercase">404</p>
        <h1 className="text-3xl font-semibold tracking-tight">This reel isn't on the card</h1>
        <p className="text-muted-foreground mx-auto max-w-sm text-sm leading-relaxed">
          The page you're looking for doesn't exist — it may have been moved, renamed, or never
          shot in the first place.
        </p>
      </div>
      <div className="flex items-center gap-2">
        <Link to="/" className={cn(buttonVariants({ variant: 'default' }))}>
          Back to Luna
        </Link>
        <a href="/docs" className={cn(buttonVariants({ variant: 'outline' }))}>
          Docs
        </a>
      </div>
    </div>
  ),
```

adding `import { Logo } from '@/components/logo'` to the imports.

- [ ] **Step 4: All four gates, commit**

```bash
git add apps/web/src/features/activity/activity-screen.tsx apps/web/src/features/report/clip-card.tsx apps/web/src/routes/__root.tsx
git commit -m "feat(web): operation-grouped activity view, captured clip paths, proper 404"
```

---

## Definition of done

- All four gates green from repo root; no changes under `apps/web/src/components/ui/` or `tools/`; maintainer's PDF files untouched except (at most) one zero-frame guard.
- Final whole-plan review (opus), fixes + re-review, ledger close.

### Maintainer QA checklist (manual, Chromium)

1. **Settings v2 migration**: after first launch, existing settings survive (cap/defaults kept) and "Generate thumbnails by default" appears ON. (Your v1 record migrates in place.)
2. **Editable defaults**: type into the defaults form fields → reload → cover pre-fills; clearing a field removes it; logo pick/remove round-trips; "Copy from current cover" still works.
3. **Thumbnail opt-out**: uncheck in settings → next scan's summary shows it unchecked; check the per-run box → thumbnails run anyway. With it off: processing goes straight to the report, no thumbnail phase, CSV shows `NotAttempted`, PDF renders without strips, workspace shows placeholder tiles (no crashes).
4. **Activity per operation**: run a scan + process + export → `/activity` shows three collapsible groups, newest first, each labeled and timestamped. **Refresh the page** → groups still there. Clear works. Download shows operation headings.
5. **Paths**: workspace clip rows show `CARD/relative/path` (truncated, hover for full); CSV has the `path` column; reel/other-files paths unchanged from your PDF band work.
6. **Clear local data**: settings → Clear → confirm → app reloads with factory defaults, recents gone, activity empty, and the ffmpeg engine re-downloads on next use.
7. **404**: garbage URL shows the new page inside the app chrome; both buttons work.
8. **Regression sweep**: normal thumbnail run unchanged; exports work; stale-recent flow (Plan 09) unchanged.

## Self-review notes

- Feedback coverage: editable defaults (T6), per-operation + persisted activity (T2/T4/T7), thumbnail opt-out with per-run option (T1/T5/T6), captured paths in workspace/CSV (T3/T5/T7 — browser cannot expose absolute paths; `sourceRoot/relativePath` is the honest maximum), clear local data (T6), 404 (T7). Reportbook deferred per maintainer.
- Type consistency: `ActivitySnapshot` everywhere app-side; `beginOperation(kind, label)` matches all call sites; `Settings.generateThumbnails` name identical in core/store/screens; CSV `path` derives from `report.sourceRoot` exactly as the workspace/clip-card render it.
- Coordination-sensitive files each carry stop-if-uncommitted orders: `scan-screen.tsx` (T5), `report-workspace.tsx` (T5), `clip-card.tsx` (T7), `pdf-document.tsx` (T5, verify-only).
- Known deliberate choices: activity persistence is debounced write-through (400 ms) — a hard browser kill can lose the last moments of a log, acceptable for a diagnostic surface; the per-run checkbox writes directly to `scanStore` (run state, not a setting); operations without entries are dropped from display but retained in the snapshot until capped.
