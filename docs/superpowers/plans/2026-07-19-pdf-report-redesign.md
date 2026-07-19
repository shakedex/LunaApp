# Luna Web — PDF Report Redesign Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Rewrite the PDF export to the approved dark, Hedge-style, data-first design — Geist/Geist Mono typography, zebra clip bands, full paths everywhere — per `docs/superpowers/specs/2026-07-19-pdf-report-design.md`.

**Architecture:** Four self-contained steps in `apps/web/src/features/export/`: (1) a font module registering Geist + Geist Mono static faces with react-pdf and disabling hyphenation; (2) pure formatting helpers (`pdf-format.ts`) plus `sourceRoot` plumbing through `pdf-prepare.ts`/`pdf-exporter.ts`; (3) the document skeleton (dark page, header, totals, path bands, reel sections, fixed footer); (4) the clip bands (left fact column + full path, right thumbnail strip, zebra alternation). The `ReportModel` already provides everything (post-`fbcd2dd`: `otherFileCount`/`otherFileSizeBytes`, honest totals, no `RawNotice`).

**Tech Stack:** `@react-pdf/renderer` 4.x (installed), `@fontsource/geist` + `@fontsource/geist-mono` static font packages (new, via `bun add`), Vite asset imports, Bun workspaces.

**Spec:** `docs/superpowers/specs/2026-07-19-pdf-report-design.md` — the sections below cite it as §N.

## Global Constraints

- **Never hand-write a dependency version.** New deps only via `bun add` (latest).
- **Every file is a file** (spec §2): no segregated sections, no warning icons, no placeholder walls. Missing metadata/thumbnails → the element is omitted, never faked.
- **No user-typed fields added** (spec §2). Paths derive from `scanStore.sourceName` + `relativePath` only.
- **File names and paths wrap, never truncate** (spec §2).
- **Palette exactly** (spec §3): page `#151519`, zebra band `#1D1D22`, path band `#26262C`, text `#E7E7EA`, muted `#9EA0A8`, accent `#9AD6F2` (reel names only). No borders, no rounded corners, no shadows.
- **Untouched** (spec §6): `packages/core`, CSV exporter, cover form/store, app screens, workers.
- **Testing convention** (spec §7): no UI test runner — the per-task cycle is `bun run lint && bun run typecheck && bun test && bun run build` from the repo root (all must exit 0; `bun test` covers core only and must stay at 141 passing), plus the visual QA steps stated in each task.
- **Commits:** conventional style, `feat(web): …`, one commit per task.

---

## File Structure

```
apps/web/src/features/export/
  pdf-fonts.ts       NEW: Font.register for Geist 400/500/600 + Geist Mono 400/500,
                          hyphenation disabled; exports GEIST / GEIST_MONO family names
  pdf-format.ts      NEW: pure helpers — path joining/breaking, reel path resolution,
                          fact-line builders (file/video/camera facts)
  pdf-prepare.ts     MODIFY: PdfReport gains sourceRoot; prepareReportForPdf(report, sourceRoot)
  pdf-exporter.ts    MODIFY: passes scanStore.state.sourceName ?? '' to prepare
  pdf-document.tsx   REWRITE: the document per spec §3–§4
```

---

## Task 1: Font module (Geist + Geist Mono registered with react-pdf)

**Files:**
- Modify: `apps/web/package.json` + lockfile (via `bun add`)
- Create: `apps/web/src/features/export/pdf-fonts.ts`

**Interfaces:**
- Produces: `GEIST: string` and `GEIST_MONO: string` (react-pdf `fontFamily` values), registered as a side effect of importing the module. Weights available: Geist 400/500/600, Geist Mono 400/500.

- [ ] **Step 1: Add the static font packages (latest, via bun)**

```bash
cd apps/web && bun add @fontsource/geist @fontsource/geist-mono
```

(These are the STATIC per-weight packages. The already-installed `@fontsource-variable/*` packages are for the app UI and useless here — react-pdf cannot consume variable fonts.)

- [ ] **Step 2: Confirm the exact file names**

Run: `ls node_modules/@fontsource/geist/files/ | grep 'latin-[456]00-normal'` (from `apps/web`)
Expected: `geist-latin-400-normal.woff2` (and 500/600 siblings). Repeat for `@fontsource/geist-mono` → `geist-mono-latin-400-normal.woff2` etc. If names differ, adapt the imports in Step 3 to the actual names — change nothing else.

