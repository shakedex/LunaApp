# Audit Remediation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Fix the confirmed findings from the 2026-07-20 codebase audit: drifted docs claims, copy-pasted utility snippets, magic values without a single source of truth, and UI-convention gaps.

**Architecture:** No new subsystems. Docs get factual corrections plus lightweight "keep in sync" guards; small pure helpers land in `packages/core` (when domain logic) or `apps/web/src/lib` (when web-only); magic values get named constants at their natural home; three screens adopt the existing `ui/empty.tsx` primitives.

**Tech Stack:** Bun workspace, TypeScript, React 19 + TanStack store/router, Tailwind v4 (`@theme inline`), `bun test`, Biome.

## Global Constraints

- Bun-only tooling: `bun test`, `bun run typecheck` (root fans out via `bun --filter '*' typecheck`), `bun run lint` (`biome check .`). Never npm/npx/yarn.
- No new dependencies anywhere in this plan.
- ZeroVer: no version bump from this work — every task is chore/refactor/docs, so **no changelog entry** (per the luna-release rule: invisible-to-user work gets none) and no `bun run release`.
- UI copy uses plain dev terminology; never invent feature names.
- `apps/web/src/components/ui/*` is Biome-excluded vendored shadcn code — do not restyle it; deleting an unused file is fine.
- React components read stores only via `useSelector` from `@tanstack/react-store` (never `useStore`).
- There is no React test infra in the repo, and adding one is out of scope. Pure helpers get `bun test` colocated tests (TDD). Component/CSS changes are verified by `bun run typecheck` + `bun run lint` + the browser preview instead — do not add testing-library.
- Commit after every task, message style per git log: `docs: …`, `docs(web): …`, `refactor(core): …`, `refactor(web): …`, `chore(web): …`.
- Verification commands run from the repo root `E:\Coding\LunaApp` unless a task says otherwise.

### Explicitly deferred (audited, decided against — do NOT implement)

- Generic Comlink worker factory: Vite needs the static `new Worker(new URL(...))` at each call site, so the abstraction saves ~3 lines per client. Not worth the indirection.
- `STORES` const for IndexedDB store names: the `LunaDb` schema type already makes a typo a compile error at every accessor.
- Shared MIME-string const: no type-safety gain over the literals the platform APIs take.
- `ConfirmButton` / `useDropTarget` extractions, Switch component, loading-state unification, toast/error-surface redesign: real inconsistencies, but each needs a design decision — separate brainstorm if wanted.
- Clip-tile status circles (`clip-tile.tsx:42-50`): icon-only floating overlays, not text badges — `Badge`'s fixed `h-5`/`px-2` text styling doesn't fit. They stay bespoke. (The scan-screen "Ready" pill IS a badge and converts in Task 14.)
- Renaming `features/report` vs `features/reports`: churn; the split (builder vs library) is coherent.
- `docs/file_proccessing` typo rename, branding-asset dedup, Biome coverage of `apps/docs`, docs tsconfig extending the workspace base, `packages/core` `build` alias: intentional-enough or cosmetic; revisit only if they bite.
- `formatBytes`/`formatDuration` move to core: single-sourced in web today; move only when a non-web consumer appears.

---

## Phase 1 — Docs corrections

### Task 1: Fix drifted factual claims in docs

**Files:**
- Modify: `README.md:30-32`
- Modify: `apps/docs/src/content/docs/changelog.md:18`
- Modify: `DEPLOY.md:73-74`
- Modify: `apps/docs/src/content/docs/privacy.md:9-10`
- Modify: `apps/docs/src/content/docs/faq.md:28`
- Modify: `apps/docs/src/content/docs/limitations.md:17`
- Modify: `DISCLAIMER.md:23-26`

**Interfaces:** none (prose only).

Ground truth for every edit below: exporters are PDF + CSV only (`apps/web/src/features/export/exporter.ts` registry gets exactly `pdf` and `csv`); vendor enrichers are ARRI, Sony, Canon, Blackmagic, Panasonic — **no RED** (`packages/core/src/metadata/vendors/registry.ts:10-16`); `__APP_VERSION__` surfaces in app header + Credits page + PDF footer; the ~31 MB is the FFmpeg core from jsDelivr, MediaInfo is ~2.5 MB served with the app (`DEPLOY.md:54-56`).

- [ ] **Step 1: README.md — export formats and RAW examples**

Replace lines 30–32:

```markdown
- Produces clean, sharable camera reports with PDF and HTML export
- Raw formats (ARRIRAW, BRAW, Sony RAW) get metadata-aware handling; generic
  codecs (ProRes, H.264/265, DNxHD, …) get the full decode pipeline
```

with:

```markdown
- Produces clean, sharable camera reports with PDF and CSV export
- Raw formats (ARRIRAW, BRAW, R3D, Canon RAW) get metadata-aware handling; generic
  codecs (ProRes, H.264/265, DNxHD, …) get the full decode pipeline
```

(HTML export does not exist; "Sony RAW" is not a row in supported-formats.md — the four listed are.)

- [ ] **Step 2: docs changelog — correct the 0.12.0 vendor list**

In `apps/docs/src/content/docs/changelog.md` line 18, replace:

```markdown
- Metadata extraction across common cinema formats, with vendor enrichment for ARRI, Sony, Canon, Panasonic, and RED.
```

with:

```markdown
- Metadata extraction across common cinema formats, with vendor enrichment for ARRI, Sony, Canon, Blackmagic, and Panasonic.
```

- [ ] **Step 3: DEPLOY.md — version surfaces**

Replace lines 73–74:

```markdown
`apps/web/package.json` `version` — it surfaces in the app header
(`__APP_VERSION__`) and the PDF footer stamp. There will never be a 1.0.
```

with:

