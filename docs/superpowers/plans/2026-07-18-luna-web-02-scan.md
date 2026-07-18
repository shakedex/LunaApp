# Luna Web — Plan 02: Folder Scan Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A user can pick a footage folder (or reuse a recent one), Luna Web recursively scans it, shows a pre-scan summary (clip count, RAW notices, total size), and on confirm shows the clip list — all local, nothing uploaded.

**Architecture:** The recursive walker is pure logic in `packages/core`, written against *structural* handle interfaces (`DirectoryHandleLike`/`FileHandleLike`) so it stays DOM-free and fully `bun test`-able with fake handles; the real `FileSystemDirectoryHandle` satisfies the shape at one documented boundary cast in the app. The app adds a TanStack Store for scan state, an idb-backed recent-sources store (directory handles are structured-cloneable into IndexedDB only), and the `/` route becomes the scan screen.

**Tech Stack (additions):** `idb`, `@tanstack/store` + `@tanstack/react-store`, `@types/wicg-file-system-access` — all added via `bun add` at latest.

**Spec:** `docs/superpowers/specs/2026-07-17-luna-web-design.md` §4 (user flow steps 1–5), §8.2 (FolderScanner), §8.12 (persistence/recent sources), §10.1 (allowlist).

## Global Constraints

- **Never hand-write a dependency version.** All new deps via `bun add` / `bun add -d` (latest). The workspace is unified on **TypeScript 6** (Astro tooling ceiling) — do not touch TS versions.
- **`packages/core` stays DOM-free**: no `window`, `document`, or DOM lib types. The walker uses the structural handle interfaces defined in this plan — never `FileSystemDirectoryHandle` in core.
- **Tests:** `bun test` only, and only in `packages/core`. No Vitest, no Playwright. UI flows are verified manually by the maintainer (non-blocking checklist at the end).
- **Gates must stay green after every task**: from `web/` — `bun run lint`, `bun run typecheck`, `bun test`, `bun run build` all exit 0 before each commit.
- **App scripts** are `tsr generate && tsc -b --noEmit [&& vite build]` — leave them as-is.
- **shadcn components**: reuse the existing `@/components/ui/button`. This plan adds NO new shadcn components and has NO interactive-CLI handoffs.
- **Biome**: `web/biome.json` `files.includes` is `["**", "!docs", "!**/*.css", "!**/*.svg", "!app/src/components/ui"]` — hand-written app/core code must lint clean; run `bunx @biomejs/biome check --write .` from `web/` before committing if needed.
- **No COOP/COEP, no SharedArrayBuffer.** The existing .NET app at the repo root is never touched.
- **One documented boundary cast**: `handle as unknown as DirectoryHandleLike` where the real browser handle enters core (TS's `FileSystemDirectoryHandle.entries()` yields base `FileSystemHandle`, which structurally lacks the discriminated members; the runtime objects match).

---

## File Structure

```
web/packages/core/src/
  media/extensions.ts        MODIFY: export fileExtensionOf (was private extensionOf)
  scan/handles.ts            NEW: FileHandleLike / DirectoryHandleLike / FileSystemEntryLike
  scan/junk.ts               NEW: isJunkName (dot-prefixed + known junk dirs)
  scan/model.ts              NEW: ClipRef, RawNotice, ScanSummary, buildScanSummary
  scan/walker.ts             NEW: scanFolder, ScanProgress, ScanResult
  index.ts                   MODIFY: export the new modules
web/packages/core/test/
  extensions.test.ts         MODIFY: add fileExtensionOf cases
  scan-model.test.ts         NEW
  walker.test.ts             NEW
web/app/src/
  persistence/db.ts          NEW: idb database (luna-web, v1, recentSources store)
  persistence/recent-sources.ts  NEW: list/remember/forget recent sources
  features/scan/permissions.ts   NEW: ensureReadPermission
  features/scan/store.ts         NEW: TanStack scan store
  features/scan/run-scan.ts      NEW: pickAndScan / scanFrom / confirmScan / resetScan
  features/scan/scan-screen.tsx  NEW: phase-driven screen
  features/scan/recent-list.tsx  NEW: recent sources list (Task 5)
  lib/format.ts              NEW: formatBytes
  routes/index.tsx           MODIFY: render <ScanScreen />
web/app/tsconfig.app.json    MODIFY: add "wicg-file-system-access" to types
```

---

## Task 1: Core scan model — extension helper, junk filter, summary (TDD)

**Files:**
- Modify: `web/packages/core/src/media/extensions.ts`, `web/packages/core/src/index.ts`, `web/packages/core/test/extensions.test.ts`
- Create: `web/packages/core/src/scan/junk.ts`, `web/packages/core/src/scan/handles.ts`, `web/packages/core/src/scan/model.ts`
- Test: `web/packages/core/test/scan-model.test.ts`

**Interfaces:**
- Consumes: nothing new.
- Produces (exact, used by Tasks 2–4):
  - `fileExtensionOf(fileName: string): string` — lowercased with dot (`'.mov'`), `''` for no extension or dotfiles.
  - `isJunkName(name: string): boolean` — true for names starting with `.` and known junk dirs.
  - `interface FileHandleLike { readonly kind: 'file'; readonly name: string; getFile(): Promise<{ readonly size: number }> }`
  - `interface DirectoryHandleLike { readonly kind: 'directory'; readonly name: string; entries(): AsyncIterable<readonly [string, FileSystemEntryLike]> }`
  - `type FileSystemEntryLike = FileHandleLike | DirectoryHandleLike`
  - `interface ClipRef { id: string; fileName: string; relativePath: string; extension: string; sizeBytes: number; file: FileHandleLike }` (`id` = `relativePath`, unique within a scan)
  - `interface RawNotice` — same fields as `ClipRef`.
  - `interface ScanSummary { clipCount: number; rawCount: number; totalClipSizeBytes: number; byExtension: Record<string, number> }`
  - `buildScanSummary(clips: readonly ClipRef[], raw: readonly RawNotice[]): ScanSummary`

- [ ] **Step 1: Write the failing tests**

Append to `web/packages/core/test/extensions.test.ts`:

```ts
describe('fileExtensionOf', () => {
  test('lowercases and keeps the dot', () => {
    expect(fileExtensionOf('CLIP.MOV')).toBe('.mov')
    expect(fileExtensionOf('a.tar.gz')).toBe('.gz')
  })
  test('empty for no extension and dotfiles', () => {
    expect(fileExtensionOf('noext')).toBe('')
    expect(fileExtensionOf('.DS_Store')).toBe('')
  })
})
```
(add `fileExtensionOf` to the existing import from `../src/media/extensions`)

Create `web/packages/core/test/scan-model.test.ts`:

```ts
import { describe, expect, test } from 'bun:test'
import { isJunkName } from '../src/scan/junk'
import { buildScanSummary, type ClipRef, type RawNotice } from '../src/scan/model'
import type { FileHandleLike } from '../src/scan/handles'

const fakeFile: FileHandleLike = { kind: 'file', name: 'x', getFile: async () => ({ size: 0 }) }
const clip = (relativePath: string, extension: string, sizeBytes: number): ClipRef => ({
  id: relativePath, fileName: relativePath.split('/').pop() ?? relativePath,
  relativePath, extension, sizeBytes, file: fakeFile,
})

describe('isJunkName', () => {
  test('dot-prefixed and known junk dirs are junk', () => {
    expect(isJunkName('.hidden')).toBe(true)
    expect(isJunkName('.DS_Store')).toBe(true)
    expect(isJunkName('System Volume Information')).toBe(true)
    expect(isJunkName('$RECYCLE.BIN')).toBe(true)
    expect(isJunkName('__MACOSX')).toBe(true)
  })
  test('normal names are not junk', () => {
    expect(isJunkName('A001')).toBe(false)
    expect(isJunkName('CARD01')).toBe(false)
  })
})

describe('buildScanSummary', () => {
  test('counts, sums clip bytes, groups by extension', () => {
    const clips = [clip('a/1.mov', '.mov', 100), clip('a/2.mov', '.mov', 50), clip('b/3.mxf', '.mxf', 25)]
    const raw: RawNotice[] = [clip('c/4.r3d', '.r3d', 999)]
    const s = buildScanSummary(clips, raw)
    expect(s.clipCount).toBe(3)
    expect(s.rawCount).toBe(1)
    expect(s.totalClipSizeBytes).toBe(175)
    expect(s.byExtension).toEqual({ '.mov': 2, '.mxf': 1 })
  })
  test('empty scan', () => {
    expect(buildScanSummary([], [])).toEqual({ clipCount: 0, rawCount: 0, totalClipSizeBytes: 0, byExtension: {} })
  })
})
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cd web/packages/core && bun test`
Expected: FAIL — `fileExtensionOf` not exported; `../src/scan/junk` and `../src/scan/model` don't exist.

- [ ] **Step 3: Implement**

In `web/packages/core/src/media/extensions.ts`, replace the private `extensionOf` with an exported helper (predicates now call it):

```ts
export function fileExtensionOf(fileName: string): string {
  const dot = fileName.lastIndexOf('.')
  return dot <= 0 ? '' : fileName.slice(dot).toLowerCase()
}
```
(`dot <= 0` makes dotfiles like `.DS_Store` extensionless. Update `isSupportedMediaExtension`/`isKnownRawExtension` to use `fileExtensionOf`.)

Create `web/packages/core/src/scan/junk.ts`:

```ts
// Browser file handles expose no hidden/system attributes; skip by name instead
// (parity with the desktop scanner's Hidden/System filter).
const JUNK_DIRECTORY_NAMES = new Set([
  'system volume information',
  '$recycle.bin',
  '__macosx',
  'lost+found',
])

export function isJunkName(name: string): boolean {
  return name.startsWith('.') || JUNK_DIRECTORY_NAMES.has(name.toLowerCase())
}
```

Create `web/packages/core/src/scan/handles.ts`:

```ts
// Structural mirrors of the File System Access API, so the walker stays
// DOM-free and testable. The real FileSystemDirectoryHandle satisfies this
// shape at runtime; the app casts once at the boundary.
export interface FileHandleLike {
  readonly kind: 'file'
  readonly name: string
  getFile(): Promise<{ readonly size: number }>
}

export interface DirectoryHandleLike {
  readonly kind: 'directory'
  readonly name: string
  entries(): AsyncIterable<readonly [string, FileSystemEntryLike]>
}

export type FileSystemEntryLike = FileHandleLike | DirectoryHandleLike
```

Create `web/packages/core/src/scan/model.ts`:

```ts
import type { FileHandleLike } from './handles'

export interface ClipRef {
  id: string // relativePath — unique within one scan
  fileName: string
  relativePath: string
  extension: string
  sizeBytes: number
  file: FileHandleLike
}

export interface RawNotice {
  id: string
  fileName: string
  relativePath: string
  extension: string
  sizeBytes: number
  file: FileHandleLike
}

export interface ScanSummary {
  clipCount: number
  rawCount: number
  totalClipSizeBytes: number
  byExtension: Record<string, number>
}

export function buildScanSummary(clips: readonly ClipRef[], raw: readonly RawNotice[]): ScanSummary {
  const byExtension: Record<string, number> = {}
  let totalClipSizeBytes = 0
  for (const c of clips) {
    totalClipSizeBytes += c.sizeBytes
    byExtension[c.extension] = (byExtension[c.extension] ?? 0) + 1
  }
  return { clipCount: clips.length, rawCount: raw.length, totalClipSizeBytes, byExtension }
}
```

Add to `web/packages/core/src/index.ts`:

```ts
export { fileExtensionOf } from './media/extensions' // add to the existing export list
export { isJunkName } from './scan/junk'
export type { DirectoryHandleLike, FileHandleLike, FileSystemEntryLike } from './scan/handles'
export { buildScanSummary } from './scan/model'
export type { ClipRef, RawNotice, ScanSummary } from './scan/model'
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cd web/packages/core && bun test`
Expected: PASS — all previous + new tests green.

- [ ] **Step 5: Gates and commit**

Run from `web/`: `bun run lint && bun run typecheck && bun test` — all exit 0 (run `bunx @biomejs/biome check --write .` first if lint complains about formatting).

```bash
git add web/packages/core
git commit -m "feat(core): scan model, junk filter, and extension helper"
```

---

## Task 2: Core recursive walker (TDD with fake handles)

**Files:**
- Create: `web/packages/core/src/scan/walker.ts`
- Modify: `web/packages/core/src/index.ts`
- Test: `web/packages/core/test/walker.test.ts`

**Interfaces:**
- Consumes: Task 1's handles/model/junk/extensions.
- Produces (exact, used by Task 3):
  - `interface ScanProgress { filesSeen: number; clipsFound: number; currentDir: string }`
  - `interface ScanResult { clips: ClipRef[]; raw: RawNotice[] }`
  - `scanFolder(root: DirectoryHandleLike, onProgress?: (p: ScanProgress) => void): Promise<ScanResult>`

- [ ] **Step 1: Write the failing test**

Create `web/packages/core/test/walker.test.ts`:

```ts
import { describe, expect, test } from 'bun:test'
import type { DirectoryHandleLike, FileHandleLike, FileSystemEntryLike } from '../src/scan/handles'
import { scanFolder, type ScanProgress } from '../src/scan/walker'

function file(name: string, size: number): FileHandleLike {
  return { kind: 'file', name, getFile: async () => ({ size }) }
}
function dir(name: string, ...children: FileSystemEntryLike[]): DirectoryHandleLike {
  return {
    kind: 'directory',
    name,
    async *entries() {
      for (const c of children) yield [c.name, c] as const
    },
  }
}

const card = dir(
  'CARD01',
  dir('A001', file('A001C001.mov', 100), file('A001C002.MP4', 50), file('notes.txt', 1)),
  dir('.hidden', file('secret.mov', 10)),
  dir('System Volume Information', file('x.mov', 10)),
  file('shot.R3D', 999),
  file('.DS_Store', 1),
)

describe('scanFolder', () => {
  test('finds nested clips with relative paths and sizes', async () => {
    const { clips } = await scanFolder(card)
    expect(clips.map((c) => c.relativePath)).toEqual(['A001/A001C001.mov', 'A001/A001C002.MP4'])
    expect(clips[0]).toMatchObject({ id: 'A001/A001C001.mov', fileName: 'A001C001.mov', extension: '.mov', sizeBytes: 100 })
    expect(clips[1]?.extension).toBe('.mp4')
  })

  test('routes known RAW to raw notices, skips junk and non-media', async () => {
    const { clips, raw } = await scanFolder(card)
    expect(raw.map((r) => r.relativePath)).toEqual(['shot.R3D'])
    expect(raw[0]?.extension).toBe('.r3d')
    const all = [...clips, ...raw].map((e) => e.relativePath).join()
    expect(all).not.toContain('secret')
    expect(all).not.toContain('x.mov')
    expect(all).not.toContain('notes.txt')
  })

  test('reports progress and counts only non-junk files', async () => {
    const seen: ScanProgress[] = []
    await scanFolder(card, (p) => seen.push({ ...p }))
    expect(seen.length).toBeGreaterThan(0)
    const last = seen[seen.length - 1]
    expect(last?.filesSeen).toBe(4) // 2 clips + notes.txt + shot.R3D (.DS_Store junk-skipped)
    expect(last?.clipsFound).toBe(2)
  })
})
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cd web/packages/core && bun test walker`
Expected: FAIL — `../src/scan/walker` does not exist.

- [ ] **Step 3: Implement `web/packages/core/src/scan/walker.ts`**

```ts
import { fileExtensionOf, isKnownRawExtension, isSupportedMediaExtension } from '../media/extensions'
import type { DirectoryHandleLike, FileHandleLike } from './handles'
import { isJunkName } from './junk'
import type { ClipRef, RawNotice } from './model'

export interface ScanProgress {
  filesSeen: number
  clipsFound: number
  currentDir: string
}

export interface ScanResult {
  clips: ClipRef[]
  raw: RawNotice[]
}

export async function scanFolder(
  root: DirectoryHandleLike,
  onProgress?: (p: ScanProgress) => void,
): Promise<ScanResult> {
  const clips: ClipRef[] = []
  const raw: RawNotice[] = []
  let filesSeen = 0

  async function walk(dir: DirectoryHandleLike, prefix: string): Promise<void> {
    for await (const [, entry] of dir.entries()) {
      if (entry.kind === 'directory') {
        if (!isJunkName(entry.name)) await walk(entry, `${prefix}${entry.name}/`)
        continue
      }
      if (isJunkName(entry.name)) continue
      filesSeen += 1
      const relativePath = `${prefix}${entry.name}`
      if (isSupportedMediaExtension(entry.name)) {
        clips.push(await toRef(entry, relativePath))
      } else if (isKnownRawExtension(entry.name)) {
        raw.push(await toRef(entry, relativePath))
      }
      onProgress?.({ filesSeen, clipsFound: clips.length, currentDir: prefix })
    }
  }

  await walk(root, '')
  return { clips, raw }
}

async function toRef(entry: FileHandleLike, relativePath: string): Promise<ClipRef> {
  const { size } = await entry.getFile()
  return {
    id: relativePath,
    fileName: entry.name,
    relativePath,
    extension: fileExtensionOf(entry.name),
    sizeBytes: size,
    file: entry,
  }
}
```

Add to `web/packages/core/src/index.ts`:

```ts
export { scanFolder } from './scan/walker'
export type { ScanProgress, ScanResult } from './scan/walker'
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cd web/packages/core && bun test`
Expected: PASS — all suites green.

- [ ] **Step 5: Gates and commit**

Run from `web/`: `bun run lint && bun run typecheck && bun test` — exit 0.

```bash
git add web/packages/core
git commit -m "feat(core): recursive folder scanner over structural handles"
```

---

## Task 3: App wiring — deps, persistence, permissions, store, orchestration

**Files:**
- Modify: `web/app/tsconfig.app.json` (types array), `web/app/package.json` + `web/bun.lock` (via bun add)
- Create: `web/app/src/persistence/db.ts`, `web/app/src/persistence/recent-sources.ts`, `web/app/src/features/scan/permissions.ts`, `web/app/src/features/scan/store.ts`, `web/app/src/features/scan/run-scan.ts`, `web/app/src/lib/format.ts`

**Interfaces:**
- Consumes: core's `scanFolder`, `buildScanSummary`, `ClipRef`, `RawNotice`, `ScanSummary`, `DirectoryHandleLike`.
- Produces (exact, used by Tasks 4–5):
  - `interface StoredRecentSource { name: string; handle: FileSystemDirectoryHandle; lastUsedAt: number }`
  - `listRecentSources(): Promise<Array<{ key: number } & StoredRecentSource>>` (newest first)
  - `rememberSource(handle: FileSystemDirectoryHandle, now: number): Promise<void>` (dedupes via `isSameEntry`, caps at 10)
  - `forgetSource(key: number): Promise<void>`
  - `ensureReadPermission(handle: FileSystemDirectoryHandle): Promise<boolean>`
  - `type ScanPhase = 'idle' | 'scanning' | 'summary' | 'confirmed' | 'error'`
  - `scanStore: Store<ScanState>` with `interface ScanState { phase: ScanPhase; sourceName: string | null; progress: { filesSeen: number; clipsFound: number } | null; clips: ClipRef[]; raw: RawNotice[]; summary: ScanSummary | null; error: string | null }`
  - `pickAndScan(): Promise<void>`, `scanFrom(handle: FileSystemDirectoryHandle): Promise<void>`, `confirmScan(): void`, `resetScan(): void`
  - `formatBytes(n: number): string` (e.g. `'1.5 GB'`)

- [ ] **Step 1: Add dependencies (latest, via bun)**

Run: `cd web/app && bun add idb @tanstack/store @tanstack/react-store && bun add -d @types/wicg-file-system-access`
Expected: all four land in `web/app/package.json` with bun-resolved versions.

- [ ] **Step 2: Make the WICG types visible to tsc**

`web/app/tsconfig.app.json` sets an explicit `types` array (which disables automatic `@types` inclusion). Change it to:

```json
"types": ["vite/client", "wicg-file-system-access"]
```

- [ ] **Step 3: Create `web/app/src/persistence/db.ts`**

```ts
import { type DBSchema, type IDBPDatabase, openDB } from 'idb'
import type { StoredRecentSource } from './recent-sources'

interface LunaDb extends DBSchema {
  recentSources: { key: number; value: StoredRecentSource }
}

let dbPromise: Promise<IDBPDatabase<LunaDb>> | null = null

export function getDb(): Promise<IDBPDatabase<LunaDb>> {
  dbPromise ??= openDB<LunaDb>('luna-web', 1, {
    upgrade(db) {
      db.createObjectStore('recentSources', { autoIncrement: true })
    },
  })
  return dbPromise
}
```

- [ ] **Step 4: Create `web/app/src/persistence/recent-sources.ts`**

Directory handles are structured-cloneable into IndexedDB (the reason idb exists in this app at all). Never `await` a non-IDB promise inside an open transaction (it auto-commits) — so `isSameEntry` comparisons happen between transactions:

```ts
import { getDb } from './db'

export interface StoredRecentSource {
  name: string
  handle: FileSystemDirectoryHandle
  lastUsedAt: number
}

const MAX_RECENT = 10

export async function listRecentSources(): Promise<Array<{ key: number } & StoredRecentSource>> {
  const db = await getDb()
  const [keys, values] = await Promise.all([db.getAllKeys('recentSources'), db.getAll('recentSources')])
  return keys
    .map((key, i) => ({ key, ...values[i] }) as { key: number } & StoredRecentSource)
    .sort((a, b) => b.lastUsedAt - a.lastUsedAt)
}

export async function rememberSource(handle: FileSystemDirectoryHandle, now: number): Promise<void> {
  const db = await getDb()
  const [keys, values] = await Promise.all([db.getAllKeys('recentSources'), db.getAll('recentSources')])

  const duplicateKeys: number[] = []
  for (let i = 0; i < values.length; i++) {
    const existing = values[i]
    if (existing && (await existing.handle.isSameEntry(handle))) duplicateKeys.push(keys[i] as number)
  }

  const tx = db.transaction('recentSources', 'readwrite')
  for (const k of duplicateKeys) void tx.store.delete(k)
  void tx.store.add({ name: handle.name, handle, lastUsedAt: now })
  await tx.done

  const all = await listRecentSources()
  if (all.length > MAX_RECENT) {
    const tx2 = db.transaction('recentSources', 'readwrite')
    for (const stale of all.slice(MAX_RECENT)) void tx2.store.delete(stale.key)
    await tx2.done
  }
}

export async function forgetSource(key: number): Promise<void> {
  const db = await getDb()
  await db.delete('recentSources', key)
}
```

- [ ] **Step 5: Create `web/app/src/features/scan/permissions.ts`**

```ts
export async function ensureReadPermission(handle: FileSystemDirectoryHandle): Promise<boolean> {
  if ((await handle.queryPermission({ mode: 'read' })) === 'granted') return true
  return (await handle.requestPermission({ mode: 'read' })) === 'granted'
}
```

- [ ] **Step 6: Create `web/app/src/features/scan/store.ts`**

```ts
import type { ClipRef, RawNotice, ScanSummary } from '@luna-web/core'
import { Store } from '@tanstack/store'

export type ScanPhase = 'idle' | 'scanning' | 'summary' | 'confirmed' | 'error'

export interface ScanState {
  phase: ScanPhase
  sourceName: string | null
  progress: { filesSeen: number; clipsFound: number } | null
  clips: ClipRef[]
  raw: RawNotice[]
  summary: ScanSummary | null
  error: string | null
}

export const initialScanState: ScanState = {
  phase: 'idle',
  sourceName: null,
  progress: null,
  clips: [],
  raw: [],
  summary: null,
  error: null,
}

export const scanStore = new Store<ScanState>(initialScanState)
```

- [ ] **Step 7: Create `web/app/src/features/scan/run-scan.ts`**

```ts
import { buildScanSummary, type DirectoryHandleLike, scanFolder } from '@luna-web/core'
import { rememberSource } from '@/persistence/recent-sources'
import { ensureReadPermission } from './permissions'
import { initialScanState, scanStore } from './store'

const PROGRESS_EVERY = 25 // throttle store updates on huge cards

export async function pickAndScan(): Promise<void> {
  let handle: FileSystemDirectoryHandle
  try {
    handle = await window.showDirectoryPicker()
  } catch {
    return // user cancelled the picker
  }
  await scanFrom(handle)
}

export async function scanFrom(handle: FileSystemDirectoryHandle): Promise<void> {
  if (!(await ensureReadPermission(handle))) {
    scanStore.setState((s) => ({ ...s, phase: 'error', error: 'Read permission was denied for this folder.' }))
    return
  }
  scanStore.setState(() => ({ ...initialScanState, phase: 'scanning', sourceName: handle.name }))
  try {
    // Boundary cast: the real FileSystemDirectoryHandle satisfies DirectoryHandleLike
    // at runtime; TS's lib types yield base FileSystemHandle from entries().
    const result = await scanFolder(handle as unknown as DirectoryHandleLike, (p) => {
      if (p.filesSeen % PROGRESS_EVERY === 0) {
        scanStore.setState((s) => ({ ...s, progress: { filesSeen: p.filesSeen, clipsFound: p.clipsFound } }))
      }
    })
    await rememberSource(handle, Date.now())
    scanStore.setState((s) => ({
      ...s,
      phase: 'summary',
      clips: result.clips,
      raw: result.raw,
      progress: { filesSeen: result.clips.length + result.raw.length, clipsFound: result.clips.length },
      summary: buildScanSummary(result.clips, result.raw),
    }))
  } catch (err) {
    scanStore.setState((s) => ({
      ...s,
      phase: 'error',
      error: err instanceof Error ? err.message : String(err),
    }))
  }
}

export function confirmScan(): void {
  scanStore.setState((s) => (s.phase === 'summary' ? { ...s, phase: 'confirmed' } : s))
}

export function resetScan(): void {
  scanStore.setState(() => initialScanState)
}
```

- [ ] **Step 8: Create `web/app/src/lib/format.ts`**

```ts
const UNITS = ['B', 'KB', 'MB', 'GB', 'TB'] as const

export function formatBytes(n: number): string {
  if (n < 1024) return `${n} B`
  let value = n
  let unit = 0
  while (value >= 1024 && unit < UNITS.length - 1) {
    value /= 1024
    unit += 1
  }
  return `${value.toFixed(1)} ${UNITS[unit]}`
}
```

- [ ] **Step 9: Gates and commit**

Run from `web/`: `bun run lint && bun run typecheck && bun test && bun run build` — all exit 0.

```bash
git add web/app web/bun.lock
git commit -m "feat(app): scan orchestration, recent-source persistence, and store"
```

---

## Task 4: Scan screen UI

**Files:**
- Create: `web/app/src/features/scan/scan-screen.tsx`
- Modify: `web/app/src/routes/index.tsx`

**Interfaces:**
- Consumes: Task 3's store/actions, `formatBytes`, existing `Button` (`@/components/ui/button`), `useStore` from `@tanstack/react-store`.
- Produces: `<ScanScreen />` — the `/` route's content; phase-driven (idle → scanning → summary → confirmed / error).

- [ ] **Step 1: Create `web/app/src/features/scan/scan-screen.tsx`**

(The recent-sources list arrives in Task 5; idle shows only the pick button for now.)

```tsx
import { useStore } from '@tanstack/react-store'
import { Button } from '@/components/ui/button'
import { formatBytes } from '@/lib/format'
import { confirmScan, pickAndScan, resetScan } from './run-scan'
import { scanStore } from './store'

export function ScanScreen() {
  const state = useStore(scanStore)

  return (
    <main className="mx-auto flex min-h-screen max-w-3xl flex-col items-center justify-center gap-6 px-6 py-12">
      <h1 className="font-heading text-3xl font-semibold">Luna Web</h1>

      {state.phase === 'idle' && (
        <>
          <p className="text-muted-foreground">Pick a footage folder — everything stays on this device.</p>
          <Button onClick={() => void pickAndScan()}>Pick folder</Button>
        </>
      )}

      {state.phase === 'scanning' && (
        <p className="text-muted-foreground" aria-live="polite">
          Scanning {state.sourceName}… {state.progress ? `${state.progress.filesSeen} files, ${state.progress.clipsFound} clips` : ''}
        </p>
      )}

      {state.phase === 'summary' && state.summary && (
        <section className="w-full rounded-lg border p-6">
          <h2 className="mb-4 text-xl font-medium">{state.sourceName}</h2>
          <dl className="mb-4 grid grid-cols-3 gap-4 text-center">
            <div><dt className="text-muted-foreground text-sm">Clips</dt><dd className="text-2xl">{state.summary.clipCount}</dd></div>
            <div><dt className="text-muted-foreground text-sm">Total size</dt><dd className="text-2xl">{formatBytes(state.summary.totalClipSizeBytes)}</dd></div>
            <div><dt className="text-muted-foreground text-sm">RAW (unsupported)</dt><dd className="text-2xl">{state.summary.rawCount}</dd></div>
          </dl>
          {state.summary.rawCount > 0 && (
            <p className="text-muted-foreground mb-4 text-sm">
              {state.summary.rawCount} ARRIRAW/R3D/BRAW file(s) were detected but cannot be decoded in a browser — they will be listed without thumbnails.
            </p>
          )}
          <div className="flex gap-3">
            <Button onClick={confirmScan}>Process {state.summary.clipCount} clips</Button>
            <Button variant="outline" onClick={resetScan}>Cancel</Button>
          </div>
        </section>
      )}

      {state.phase === 'confirmed' && (
        <section className="w-full">
          <div className="mb-4 flex items-center justify-between">
            <h2 className="text-xl font-medium">{state.clips.length} clips ready</h2>
            <Button variant="outline" onClick={resetScan}>Start over</Button>
          </div>
          <ul className="divide-y rounded-lg border">
            {state.clips.map((c) => (
              <li key={c.id} className="flex items-center justify-between px-4 py-2 text-sm">
                <span className="truncate">{c.relativePath}</span>
                <span className="text-muted-foreground ml-4 shrink-0">{formatBytes(c.sizeBytes)}</span>
              </li>
            ))}
          </ul>
          <p className="text-muted-foreground mt-3 text-sm">Metadata and thumbnails arrive in the next milestone.</p>
        </section>
      )}

      {state.phase === 'error' && (
        <section className="text-center">
          <p className="text-destructive mb-4">{state.error}</p>
          <Button variant="outline" onClick={resetScan}>Back</Button>
        </section>
      )}
    </main>
  )
}
```
(If the existing `Button` has no `variant="outline"`, check `@/components/ui/button`'s variants and use the closest neutral variant it defines — do not modify the generated component.)

- [ ] **Step 2: Point the route at it — `web/app/src/routes/index.tsx`**

```tsx
import { createFileRoute } from '@tanstack/react-router'
import { ScanScreen } from '@/features/scan/scan-screen'

export const Route = createFileRoute('/')({
  component: ScanScreen,
})
```

- [ ] **Step 3: Gates and commit**

Run from `web/`: `bun run lint && bun run typecheck && bun test && bun run build` — all exit 0.

```bash
git add web/app/src
git commit -m "feat(app): folder scan screen"
```

---

## Task 5: Recent sources list with permission re-request

**Files:**
- Create: `web/app/src/features/scan/recent-list.tsx`
- Modify: `web/app/src/features/scan/scan-screen.tsx` (render the list in the idle phase)

**Interfaces:**
- Consumes: Task 3's `listRecentSources`/`forgetSource`/`scanFrom`.
- Produces: `<RecentList />` — shows up to 10 recent folders newest-first; clicking one re-requests permission and rescans; a remove control forgets an entry.

- [ ] **Step 1: Create `web/app/src/features/scan/recent-list.tsx`**

```tsx
import { useEffect, useState } from 'react'
import { Button } from '@/components/ui/button'
import { forgetSource, listRecentSources, type StoredRecentSource } from '@/persistence/recent-sources'
import { scanFrom } from './run-scan'

type Entry = { key: number } & StoredRecentSource

export function RecentList() {
  const [entries, setEntries] = useState<Entry[]>([])

  useEffect(() => {
    void listRecentSources().then(setEntries)
  }, [])

  if (entries.length === 0) return null

  return (
    <section className="w-full max-w-md">
      <h2 className="text-muted-foreground mb-2 text-sm font-medium">Recent folders</h2>
      <ul className="divide-y rounded-lg border">
        {entries.map((e) => (
          <li key={e.key} className="flex items-center justify-between px-4 py-2">
            <button
              type="button"
              className="truncate text-left text-sm hover:underline"
              onClick={() => void scanFrom(e.handle)}
            >
              {e.name}
            </button>
            <Button
              variant="ghost"
              size="sm"
              aria-label={`Remove ${e.name} from recent folders`}
              onClick={() => {
                void forgetSource(e.key).then(() => listRecentSources().then(setEntries))
              }}
            >
              ✕
            </Button>
          </li>
        ))}
      </ul>
    </section>
  )
}
```
(Same variant note as Task 4: use the generated Button's actual variants/sizes; don't modify the component.)

- [ ] **Step 2: Render it in the idle phase of `scan-screen.tsx`**

In the `state.phase === 'idle'` block, after the pick Button, add:

```tsx
<RecentList />
```
with the import `import { RecentList } from './recent-list'`.

- [ ] **Step 3: Gates and commit**

Run from `web/`: `bun run lint && bun run typecheck && bun test && bun run build` — all exit 0.

```bash
git add web/app/src
git commit -m "feat(app): recent sources with permission re-request"
```

---

## Definition of done (this plan)

- `bun test` green: extensions + scan-model + walker suites (core only).
- All gates green from `web/`: lint, typecheck, test, build.
- Five commits, one per task.
- **Manual QA (maintainer, non-blocking, in Chromium via `bun --filter '@luna-web/app' dev`):**
  1. Pick a folder with mixed content → permission prompt → summary shows correct clip count / size / RAW count.
  2. Confirm → clip list with relative paths and sizes.
  3. Cancel in the picker → app stays idle (no error).
  4. Reload the app → the folder appears under Recent folders; clicking it re-prompts for permission and rescans.
  5. Remove a recent entry → it disappears and stays gone after reload.
  6. A folder with zero media → summary shows 0 clips (confirm button reads "Process 0 clips" — acceptable for now).

## Self-review notes

- **Spec coverage:** §4 flow steps 1–5 (pick → permission → scan → summary → confirm) ✓; §8.2 FolderScanner (recursive, allowlist, junk skip, progress, pre-scan summary) ✓; §8.12 recent sources in IndexedDB with `requestPermission` on reuse, capped at 10 ✓; §10.1 allowlist + RAW detection notices ✓. Deferred by design: metadata/thumbnails (Plans 03–05), TanStack Table/Virtual results grid (Plan 06 — the confirmed-phase list here is a plain placeholder), settings persistence (Plan 08).
- **Placeholders:** none — every step has complete code/commands.
- **Type consistency:** `ClipRef`/`RawNotice`/`ScanSummary`/`ScanProgress`/`ScanResult` names match across core tasks and app imports; `StoredRecentSource`/`listRecentSources`/`rememberSource`/`forgetSource`/`ensureReadPermission`/`scanFrom`/`pickAndScan`/`confirmScan`/`resetScan` consistent across Tasks 3–5.
- **Known risks flagged to implementers:** idb transaction auto-commit vs `isSameEntry` (handled by design in Task 3 Step 4); the single boundary cast (documented); Button variant names depend on the generated shadcn/Base UI component (both UI tasks instruct checking rather than assuming).