- [ ] **Step 3: Create `apps/web/src/features/export/pdf-fonts.ts`**

```ts
import { Font } from '@react-pdf/renderer'
import geistMono400 from '@fontsource/geist-mono/files/geist-mono-latin-400-normal.woff2'
import geistMono500 from '@fontsource/geist-mono/files/geist-mono-latin-500-normal.woff2'
import geist400 from '@fontsource/geist/files/geist-latin-400-normal.woff2'
import geist500 from '@fontsource/geist/files/geist-latin-500-normal.woff2'
import geist600 from '@fontsource/geist/files/geist-latin-600-normal.woff2'

export const GEIST = 'Geist'
export const GEIST_MONO = 'Geist Mono'

Font.register({
  family: GEIST,
  fonts: [
    { src: geist400, fontWeight: 400 },
    { src: geist500, fontWeight: 500 },
    { src: geist600, fontWeight: 600 },
  ],
})

Font.register({
  family: GEIST_MONO,
  fonts: [
    { src: geistMono400, fontWeight: 400 },
    { src: geistMono500, fontWeight: 500 },
  ],
})

// Spec §3: no mid-word hyphenation, ever. Paths wrap at '/' via zero-width
// spaces inserted in pdf-format.ts, not via hyphenation.
Font.registerHyphenationCallback((word) => [word])
```

Notes for the implementer:
- Vite turns `.woff2` imports into URL strings (`vite/client` types already declare them); react-pdf 4.x fetches and parses WOFF2 (`@react-pdf/font` supports TTF/WOFF/WOFF2).
- **Fallback (only if runtime QA in Task 3 shows a font-parse failure):** fontsource also ships `.ttf` under the same `files/` directory naming; switch the five imports to `.ttf` and report which path was taken.

- [ ] **Step 4: Gates**

Run from repo root: `bun run lint && bun run typecheck && bun test && bun run build`
Expected: all exit 0; 141 core tests pass. (The module is not imported yet — build proves the imports resolve.) If lint flags the unused module, that is acceptable only if Biome is configured to ignore it; otherwise add the import to `pdf-document.tsx` in this task's commit is WRONG — instead suppress nothing and verify Biome does not flag unimported files (it does not; it lints file contents).

- [ ] **Step 5: Commit**

```bash
git add apps/web/package.json bun.lock apps/web/src/features/export/pdf-fonts.ts
git commit -m "feat(web): register Geist static faces for the PDF exporter"
```

---

## Task 2: Formatting helpers + sourceRoot plumbing

**Files:**
- Create: `apps/web/src/features/export/pdf-format.ts`
- Modify: `apps/web/src/features/export/pdf-prepare.ts`
- Modify: `apps/web/src/features/export/pdf-exporter.ts`

**Interfaces:**
- Consumes: `PdfClip`, `PdfReel` from `./pdf-prepare`; `ClipMetadata` from `@luna-web/core`; `formatBytes`, `formatDuration` from `@/lib/format`.
- Produces (used verbatim by Tasks 3–4):

```ts
// pdf-format.ts
export interface Fact { label?: string; value: string; mono?: boolean }
export function joinPath(root: string, relative: string): string
export function breakablePath(path: string): string
export function reelPath(root: string, reel: PdfReel): string
export function fileFacts(clip: PdfClip): Fact[]
export function videoFacts(metadata: ClipMetadata): Fact[]
export function cameraFacts(metadata: ClipMetadata): Fact[]
// pdf-prepare.ts (changed surface only)
export interface PdfReport { /* existing fields */; sourceRoot: string }
export function prepareReportForPdf(report: ReportModel<Blob>, sourceRoot: string): Promise<PdfReport>
```

- [ ] **Step 1: Create `apps/web/src/features/export/pdf-format.ts`**