```markdown
`apps/web/package.json` `version` — it surfaces in the app header
(`__APP_VERSION__`), the Credits page, and the PDF footer stamp. There will never be a 1.0.
```

- [ ] **Step 4: privacy.md — attribute the 31 MB correctly**

Replace lines 9–10:

```markdown
The only things it downloads are the app itself and its decoding engines — FFmpeg and MediaInfo,
about 31 MB, fetched once from a CDN and then cached. After that first load, Luna works offline.
```

with:

```markdown
The only things it downloads are the app itself and its decoding engines — the FFmpeg core
(~31 MB, from a CDN) and MediaInfo (~2.5 MB) — each fetched once and then cached. After that
first load, Luna works offline.
```

- [ ] **Step 5: faq.md and limitations.md — same attribution**

`faq.md` line 28, replace:

```markdown
Yes, after the first visit. The decoding engines (~31 MB) download once and are then cached.
```

with:

```markdown
Yes, after the first visit. The decoding engines (the ~31 MB FFmpeg core and the smaller
MediaInfo engine) download once and are then cached.
```

`limitations.md` line 17, replace:

```markdown
**First load downloads ~31 MB** of decoding engines, cached afterward for offline use.
```

with:

```markdown
**First load downloads the decoding engines** (~31 MB, mostly the FFmpeg core), cached
afterward for offline use.
```

- [ ] **Step 6: DISCLAIMER.md — cover every vendor the code enriches**

After the Blackmagic bullet (line 24), insert three bullets so the list reads ARRI, Blackmagic, Canon, Panasonic, RED, Sony, Apple:

```markdown
- **Canon** — Canon, Cinema RAW Light, and the Canon logo are trademarks of Canon Inc.
- **Panasonic** — Panasonic, VariCam, and the Panasonic logo are trademarks of Panasonic Holdings Corporation.
- **RED** — RED, REDCODE, R3D, and the RED logo are trademarks of RED Digital Cinema, LLC.
```