```ts
import type { ClipMetadata } from '@luna-web/core'
import { formatBytes, formatDuration } from '@/lib/format'
import type { PdfClip, PdfReel } from './pdf-prepare'

/** One ` · `-separated segment of a clip fact line (spec §4.5). */
export interface Fact {
  label?: string
  value: string
  mono?: boolean
}

export function joinPath(root: string, relative: string): string {
  return root ? `${root}/${relative}` : relative
}

/** Zero-width space after each '/' so paths wrap at segment boundaries (§3). */
export function breakablePath(path: string): string {
  return path.replaceAll('/', '/\u200B')
}

/**
 * Spec §4.4: a reel grouped by its top-level folder shows root/folder; a reel
 * grouped by embedded reelName (clips scattered across folders) shows root only.
 */
export function reelPath(root: string, reel: PdfReel): string {
  const isFolderReel =
    reel.clips.length > 0 && reel.clips.every((c) => c.relativePath.startsWith(`${reel.name}/`))
  return isFolderReel ? joinPath(root, reel.name) : root
}

/** `MOV · 1443 frames (57.7s) · 7.09 GB` — size always exists (§4.5 line 1). */
export function fileFacts(clip: PdfClip): Fact[] {
  const facts: Fact[] = []
  const ext = clip.relativePath.slice(clip.relativePath.lastIndexOf('.') + 1).toUpperCase()
  if (ext) facts.push({ value: ext })
  const { durationSeconds, frameRate } = clip.metadata
  if (durationSeconds !== undefined && frameRate !== undefined) {
    facts.push({
      value: `${Math.round(durationSeconds * frameRate)} frames (${formatDuration(durationSeconds)})`,
      mono: true,
    })
  } else if (durationSeconds !== undefined) {
    facts.push({ value: formatDuration(durationSeconds), mono: true })
  }
  facts.push({ value: formatBytes(clip.sizeBytes), mono: true })
  return facts
}

/** `3840×2160 (1.78:1) · Apple ProRes 4444 · 25 fps` (§4.5 line 2). */
export function videoFacts(metadata: ClipMetadata): Fact[] {
  const facts: Fact[] = []
  if (metadata.width && metadata.height) {
    const aspect = `${(metadata.width / metadata.height).toFixed(2).replace(/\.?0+$/, '')}:1`
    facts.push({ value: `${metadata.width}×${metadata.height} (${aspect})`, mono: true })
  }
  if (metadata.codec) facts.push({ value: metadata.codec })
  if (metadata.frameRate !== undefined) {
    facts.push({ value: `${Number(metadata.frameRate.toFixed(3))} fps`, mono: true })
  }
  return facts
}

/** Camera extras, only what exists (§4.5 line 4). */
export function cameraFacts(metadata: ClipMetadata): Fact[] {
  const facts: Fact[] = []
  if (metadata.camera) facts.push({ value: metadata.camera })
  if (metadata.iso) facts.push({ label: 'ISO', value: metadata.iso, mono: true })
  if (metadata.whiteBalance) facts.push({ label: 'WB', value: metadata.whiteBalance, mono: true })
  if (metadata.lens) facts.push({ value: metadata.lens })
  if (metadata.focalLength) facts.push({ value: metadata.focalLength, mono: true })
  if (metadata.aperture) facts.push({ value: metadata.aperture, mono: true })
  if (metadata.shutter) facts.push({ label: 'Shutter', value: metadata.shutter, mono: true })
  if (metadata.gamma) facts.push({ value: metadata.gamma })
  if (metadata.colorSpace) facts.push({ value: metadata.colorSpace })
  return facts
}
```

Implementer note: check `ClipMetadata` in `packages/core/src/metadata/model.ts` before coding — if any field above is typed `number` rather than `string` (e.g. `iso`), wrap it in `String(...)`. Field names must match the real interface; do not invent fields.

- [ ] **Step 2: Add `sourceRoot` to `pdf-prepare.ts`**

In the `PdfReport` interface add one field (after `stats`):

```ts
  /** The scanned root folder's name — '' when unknown. Browsers cannot see the
   *  absolute disk path; this is the deepest prefix we can honestly print (§2). */
  sourceRoot: string
```

Change the signature and return of `prepareReportForPdf`:

```ts
export async function prepareReportForPdf(
  report: ReportModel<Blob>,
  sourceRoot: string,
): Promise<PdfReport> {
```

and in the returned object literal add `sourceRoot,`.

- [ ] **Step 3: Pass the root from `pdf-exporter.ts`**

```ts
import { scanStore } from '@/features/scan/store'
```

and change the prepare call inside `generate`:

```ts
    const prepared = await prepareReportForPdf(report, scanStore.state.sourceName ?? '')
```

- [ ] **Step 4: Gates**

Run from repo root: `bun run lint && bun run typecheck && bun test && bun run build`
Expected: all exit 0. (`pdf-format.ts` is not imported yet; `pdf-document.tsx` still compiles because `PdfReport` only gained a field.)

- [ ] **Step 5: Commit**

```bash
git add apps/web/src/features/export
git commit -m "feat(web): pdf formatting helpers and sourceRoot plumbing"
```

---

## Task 3: Document skeleton — dark page, header, totals, path bands, reel sections, footer

**Files:**
- Rewrite: `apps/web/src/features/export/pdf-document.tsx`

**Interfaces:**
- Consumes: `GEIST`/`GEIST_MONO` (Task 1), `joinPath`/`breakablePath`/`reelPath` (Task 2), `PdfReport`/`PdfReel`/`PdfClip` from `./pdf-prepare`.
- Produces: `ReportDocument({ report }: { report: PdfReport })` — same export name/props as today (`pdf-exporter.ts` keeps working). A temporary minimal `ClipBand` renders name + size + path; Task 4 replaces ONLY that component and its styles.

- [ ] **Step 1: Rewrite `pdf-document.tsx`**

Replace the entire file:

```tsx
import { Document, Image, Page, StyleSheet, Text, View } from '@react-pdf/renderer'
import { formatBytes, formatDuration } from '@/lib/format'
import { GEIST, GEIST_MONO } from './pdf-fonts'
import { breakablePath, joinPath, reelPath } from './pdf-format'
import type { PdfClip, PdfReel, PdfReport } from './pdf-prepare'

// Spec §3 palette — the app's Cinema Dark tokens, hex-resolved.
const C = {
  page: '#151519',
  band: '#1D1D22',
  pathBand: '#26262C',
  text: '#E7E7EA',
  muted: '#9EA0A8',
  accent: '#9AD6F2',
} as const

const styles = StyleSheet.create({
  page: {
    backgroundColor: C.page,
    color: C.text,
    fontFamily: GEIST,
    fontSize: 8,
    paddingTop: 28,
    paddingHorizontal: 28,
    paddingBottom: 44,
  },
  mono: { fontFamily: GEIST_MONO },

  headerRow: {
    flexDirection: 'row',
    justifyContent: 'space-between',
    alignItems: 'flex-start',
    marginBottom: 10,
  },
  eyebrow: { color: C.muted, fontSize: 7, letterSpacing: 2, marginBottom: 4 },
  title: { fontSize: 20, fontWeight: 600 },
  coverMeta: { color: C.muted, marginTop: 3, fontSize: 8 },
  coverMetaValue: { color: C.text },
  logo: { maxHeight: 32, maxWidth: 140, objectFit: 'contain' },

  totals: { flexDirection: 'row', flexWrap: 'wrap', marginBottom: 10 },
  totalValue: { fontFamily: GEIST_MONO, fontWeight: 500 },
  totalLabel: { color: C.muted },

  pathBand: {
    backgroundColor: C.pathBand,
    color: C.muted,
    fontFamily: GEIST_MONO,
    fontSize: 7,
    paddingVertical: 4,
    paddingHorizontal: 10,
    marginBottom: 8,
  },

  reelSection: { marginBottom: 12 },
  reelHeader: {
    flexDirection: 'row',
    justifyContent: 'space-between',
    alignItems: 'baseline',
    marginBottom: 4,
  },
  reelName: { color: C.accent, fontSize: 12, fontWeight: 600 },
  reelMeta: { color: C.muted, fontFamily: GEIST_MONO, fontSize: 7 },

  band: { flexDirection: 'row', paddingVertical: 8, paddingHorizontal: 10 },
  bandAlt: { backgroundColor: C.band },
  clipName: { fontSize: 10, fontWeight: 600, marginBottom: 3 },
  clipPath: { color: C.muted, fontFamily: GEIST_MONO, fontSize: 7, marginTop: 3 },

  footerLeft: {
    position: 'absolute',
    bottom: 20,
    left: 28,
    color: C.muted,
    fontSize: 7,
  },
  footerRight: {
    position: 'absolute',
    bottom: 20,
    right: 28,
    color: C.muted,
    fontFamily: GEIST_MONO,
    fontSize: 7,
  },
})

const SEP = '  ·  '

/** Temporary minimal band — Task 4 replaces this component and nothing else. */
function ClipBand({ clip, index, root }: { clip: PdfClip; index: number; root: string }) {
  return (
    <View style={index % 2 === 0 ? [styles.band, styles.bandAlt] : styles.band} wrap={false}>
      <View style={{ width: '100%' }}>
        <Text style={styles.clipName}>{clip.fileName}</Text>
        <Text style={styles.mono}>{formatBytes(clip.sizeBytes)}</Text>
        <Text style={styles.clipPath}>{breakablePath(joinPath(root, clip.relativePath))}</Text>
      </View>
    </View>
  )
}

function ReelSection({ reel, root }: { reel: PdfReel; root: string }) {
  const meta = [
    `${reel.stats.clipCount} clips`,
    reel.stats.otherFileCount > 0
      ? `${reel.stats.otherFileCount} other files (${formatBytes(reel.stats.otherFileSizeBytes)})`
      : null,
    formatDuration(reel.stats.totalDurationSeconds),
    formatBytes(reel.stats.totalSizeBytes),
  ]
    .filter(Boolean)
    .join(SEP)
  return (
    <View style={styles.reelSection}>
      <View style={styles.reelHeader}>
        <Text style={styles.reelName}>{reel.name}</Text>
        <Text style={styles.reelMeta}>{meta}</Text>
      </View>
      <Text style={styles.pathBand}>{breakablePath(reelPath(root, reel))}</Text>
      {reel.clips.map((clip, i) => (
        <ClipBand key={clip.relativePath} clip={clip} index={i} root={root} />
      ))}
    </View>
  )
}

export function ReportDocument({ report }: { report: PdfReport }) {
  const { cover, stats, sourceRoot } = report
  const generated = new Date().toISOString().slice(0, 10)
  const title = cover.projectTitle || 'Camera report'

  const crewLine = [
    cover.dit ? ['DIT', cover.dit] : null,
    cover.director ? ['Director', cover.director] : null,
    cover.dp ? ['DP', cover.dp] : null,
  ].filter((x): x is [string, string] => x !== null)

  const totals: [string, string][] = [
    [String(stats.cardCount), stats.cardCount === 1 ? 'card' : 'cards'],
    [String(stats.clipCount), stats.clipCount === 1 ? 'clip' : 'clips'],
    ...(stats.otherFileCount > 0
      ? [
          [
            `${stats.otherFileCount} (${formatBytes(stats.otherFileSizeBytes)})`,
            'other files',
          ] as [string, string],
        ]
      : []),
    [formatDuration(stats.totalDurationSeconds), 'duration'],
    [formatBytes(stats.totalSizeBytes), 'total size'],
  ]

  return (
    <Document title={title} creator="Luna">
      <Page size="A4" style={styles.page}>
        <View style={styles.headerRow}>
          <View style={{ maxWidth: '75%' }}>
            <Text style={styles.eyebrow}>CAMERA REPORT</Text>
            <Text style={styles.title}>{title}</Text>
            {(cover.productionCompany || cover.date) && (
              <Text style={styles.coverMeta}>
                {[cover.productionCompany, cover.date].filter(Boolean).join(SEP)}
              </Text>
            )}
            {crewLine.length > 0 && (
              <Text style={styles.coverMeta}>
                {crewLine.map(([role, name], i) => (
                  <Text key={role}>
                    {i > 0 ? SEP : ''}
                    {role} <Text style={styles.coverMetaValue}>{name}</Text>
                  </Text>
                ))}
              </Text>
            )}
          </View>
          {cover.logoDataUrl ? <Image src={cover.logoDataUrl} style={styles.logo} /> : null}
        </View>

        <View style={styles.totals}>
          {totals.map(([value, label], i) => (
            <Text key={label}>
              {i > 0 ? SEP : ''}
              <Text style={styles.totalValue}>{value}</Text>
              <Text style={styles.totalLabel}> {label}</Text>
            </Text>
          ))}
        </View>

        {sourceRoot ? (
          <Text style={styles.pathBand}>{breakablePath(sourceRoot)}</Text>
        ) : null}

        {report.reels.map((reel) => (
          <ReelSection key={reel.name} reel={reel} root={sourceRoot} />
        ))}

        <Text style={styles.footerLeft} fixed>
          {title}
        </Text>
        <Text
          style={styles.footerRight}
          fixed
          render={({ pageNumber, totalPages }) => `Luna${SEP}${generated}${SEP}${pageNumber}/${totalPages}`}
        />
      </Page>
    </Document>
  )
}
```