(Canon and Panasonic are enriched by the code; RED's R3D format is named across the docs.)

- [ ] **Step 7: Verify and commit**

Run: `bun run lint` — expected: no new diagnostics (all files are .md; biome excludes docs, this is a sanity pass).
Re-read each edited hunk to confirm no surrounding text was mangled.

```bash
git add README.md DEPLOY.md DISCLAIMER.md apps/docs/src/content/docs/changelog.md apps/docs/src/content/docs/privacy.md apps/docs/src/content/docs/faq.md apps/docs/src/content/docs/limitations.md
git commit -m "docs: correct drifted claims (exports, vendors, version surfaces, engine sizes)"
```

### Task 2: Drift guards + dead-dir cleanup

**Files:**
- Modify: `packages/core/src/thumbs/router.ts:1` (top-of-file comment)
- Modify: `packages/core/src/metadata/vendors/registry.ts` (comment above the enricher list, lines ~10-16)
- Modify: `apps/docs/src/content/docs/supported-formats.md` (comment after frontmatter)
- Modify: `apps/web/src/index.css:53-54` (theme block comment)
- Modify: `apps/docs/src/styles/cinema-dark.css:1` (header comment)
- Modify: `apps/web/src/features/export/pdf-document.tsx:15` (palette comment)
- Delete: `docs/output-analysis/` (empty directory)

**Interfaces:** none (comments only — zero behavior change).

- [ ] **Step 1: Code↔doc sync comments**

Top of `packages/core/src/thumbs/router.ts`, add:

```ts
// The four-way extension split below is restated as a table in
// apps/docs/src/content/docs/supported-formats.md — update both together.
```

Above the enricher registration list in `packages/core/src/metadata/vendors/registry.ts`, add:

```ts
// The vendor list here is named in prose in
// apps/docs/src/content/docs/supported-formats.md — update both together.
```

In `apps/docs/src/content/docs/supported-formats.md`, directly after the frontmatter closing `---`, add:

```markdown
{/* Keep the table in sync with packages/core/src/thumbs/router.ts and the
    vendor sentence with packages/core/src/metadata/vendors/registry.ts. */}
```

Note: if the file is plain `.md` (not `.mdx`), use an HTML comment instead: `<!-- Keep the table in sync with … -->`.

- [ ] **Step 2: Palette mirror comments**

`apps/web/src/index.css` — extend the existing comment at lines 53–54 to:

```css
/* Cinema Dark — dark-only v1. Light values are intentionally identical to
   dark for now; a real light theme is a later additive pass.
   Mirrored by hand in apps/docs/src/styles/cinema-dark.css and (hex-resolved)
   in apps/web/src/features/export/pdf-document.tsx — edit all three together. */
```

`apps/docs/src/styles/cinema-dark.css` — add at the top:

```css
/* Hand-mirrored from apps/web/src/index.css (Cinema Dark) — edit both together. */
```

`apps/web/src/features/export/pdf-document.tsx` — replace the comment at line 15:

```ts
// Spec §3 palette — the app's Cinema Dark tokens, hex-resolved.
```

with:

```ts
// Spec §3 palette — the app's Cinema Dark tokens (apps/web/src/index.css),
// hex-resolved because react-pdf can't read CSS vars. Edit both together.
```

- [ ] **Step 3: Remove the empty directory**

Run: `Remove-Item docs/output-analysis` (it contains zero files; git does not track empty dirs, so no git change results — this is disk cleanup only).

- [ ] **Step 4: Verify and commit**

Run: `bun run typecheck` — expected: pass (comments only).
Run: `cd apps/docs; bun run build` — expected: builds clean (confirms the supported-formats comment syntax is valid for its format).

```bash
git add packages/core/src/thumbs/router.ts packages/core/src/metadata/vendors/registry.ts apps/docs/src/content/docs/supported-formats.md apps/web/src/index.css apps/docs/src/styles/cinema-dark.css apps/web/src/features/export/pdf-document.tsx
git commit -m "docs: add keep-in-sync guards for code-mirrored docs and palettes"
```

---

## Phase 2 — Shared helpers (dedup)

### Task 3: `errorMessage()` helper

**Files:**
- Create: `apps/web/src/lib/errors.ts`
- Test: `apps/web/src/lib/errors.test.ts`
- Modify: `apps/web/src/features/scan/run-scan.ts:80,84`
- Modify: `apps/web/src/features/export/export-buttons.tsx:24`
- Modify: `apps/web/src/features/export/exporter.ts:31`
- Modify: `apps/web/src/features/process/run-processing.ts:104,121,143`
- Modify: `apps/web/src/features/process/run-thumbnails.ts:113,136,173,191,214,220`

**Interfaces:**
- Produces: `errorMessage(err: unknown): string` — exact replacement for the inline `err instanceof Error ? err.message : String(err)` ternary.

- [ ] **Step 1: Write the failing test** — `apps/web/src/lib/errors.test.ts`:

```ts
import { expect, test } from 'bun:test'
import { errorMessage } from './errors'

test('unwraps an Error message', () => {
  expect(errorMessage(new Error('boom'))).toBe('boom')
})

test('stringifies non-Error throwables', () => {
  expect(errorMessage('raw string')).toBe('raw string')
  expect(errorMessage(42)).toBe('42')
  expect(errorMessage(undefined)).toBe('undefined')
})
```

- [ ] **Step 2: Run it** — `bun test apps/web/src/lib/errors.test.ts` — expected: FAIL (module not found).
- [ ] **Step 3: Implement** — `apps/web/src/lib/errors.ts`:

```ts
/** The one way to turn a caught unknown into a display/log string. */
export function errorMessage(err: unknown): string {
  return err instanceof Error ? err.message : String(err)
}
```

- [ ] **Step 4: Run it** — `bun test apps/web/src/lib/errors.test.ts` — expected: PASS (2 tests).
- [ ] **Step 5: Replace all 13 inline copies**

In each of the five feature files, add `import { errorMessage } from '@/lib/errors'` and replace every `err instanceof Error ? err.message : String(err)` with `errorMessage(err)`. Examples of the resulting lines:

```ts
// run-scan.ts:80
logger.error('Scan failed', errorMessage(err))
// export-buttons.tsx:24
.catch((err) => setError(errorMessage(err)))
// run-processing.ts:104 (same shape at 121, 143 and the run-thumbnails sites)
const message = errorMessage(err)
```

- [ ] **Step 6: Verify no stragglers**

Run: `rg -n "instanceof Error \? err\.message" apps/web/src packages/core/src` — expected: no matches.
Run: `bun run typecheck` — expected: pass.

- [ ] **Step 7: Commit**

```bash
git add apps/web/src/lib/errors.ts apps/web/src/lib/errors.test.ts apps/web/src/features
git commit -m "refactor(web): single errorMessage() helper for caught unknowns"
```

### Task 4: `todayIso()` helper

**Files:**
- Modify: `apps/web/src/lib/format.ts` (append)
- Test: `apps/web/src/lib/format.test.ts` (create)
- Modify: `apps/web/src/features/report/cover-store.ts:11`
- Modify: `apps/web/src/features/report/cover-form.tsx:32`
- Modify: `apps/web/src/features/export/save.ts:10`
- Modify: `apps/web/src/features/export/pdf-prepare.ts:147`
- Modify: `apps/web/src/features/activity/activity-screen.tsx:88`

**Interfaces:**
- Produces: `todayIso(): string` — today as `YYYY-MM-DD` (UTC), exported from `@/lib/format` alongside `formatBytes`/`formatDuration`.

- [ ] **Step 1: Write the failing test** — `apps/web/src/lib/format.test.ts`:

```ts
import { expect, test } from 'bun:test'
import { todayIso } from './format'

test('todayIso is a YYYY-MM-DD date', () => {
  expect(todayIso()).toMatch(/^\d{4}-\d{2}-\d{2}$/)
})
```

- [ ] **Step 2: Run it** — `bun test apps/web/src/lib/format.test.ts` — expected: FAIL (`todayIso` not exported).
- [ ] **Step 3: Implement** — append to `apps/web/src/lib/format.ts`:

```ts
/** Today as YYYY-MM-DD (UTC) — the cover-date default and export-filename stamp. */
export function todayIso(): string {
  return new Date().toISOString().slice(0, 10)
}
```

- [ ] **Step 4: Run it** — expected: PASS.
- [ ] **Step 5: Replace the 5 call sites**

Add `todayIso` to each file's `@/lib/format` import (or create the import) and replace `new Date().toISOString().slice(0, 10)`:

```ts
// cover-store.ts:11
export const coverStore = new Store<CoverFields<Blob>>({ date: todayIso() })
// cover-form.tsx:32
date: cover.date ?? todayIso(),
// save.ts:10
const day = date ?? todayIso()
// pdf-prepare.ts:147
date: report.cover.date ?? todayIso(),
// activity-screen.tsx:88
`luna-activity-${todayIso()}.txt`,
```

- [ ] **Step 6: Verify** — `rg -n "toISOString\(\)\.slice\(0, 10\)" apps/web/src` — expected: only `lib/format.ts`. Then `bun run typecheck` — pass.
- [ ] **Step 7: Commit**

```bash
git add apps/web/src/lib/format.ts apps/web/src/lib/format.test.ts apps/web/src/features
git commit -m "refactor(web): todayIso() replaces five inline date-stamp copies"
```

### Task 5: `useObjectUrl()` hook

**Files:**
- Create: `apps/web/src/lib/use-object-url.ts`
- Modify: `apps/web/src/features/scan/clip-tile.tsx:16-26`
- Modify: `apps/web/src/features/report/report-view.tsx:130-141` (`CoverLogo`)
- Modify: `apps/web/src/features/report/cover-form.tsx:84-96` (`LogoDropWell`)

**Interfaces:**
- Produces: `useObjectUrl(blob: Blob | null | undefined): string | null` — object URL with automatic revoke on change/unmount.
- Out of scope: `FrameViewer` in `clip-card.tsx:104-113` maps an *array* of blobs to URLs in one effect; a single-blob hook can't be called in a loop, so it keeps its bespoke effect. Leave it.

- [ ] **Step 1: Implement the hook** (no React test infra — verified by typecheck + preview):

```ts
import { useEffect, useState } from 'react'

/** Object URL for a Blob, revoked automatically on change and unmount.
 *  Null while no blob is set. */
export function useObjectUrl(blob: Blob | null | undefined): string | null {
  const [url, setUrl] = useState<string | null>(null)
  useEffect(() => {
    if (!blob) {
      setUrl(null)
      return
    }
    const u = URL.createObjectURL(blob)
    setUrl(u)
    return () => URL.revokeObjectURL(u)
  }, [blob])
  return url
}
```

- [ ] **Step 2: Adopt at the three sites**

`clip-tile.tsx` — delete the `useState`/`useEffect` block (lines 16–26) and use:

```ts
const url = useObjectUrl(firstImage)
```

`report-view.tsx` `CoverLogo` — delete its state/effect and use:

```ts
const url = useObjectUrl(logo)
```

`cover-form.tsx` `LogoDropWell` — delete its state/effect and use:

```ts
const previewUrl = useObjectUrl(logo)
```

Remove now-unused `useEffect`/`useState` imports where nothing else in the file needs them (clip-tile still uses neither after this; report-view and cover-form: check remaining usages before pruning).

- [ ] **Step 3: Verify**

Run: `bun run typecheck` and `bun run lint` — expected: pass.
Preview check (dev server `@luna-web/app`): scan a small folder — clip tiles show thumbnails; add a cover logo in the report workspace — the drop-well preview and the report-view header logo both render.

- [ ] **Step 4: Commit**

```bash
git add apps/web/src/lib/use-object-url.ts apps/web/src/features
git commit -m "refactor(web): useObjectUrl hook replaces three copy-pasted URL lifecycles"
```

### Task 6: `joinPath` to core + `fileExtensionOf` adoption

**Files:**
- Create: `packages/core/src/media/paths.ts`
- Test: `packages/core/src/media/paths.test.ts`
- Modify: `packages/core/src/index.ts:16-20`
- Modify: `packages/core/src/export/csv.ts:54,85`
- Modify: `apps/web/src/features/export/pdf-format.ts:12-14,34`
- Modify: `apps/web/src/features/export/pdf-document.tsx:4-12` (import line)
- Modify: `apps/web/src/features/report/clip-card.tsx:178`
- Modify: `apps/web/src/features/export/save.ts:23`

**Interfaces:**
- Produces: `joinPath(root: string, relative: string): string` exported from `@luna-web/core`.
- Consumes: existing `fileExtensionOf(fileName: string): string` from `@luna-web/core` (returns `'.ext'` lowercased, `''` when none).

- [ ] **Step 1: Write the failing test** — `packages/core/src/media/paths.test.ts`:

```ts
import { expect, test } from 'bun:test'
import { joinPath } from './paths'

test('joins root and relative with a slash', () => {
  expect(joinPath('CARD_A', 'A001/clip.mov')).toBe('CARD_A/A001/clip.mov')
})

test('passes the relative path through when root is unknown', () => {
  expect(joinPath('', 'clip.mov')).toBe('clip.mov')
})
```

- [ ] **Step 2: Run it** — `bun test packages/core/src/media/paths.test.ts` — expected: FAIL (module not found).
- [ ] **Step 3: Implement** — `packages/core/src/media/paths.ts`:

```ts
/** root/rel when the scanned root is known; rel alone otherwise (browsers
 *  cannot see the absolute disk path — the root folder name is the deepest
 *  honest prefix). */
export function joinPath(root: string, relative: string): string {
  return root ? `${root}/${relative}` : relative
}
```

And in `packages/core/src/index.ts`, next to the media exports (line 16–20), add:

```ts
export { joinPath } from './media/paths'
```

- [ ] **Step 4: Run it** — expected: PASS.
- [ ] **Step 5: Replace the inline copies**

`packages/core/src/export/csv.ts` — import `{ joinPath }` from `'../media/paths'`; lines 54 and 85 become:

```ts
joinPath(report.sourceRoot, clip.relativePath),
// …and in the otherFiles loop:
joinPath(report.sourceRoot, f.relativePath),
```

`apps/web/src/features/export/pdf-format.ts` — delete the local `joinPath` (lines 12–14) and add `joinPath` + `fileExtensionOf` to the `@luna-web/core` import. Line 34 becomes:

```ts
const ext = fileExtensionOf(clip.relativePath).slice(1).toUpperCase()
```

`apps/web/src/features/export/pdf-document.tsx` — `joinPath` was imported from `'./pdf-format'`; move it to the `@luna-web/core` import (keep `reelPath`, `breakablePath`, facts imports from `'./pdf-format'`).

`apps/web/src/features/report/clip-card.tsx` line 178:

```ts
const fullPath = joinPath(sourceRoot, clip.relativePath)
```

with `joinPath` added to the existing `@luna-web/core` import.

`apps/web/src/features/export/save.ts` line 23:

```ts
const ext = fileExtensionOf(fileName).slice(1)
```

with `import { fileExtensionOf } from '@luna-web/core'`.

- [ ] **Step 6: Verify** — `rg -n "sourceRoot \?|lastIndexOf\('\.'\)|split\('\.'\)\.pop" apps/web/src packages/core/src` — expected: no path/extension hand-rolls left (the `??` in save.ts is gone with the rewrite). Then `bun test` and `bun run typecheck` — pass.
- [ ] **Step 7: Commit**

```bash
git add packages/core/src apps/web/src
git commit -m "refactor(core): joinPath as the one path joiner; adopt fileExtensionOf in web"
```

### Task 7: `compareReelNames` in core

**Files:**
- Modify: `packages/core/src/reels/detect.ts` (add export; use at line 37)
- Test: `packages/core/src/reels/detect.test.ts` (create)
- Modify: `packages/core/src/report/model.ts:157`
- Modify: `packages/core/src/index.ts:41`

**Interfaces:**
- Produces: `compareReelNames(a: string, b: string): number` exported from `@luna-web/core`.

- [ ] **Step 1: Write the failing test** — `packages/core/src/reels/detect.test.ts`:

```ts
import { expect, test } from 'bun:test'
import { compareReelNames } from './detect'

test('orders reel names numerically, not lexically', () => {
  expect(['A010', 'A002', 'A001'].sort(compareReelNames)).toEqual(['A001', 'A002', 'A010'])
})
```

- [ ] **Step 2: Run it** — `bun test packages/core/src/reels/detect.test.ts` — expected: FAIL (`compareReelNames` not exported).
- [ ] **Step 3: Implement** — in `detect.ts`:

```ts
/** Reel-name ordering used everywhere reels are listed: numeric-aware so
 *  A002 < A010. */
export function compareReelNames(a: string, b: string): number {
  return a.localeCompare(b, undefined, { numeric: true })
}
```

Line 37 becomes `return reels.sort((a, b) => compareReelNames(a.name, b.name))`.
`report/model.ts:157` becomes `reels.sort((a, b) => compareReelNames(a.name, b.name))` with `compareReelNames` added to the file's existing `../reels/detect` import (or a new import line if reels are imported elsewhere — check the file head).
`index.ts:41` becomes `export { compareReelNames, detectReels, UNGROUPED_REEL } from './reels/detect'`.

- [ ] **Step 4: Run it** — expected: PASS. Then `bun test` (whole suite) — pass.
- [ ] **Step 5: Commit**

```bash
git add packages/core/src
git commit -m "refactor(core): one numeric-aware reel-name comparator"
```

### Task 8: Shared camera-field descriptors (screen + PDF)

**Files:**
- Create: `packages/core/src/metadata/fields.ts`
- Test: `packages/core/src/metadata/fields.test.ts`
- Modify: `packages/core/src/index.ts:23-24`
- Modify: `apps/web/src/features/report/clip-card.tsx:51-63`
- Modify: `apps/web/src/features/export/pdf-format.ts:64-76`

**Interfaces:**
- Consumes: `ClipMetadata` from `packages/core/src/metadata/model.ts:4` (all eight fields are `string | undefined`).
- Produces: `CAMERA_FIELDS: readonly { key: …; label: string }[]` and `type CameraFieldKey`, exported from `@luna-web/core`. Field set and order are canonical; each renderer keeps its own presentation (icons on screen, short-labels/mono in PDF).
- Deliberately narrow: only the camera column is shared. The "technical" rows differ structurally between screen and PDF (facts lines vs labeled rows) — forcing them into one descriptor would couple presentation, so they stay per-renderer.

- [ ] **Step 1: Write the failing test** — `packages/core/src/metadata/fields.test.ts`:

```ts
import { expect, test } from 'bun:test'
import { CAMERA_FIELDS } from './fields'

test('camera fields are unique and in canonical display order', () => {
  const keys = CAMERA_FIELDS.map((f) => f.key)
  expect(new Set(keys).size).toBe(keys.length)
  expect(keys).toEqual([
    'camera',
    'iso',
    'whiteBalance',
    'lens',
    'focalLength',
    'aperture',
    'shutter',
    'gamma',
  ])
})
```

- [ ] **Step 2: Run it** — `bun test packages/core/src/metadata/fields.test.ts` — expected: FAIL (module not found).
- [ ] **Step 3: Implement** — `packages/core/src/metadata/fields.ts`:

```ts
import type { ClipMetadata } from './model'

/** Camera-metadata fields in canonical display order — the single source for
 *  every renderer that lists camera fields (clip cards, PDF). Add a field
 *  here and it surfaces on screen and in the PDF at once (each renderer maps
 *  its own presentation by key). */
export const CAMERA_FIELDS = [
  { key: 'camera', label: 'Camera' },
  { key: 'iso', label: 'ISO' },
  { key: 'whiteBalance', label: 'White balance' },
  { key: 'lens', label: 'Lens' },
  { key: 'focalLength', label: 'Focal length' },
  { key: 'aperture', label: 'Aperture' },
  { key: 'shutter', label: 'Shutter' },
  { key: 'gamma', label: 'Gamma' },
] as const satisfies readonly { key: keyof ClipMetadata; label: string }[]

export type CameraFieldKey = (typeof CAMERA_FIELDS)[number]['key']
```

`index.ts`, next to the metadata exports (lines 23–24), add:

```ts
export type { CameraFieldKey } from './metadata/fields'
export { CAMERA_FIELDS } from './metadata/fields'
```

- [ ] **Step 4: Run it** — expected: PASS.
- [ ] **Step 5: Consume in `clip-card.tsx`** — replace `cameraRows` (lines 51–63) with:

```tsx
const CAMERA_ICONS: Record<CameraFieldKey, LucideIcon> = {
  camera: Camera,
  iso: Film,
  whiteBalance: Thermometer,
  lens: Focus,
  focalLength: Ruler,
  aperture: Aperture,
  shutter: Timer,
  gamma: SlidersHorizontal,
}

function cameraRows(clip: ReportClip<Blob>): Row[] {
  const m = clip.metadata
  return CAMERA_FIELDS.map((f) => ({ icon: CAMERA_ICONS[f.key], label: f.label, value: m[f.key] }))
}
```

with `CAMERA_FIELDS` and `type CameraFieldKey` added to the `@luna-web/core` import.

- [ ] **Step 6: Consume in `pdf-format.ts`** — replace `cameraFacts` (lines 64–76) with:

```ts
// PDF-only presentation: short labels where a bare value is ambiguous, mono
// for numeric-ish values. Field set and order come from CAMERA_FIELDS.
const PDF_CAMERA_PRESENTATION: Record<CameraFieldKey, { label?: string; mono?: boolean }> = {
  camera: {},
  iso: { label: 'ISO', mono: true },
  whiteBalance: { label: 'WB', mono: true },
  lens: {},
  focalLength: { mono: true },
  aperture: { mono: true },
  shutter: { label: 'Shutter', mono: true },
  gamma: {},
}

/** Camera extras, only what exists (§4.5 line 4). */
export function cameraFacts(metadata: ClipMetadata): Fact[] {
  const facts: Fact[] = []
  for (const f of CAMERA_FIELDS) {
    const value = metadata[f.key]
    if (!value) continue
    const p = PDF_CAMERA_PRESENTATION[f.key]
    facts.push({ ...(p.label !== undefined && { label: p.label }), value, ...(p.mono && { mono: true }) })
  }
  if (metadata.colorSpace) facts.push({ value: metadata.colorSpace })
  return facts
}
```

(`colorSpace` stays a PDF-only trailing fact — on screen it lives in the technical column; that asymmetry is pre-existing and intentional.) Add `CAMERA_FIELDS` / `type CameraFieldKey` to this file's `@luna-web/core` import.

- [ ] **Step 7: Verify** — `bun test`, `bun run typecheck` — pass. Preview: open a processed report — clip-card camera column unchanged; export a PDF — camera fact line reads identically to before (e.g. `ALEXA 35 · ISO 800 · WB 5600K · …`).
- [ ] **Step 8: Commit**

```bash
git add packages/core/src apps/web/src
git commit -m "refactor(core): CAMERA_FIELDS descriptor shared by clip cards and PDF"
```

---

## Phase 3 — Single-source constants

### Task 9: Thumbnail width + encode quality in core

**Files:**
- Modify: `packages/core/src/thumbs/model.ts` (append)
- Modify: `packages/core/src/index.ts:76`
- Modify: `apps/web/src/features/process/run-thumbnails.ts:19,101,163`
- Modify: `apps/web/src/features/process/preview-frame.ts:3-4,21,28`
- Modify: `apps/web/src/workers/thumbs.worker.ts:3,48`
- Modify: `apps/web/src/features/export/pdf-prepare.ts:75`

**Interfaces:**
- Produces: `THUMBNAIL_TARGET_WIDTH = 1280` and `THUMBNAIL_ENCODE_QUALITY = 0.85` exported from `@luna-web/core`.

- [ ] **Step 1: Add the constants** — append to `packages/core/src/thumbs/model.ts`:

```ts
/** Max width for every stored thumbnail frame — decoded captures and
 *  embedded RAW previews share it so reports never mix resolutions. */
export const THUMBNAIL_TARGET_WIDTH = 1280

/** Lossy quality for every thumbnail encode (WebP capture, JPEG re-encode
 *  for the PDF). */
export const THUMBNAIL_ENCODE_QUALITY = 0.85
```

`index.ts:76` becomes:

```ts
export {
  THUMBNAIL_ENCODE_QUALITY,
  THUMBNAIL_POSITIONS,
  THUMBNAIL_TARGET_WIDTH,
  thumbnailTimestamps,
} from './thumbs/model'
```

- [ ] **Step 2: Adopt at the four sites**

`run-thumbnails.ts` — delete `const THUMB_WIDTH = 1280` (line 19), add `THUMBNAIL_TARGET_WIDTH` to the `@luna-web/core` import, and replace both uses (lines 101, 163) with `THUMBNAIL_TARGET_WIDTH`.

`preview-frame.ts` — delete both local consts (lines 3–4), import `{ THUMBNAIL_ENCODE_QUALITY, THUMBNAIL_TARGET_WIDTH }` from `@luna-web/core`; line 21 uses `THUMBNAIL_TARGET_WIDTH / bitmap.width`, line 28 uses `quality: THUMBNAIL_ENCODE_QUALITY`.

`thumbs.worker.ts` — extend line 3's core import with `THUMBNAIL_ENCODE_QUALITY`; line 48 becomes `convertToBlob({ type: 'image/webp', quality: THUMBNAIL_ENCODE_QUALITY })`.

`pdf-prepare.ts` — import `{ THUMBNAIL_ENCODE_QUALITY }` from `@luna-web/core`; line 75 becomes `convertToBlob({ type: 'image/jpeg', quality: THUMBNAIL_ENCODE_QUALITY })`.

- [ ] **Step 3: Verify** — `rg -n "0\.85|= 1280" apps/web/src` — expected: no hits outside comments. `bun run typecheck`, `bun test` — pass. Preview: process a folder with thumbnails on — frames render as before.
- [ ] **Step 4: Commit**

```bash
git add packages/core/src apps/web/src
git commit -m "refactor(core): thumbnail width/quality constants single-sourced"
```

### Task 10: `DB_NAME`, `LUNA_URL`, and the ffmpeg-version guard test

**Files:**
- Modify: `apps/web/src/persistence/db.ts:21` (+ export)
- Modify: `apps/web/src/persistence/clear.ts:3,10`
- Create: `apps/web/src/lib/site.ts`
- Modify: `apps/web/src/features/export/pdf-document.tsx:24`
- Test: `apps/web/src/features/process/ffmpeg-core-version.test.ts` (create)

**Interfaces:**
- Produces: `DB_NAME = 'luna-web'` from `@/persistence/db`; `LUNA_URL = 'https://luna.ozer2.one'` from `@/lib/site`.

- [ ] **Step 1: `DB_NAME`** — in `db.ts` add above `getDb`:

```ts
export const DB_NAME = 'luna-web'
```

and use it: `openDB<LunaDb>(DB_NAME, 4, {`. In `clear.ts`, change the import to `import { closeDb, DB_NAME } from './db'` and line 10 to `await deleteDB(DB_NAME, {`.

- [ ] **Step 2: `LUNA_URL`** — create `apps/web/src/lib/site.ts`:

```ts
/** Canonical production origin. Also hardcoded (by necessity) in
 *  index.html's meta tags and public/llms.txt — update those alongside. */
export const LUNA_URL = 'https://luna.ozer2.one'
```

In `pdf-document.tsx`, delete `const LUNA_URL = 'https://luna.ozer2.one'` (line 24) and add `import { LUNA_URL } from '@/lib/site'`.

- [ ] **Step 3: Write the version-guard test (fails only on drift — write it, watch it pass, then prove it guards)** — `apps/web/src/features/process/ffmpeg-core-version.test.ts`:

```ts
import { join } from 'node:path'
import { expect, test } from 'bun:test'

// FFMPEG_CORE_VERSION in ffmpeg-engine.ts mirrors the pinned @ffmpeg/core
// devDependency because the package's exports map blocks importing its
// version (see the comment there). This test fails when one is bumped
// without the other.
test('FFMPEG_CORE_VERSION matches the @ffmpeg/core devDependency', async () => {
  const source = await Bun.file(join(import.meta.dir, 'ffmpeg-engine.ts')).text()
  const constant = source.match(/const FFMPEG_CORE_VERSION = '([^']+)'/)?.[1]
  const pkg = await Bun.file(join(import.meta.dir, '../../../package.json')).json()
  const declared = (pkg.devDependencies['@ffmpeg/core'] as string).replace(/^[~^]/, '')
  expect(constant).toBe(declared)
})
```

- [ ] **Step 4: Run it** — `bun test apps/web/src/features/process/ffmpeg-core-version.test.ts` — expected: PASS (`0.12.10` on both sides). Then temporarily edit the constant to `0.12.99`, re-run, confirm FAIL, and revert — that failure is the test's reason to exist.
- [ ] **Step 5: Verify** — `bun run typecheck`, `bun test` — pass. Preview: Settings → Clear local data still wipes and reloads (exercises `DB_NAME` in both files); export a PDF — footer link unchanged.
- [ ] **Step 6: Commit**

```bash
git add apps/web/src
git commit -m "chore(web): name DB/site constants; guard ffmpeg core version against drift"
```

---

## Phase 4 — UI conventions

### Task 11: `--warning` and `shadow-glow` design tokens

**Files:**
- Modify: `apps/web/src/index.css` (`@theme inline` block lines 9–51; `:root` block lines 55–88)
- Modify: `apps/web/src/features/activity/activity-screen.tsx:26`
- Modify: `apps/web/src/features/scan/recent-list.tsx:37`
- Modify: `apps/web/src/features/scan/scan-screen.tsx:166-173`
- Modify: `apps/web/src/features/scan/dropzone.tsx:71-76`

**Interfaces:**
- Produces: Tailwind utilities `text-warning` (et al.) and `shadow-glow` via `@theme inline` entries.

- [ ] **Step 1: Tokens** — in `index.css` `@theme inline` (after `--color-destructive`, line 29) add:

```css
    --color-warning: var(--warning);
    --shadow-glow: 0 0 24px oklch(0.72 0.14 245 / 0.25);
```

In `:root` (after `--destructive`, line 71) add (oklch equivalent of the Tailwind `amber-500` currently in use, so nothing shifts visually):

```css
    --warning: oklch(0.77 0.19 70);
```

- [ ] **Step 2: Adopt `text-warning`**

`activity-screen.tsx:26`: `warn: 'text-warning',`
`recent-list.tsx:37`: `<TriangleAlert className="size-4 shrink-0 text-warning" />`

- [ ] **Step 3: Adopt `shadow-glow`**

`scan-screen.tsx` — the Process button (lines 166–173) drops its `style` prop:

```tsx
<Button size="lg" className="shadow-glow" onClick={() => void startProcessing()}>
```

`dropzone.tsx` — the Pick-folder span (lines 71–76) drops its `style` prop and adds `shadow-glow` to the class string:

```tsx
<span className="bg-primary text-primary-foreground shadow-glow mt-1 inline-flex items-center gap-1.5 rounded-lg px-3 py-1.5 text-sm font-medium">
```

- [ ] **Step 4: Verify** — `rg -n "amber-500|boxShadow" apps/web/src` — expected: no hits (pdf-document uses react-pdf styles, not this pattern — confirm it wasn't touched). `bun run typecheck` + `bun run lint` — pass. Preview: dropzone pick-button and Process button still glow; a stale recent-folder row and a warn log entry still render amber.
- [ ] **Step 5: Commit**

```bash
git add apps/web/src
git commit -m "refactor(web): warning color and primary glow become theme tokens"
```

### Task 12: Empty-state markup goes through `ui/empty`

**Files:**
- Modify: `apps/web/src/features/reports/report-library-screen.tsx:37-40`
- Modify: `apps/web/src/features/activity/activity-screen.tsx:107-112`
- Modify: `apps/web/src/features/reports/saved-report-screen.tsx:29-41`

**Interfaces:**
- Consumes: `Empty`, `EmptyHeader`, `EmptyTitle`, `EmptyDescription`, `EmptyContent` from `@/components/ui/empty` (already used by scan-screen's error state — that's the house pattern).

- [ ] **Step 1: Report library** — replace the empty `<p>` (lines 38–40) with:

```tsx
<Empty className="py-12">
  <EmptyHeader>
    <EmptyTitle>No saved reports yet</EmptyTitle>
    <EmptyDescription>Finish a run and use "Save report" to keep it here.</EmptyDescription>
  </EmptyHeader>
</Empty>
```

- [ ] **Step 2: Activity** — replace the empty `<p>` (lines 108–112) with:

```tsx
<Empty className="py-12">
  <EmptyHeader>
    <EmptyTitle>
      {snapshot.entries.length === 0 ? 'Nothing logged yet' : 'No entries at this level'}
    </EmptyTitle>
    {snapshot.entries.length === 0 && (
      <EmptyDescription>Scan a folder and activity will show up here.</EmptyDescription>
    )}
  </EmptyHeader>
</Empty>
```

- [ ] **Step 3: Saved report (missing)** — replace the hand-rolled `<div>` (lines 30–40) with:

```tsx
<Empty className="py-24">
  <EmptyHeader>
    <EmptyTitle className="text-2xl">Report not found</EmptyTitle>
    <EmptyDescription>
      This saved report doesn't exist anymore — it may have been deleted.
    </EmptyDescription>
  </EmptyHeader>
  <EmptyContent>
    <Link to="/reports/" className={cn(buttonVariants({ variant: 'outline' }))}>
      Back to Reports
    </Link>
  </EmptyContent>
</Empty>
```

Add the `@/components/ui/empty` imports each file needs.

- [ ] **Step 4: Verify** — `bun run typecheck` + `bun run lint` — pass. Preview: `/reports/` with no saved reports, `/activity` with an empty log, and `/reports/does-not-exist/` — all three render the Empty layout, copy unchanged.
- [ ] **Step 5: Commit**

```bash
git add apps/web/src/features
git commit -m "refactor(web): empty states use the ui/empty primitives"
```

### Task 13: Adopt `ui/table` in the report view

**Files:**
- Modify: `apps/web/src/features/report/report-view.tsx:105-121`

**Interfaces:**
- Consumes: `Table`, `TableBody`, `TableRow`, `TableCell` from `@/components/ui/table` (currently unused anywhere — this is its first consumer; the shadcn primitives are the house style, hand-rolled `<table>` markup is not).
- Visual parity notes: old `<tbody className="divide-y">` ≈ `TableRow`'s `border-b` + `TableBody`'s last-child border removal; old `py-2` matches `TableCell`'s `p-2` vertical padding, so only `px-4` needs overriding; `truncate` already implies `whitespace-nowrap`. `TableRow` adds a `hover:bg-muted/50` highlight the old markup lacked — that's the shadcn default and is welcome, keep it.

- [ ] **Step 1: Replace the raw table** — in `report-view.tsx`, replace lines 105–121:

```tsx
<Card className="overflow-hidden py-0">
  <Table>
    <TableBody>
      {reel.otherFiles.map((f) => (
        <TableRow key={f.relativePath}>
          <TableCell className="px-4 font-medium">{f.fileName}</TableCell>
          <TableCell className="text-muted-foreground w-full truncate px-4">
            {f.relativePath}
          </TableCell>
          <TableCell className="text-muted-foreground px-4 text-right font-mono tabular-nums">
            {formatBytes(f.sizeBytes)}
          </TableCell>
        </TableRow>
      ))}
    </TableBody>
  </Table>
</Card>
```

Add `import { Table, TableBody, TableCell, TableRow } from '@/components/ui/table'`.

- [ ] **Step 2: Verify** — `bun run typecheck` + `bun run lint` — pass. Preview: a report whose reel has other files (sidecars) — the Other-files card renders with the same columns/spacing as before, plus a row hover highlight.
- [ ] **Step 3: Commit**

```bash
git add apps/web/src/features/report/report-view.tsx
git commit -m "refactor(web): Other-files table uses the ui/table primitives"
```

### Task 14: Adopt `Badge` for the scan-screen "Ready" pill

**Files:**
- Modify: `apps/web/src/features/scan/scan-screen.tsx:101-104`

**Interfaces:**
- Consumes: `Badge` from `@/components/ui/badge`. No variant matches the soft `bg-primary/10 text-primary` look, and `ui/badge.tsx` is vendored/Biome-excluded so we don't add one — a `className` override on `variant="secondary"` is the shadcn-sanctioned way. `Badge` is `h-5` where the old pill's `py-1` made it slightly taller; accept the standard badge height.

- [ ] **Step 1: Replace the hand-rolled pill** — in `scan-screen.tsx`, replace lines 101–104:

```tsx
<span className="bg-primary/10 text-primary inline-flex shrink-0 items-center gap-1.5 rounded-full px-2.5 py-1 text-xs font-medium">
  <span className="bg-primary size-1.5 rounded-full" />
  Ready
</span>
```

with:

```tsx
<Badge variant="secondary" className="bg-primary/10 text-primary gap-1.5 rounded-full px-2.5">
  <span className="bg-primary size-1.5 rounded-full" />
  Ready
</Badge>
```

Add `import { Badge } from '@/components/ui/badge'` (scan-screen has no Badge import today).

- [ ] **Step 2: Verify** — `bun run typecheck` + `bun run lint` — pass. Preview: scan a folder to the summary card — the Ready pill renders (cyan tint, dot, rounded-full) at standard badge height.
- [ ] **Step 3: Commit**

```bash
git add apps/web/src/features/scan/scan-screen.tsx
git commit -m "refactor(web): scan Ready pill uses the Badge component"
```