Implementer notes:
- Consolidate the two `@react-pdf/renderer` imports into one line (shown split above only for diff clarity); Biome will flag duplicates.
- `fontWeight: 600` etc. select the faces registered in Task 1 — do NOT use `Helvetica-Bold`-style family suffixes anywhere.
- `wrap={false}` on clip bands is what keeps a band from splitting across pages.

- [ ] **Step 2: Gates**

Run from repo root: `bun run lint && bun run typecheck && bun test && bun run build`
Expected: all exit 0; 141 tests.

- [ ] **Step 3: Visual smoke render (agent-runnable, no browser needed)**

react-pdf renders in Bun/Node, and Bun's file loader turns the `.woff2` imports into paths `Font.register` can read. Write a THROWAWAY script in the session scratchpad (NOT in the repo) that imports `ReportDocument` with a synthetic `PdfReport` (two reels — one folder-grouped with 3 clips of full metadata + 1 metadata-less clip, one reelName-grouped; `sourceRoot: 'CARD_A'`; no logo; thumbnails `dataUrl: null`) and calls `renderToFile` from `@react-pdf/renderer` (run it with `bun --conditions node` from `apps/web` so React resolves; if module resolution fights back, fall back to rendering via the dev server in Step 4 and note it). Convert the output PDF to PNG (pdfjs-dist + @napi-rs/canvas, or open it) and CHECK: dark full-bleed page, Geist actually rendering (not Helvetica fallback), zebra alternation visible, footer on the page, no mid-word hyphens in the path band.
Expected: a legible dark page matching spec §3–§4. Fix before committing; if WOFF2 parsing fails here, apply Task 1's TTF fallback and re-run.

- [ ] **Step 4: Commit**

```bash
git add apps/web/src/features/export/pdf-document.tsx
git commit -m "feat(web): dark data-first pdf document skeleton"
```

---

## Task 4: Clip bands — fact lines, full path, thumbnail strip

**Files:**
- Modify: `apps/web/src/features/export/pdf-document.tsx` (replace `ClipBand` + add its styles ONLY)

**Interfaces:**
- Consumes: `Fact`, `fileFacts`, `videoFacts`, `cameraFacts`, `breakablePath`, `joinPath` from `./pdf-format`; `PdfClip` from `./pdf-prepare`; existing `styles`/`C` from Task 3.
- Produces: the final `ClipBand({ clip, index, root })` per spec §4.5.

- [ ] **Step 1: Add band styles**

Add to the `StyleSheet.create` block (keep every existing style):

```ts
  bandLeft: { width: '40%', paddingRight: 10 },
  bandRight: {
    width: '60%',
    flexDirection: 'row',
    justifyContent: 'flex-end',
    gap: 4,
  },
  factLine: { marginBottom: 2, color: C.text },
  factLabel: { color: C.muted },
  timecode: { fontFamily: GEIST_MONO, marginBottom: 2 },
  thumb: { width: 104, height: 58, objectFit: 'contain' },
```

- [ ] **Step 2: Replace the temporary `ClipBand` with the final one**

```tsx
function FactLine({ facts }: { facts: Fact[] }) {
  if (facts.length === 0) return null
  return (
    <Text style={styles.factLine}>
      {facts.map((f, i) => (
        <Text key={`${f.label ?? ''}${f.value}`}>
          {i > 0 ? SEP : ''}
          {f.label ? <Text style={styles.factLabel}>{f.label} </Text> : null}
          <Text style={f.mono ? styles.mono : undefined}>{f.value}</Text>
        </Text>
      ))}
    </Text>
  )
}

function ClipBand({ clip, index, root }: { clip: PdfClip; index: number; root: string }) {
  const frames = clip.frames.filter((f) => f.dataUrl !== null)
  return (
    <View style={index % 2 === 0 ? [styles.band, styles.bandAlt] : styles.band} wrap={false}>
      <View style={frames.length > 0 ? styles.bandLeft : { width: '100%' }}>
        <Text style={styles.clipName}>{clip.fileName}</Text>
        <FactLine facts={fileFacts(clip)} />
        <FactLine facts={videoFacts(clip.metadata)} />
        {clip.metadata.startTimecode ? (
          <Text style={styles.timecode}>
            <Text style={styles.factLabel}>TC </Text>
            {clip.metadata.startTimecode}
          </Text>
        ) : null}
        <FactLine facts={cameraFacts(clip.metadata)} />
        <Text style={styles.clipPath}>{breakablePath(joinPath(root, clip.relativePath))}</Text>
      </View>
      {frames.length > 0 ? (
        <View style={styles.bandRight}>
          {frames.map((frame, i) => (
            <Image key={String(i)} src={frame.dataUrl as string} style={styles.thumb} />
          ))}
        </View>
      ) : null}
    </View>
  )
}
```

Update the imports at the top of the file to pull `cameraFacts`, `fileFacts`, `videoFacts`, and `type Fact` from `./pdf-format`. Spec conformance checklist for this component: failed/missing frames render NOTHING (no placeholder boxes — §4.5); a clip with zero frames spans the full width; metadata-less clips show name + file facts + path only (§5); nothing truncates.

- [ ] **Step 3: Gates**

Run from repo root: `bun run lint && bun run typecheck && bun test && bun run build`
Expected: all exit 0; 141 tests.

- [ ] **Step 4: Visual verification (same harness as Task 3 Step 3)**

Re-run the scratchpad render with the synthetic report, now also including one clip with 3 fake thumbnail data URLs (tiny 16×9 JPEG data URI is fine) and one clip whose `frames` are all `dataUrl: null`. CHECK against spec §4.5: 40/60 split; thumbs right-aligned and ~104pt; no placeholders for the null-frame clip (full-width left column); fact lines omit missing values; the path line wraps at segment boundaries; zebra unbroken.

- [ ] **Step 5: Commit**

```bash
git add apps/web/src/features/export/pdf-document.tsx
git commit -m "feat(web): pdf clip bands with fact lines and thumbnail strip"
```

---

## Definition of done

- All four gates green from repo root; 141 core tests untouched and passing.
- **Maintainer QA (Shaked, Chromium, real corpus — non-blocking, after Task 4):**
  1. Export a multi-reel card → dark pages, Geist/Geist Mono everywhere (no Helvetica), zebra bands, reel headers in cyan, per-reel and per-clip paths prefixed with the scanned folder name.
  2. MXF with embedded reel names → that reel's path band shows the root only.
  3. `.braw`/`.ari`/metadata-less clips → normal bands, no placeholders, no icons.
  4. A card with non-media files → totals line and reel headers show "other files" counts/sizes; total size matches the delivered bytes.
  5. 100+ clip card → export completes, bands never split across pages, footer paginates.
  6. Side-by-side with a Hedge sample from `E:\Coding\wrapport\docs\hedge_pdf` → comparable density and readability.

## Self-review notes

- **Spec coverage:** §2 hard rules → Tasks 3–4 (no RAW anything, omission over placeholders, wrap-never-truncate via `breakablePath` + hyphenation callback, no new user fields); §3 palette/typography → Task 1 + Task 3 `C`/`styles`; §4.1–4.4+4.6 → Task 3; §4.5 → Task 4; §5 edge cases → Task 4 checklist; §6 scope → file map matches (spec's "vendored TTFs" is implemented as fontsource static packages via `bun add` — same fonts, better fit for the project's dependency rules, TTF files still available as the documented fallback); §7 testing → gates + agent-run render harness + maintainer QA.
- **Type consistency:** `Fact`/`fileFacts`/`videoFacts`/`cameraFacts`/`breakablePath`/`joinPath`/`reelPath` identical across Tasks 2–4; `ClipBand({ clip, index, root })` signature stable between Task 3 (temporary) and Task 4 (final); `prepareReportForPdf(report, sourceRoot)` matches the Task 2 exporter call.
- **Honest risks:** (1) WOFF2 parsing in `@react-pdf/font` — verified in docs, but the TTF fallback is specified in Task 1 and exercised at the first visual render; (2) `ClipMetadata` field types — Task 2 tells the implementer to check the real interface before coding; (3) the Bun-side render harness may hit module-resolution friction — Task 3 Step 3 includes the dev-server fallback so QA never blocks on it.
