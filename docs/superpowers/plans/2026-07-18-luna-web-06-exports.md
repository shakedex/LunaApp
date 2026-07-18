# Luna Web — Plan 06: Exports Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** The report workspace exports a real **PDF** (cover, hero stats, reel sections, clip cards with thumbnails — the product's deliverable) and a **CSV** for Excel/Sheets, through the spec's extensible exporter registry, saved via File System Access with a download fallback.

**Architecture:** CSV generation and per-reel aggregates are pure core (`bun test`-ed); the exporter *registry* interface lives in the app (it deals in `Blob`s). PDF rendering is two-stage: an async **image-preparation pass** converts every thumbnail Blob to a JPEG data-URL (mime-branched — WebP from mediabunny is re-encoded via canvas, JPEG from ffmpeg passes through) and the logo to a PNG data-URL; then a plain `@react-pdf/renderer` document renders from strings only. Clip cards use `wrap={false}` (the desktop's `ShowEntire()` analog); page numbers in the footer.

**Tech Stack (additions):** `@react-pdf/renderer` — via `bun add` at latest. Nothing else. (TanStack Table/Virtual grid → Plan 07.)

**Spec:** `docs/superpowers/specs/2026-07-17-luna-web-design.md` §8.9 (registry), §12.1 (PDF layout), §12.2 (CSV columns), §12.3 (delivery).

## Global Constraints

- **Never hand-write a dependency version.** New dep via `bun add` (latest).
- **`packages/core` stays DOM-free** — CSV returns a `string`; the Blob-producing `Exporter` interface is app-side.
- **Never mutate `report.cover`** (it is a reference into `coverStore`); exporters treat the whole `ReportModel` as read-only.
- **Exporter defaults `cover.date ?? today`** anyway (belt-and-braces on top of the seeded store).
- **PDF images:** react-pdf accepts JPEG/PNG only — every frame goes through the mime branch; never hand a WebP to the renderer. Missing/failed frames render a placeholder box, never fabricated imagery.
- **CSV columns exactly** per spec §12.2 (no "card" column — and if one is ever added, it derives from the top-level folder, NOT the reel name; they diverge).
- **Tests:** `bun test`, core only (aggregates, CSV quoting/columns, outcome aggregation). PDF/UI are maintainer-QA.
- **Gates green after every task** (from `web/`): `bun run lint && bun run typecheck && bun test && bun run build`.
- **Saving:** `showSaveFilePicker` when available; user-cancel (AbortError) is silent; anchor-download fallback otherwise. Never upload anything.
- **No new undocumented casts.** No interactive-CLI handoffs. The .NET app at the repo root is never touched.

---

## File Structure

```
web/packages/core/src/
  report/model.ts              MODIFY: additive ReelStats on Reel (built by buildReportModel)
  export/csv.ts                NEW: generateReportCsv, aggregateThumbnailOutcome, CSV_COLUMNS
  index.ts                     MODIFY: export new module
web/packages/core/test/
  report-model.test.ts         MODIFY: assert reel.stats
  csv.test.ts                  NEW
web/app/src/features/export/
  exporter.ts                  NEW: Exporter interface, registry, runExport, buildCurrentReport
  save.ts                      NEW: saveBlob (File System Access → download fallback), reportFileName
  csv-exporter.ts              NEW: CSV Exporter wrapper (string → Blob)
  pdf-prepare.ts               NEW: prepareReportForPdf (mime-branched data-URLs)
  pdf-document.tsx             NEW: react-pdf ReportDocument
  pdf-exporter.ts              NEW: PDF Exporter (prepare → render → blob)
  export-buttons.tsx           NEW: workspace export buttons with busy/error state
web/app/src/features/report/report-workspace.tsx  MODIFY: render <ExportButtons/>, use reel.stats
```

---

## Task 1: Core per-reel aggregates (additive, TDD)

**Files:**
- Modify: `web/packages/core/src/report/model.ts`, `web/packages/core/test/report-model.test.ts`, `web/packages/core/src/index.ts`

**Interfaces:**
- Produces (additive to the frozen contract — existing fields unchanged):

```ts
export interface ReelStats {
  clipCount: number
  totalSizeBytes: number
  totalDurationSeconds: number
}
// Reel<TImage> gains: stats: ReelStats   (computed by buildReportModel)
```

- [ ] **Step 1: Extend the test (red)**

In `web/packages/core/test/report-model.test.ts`, inside the existing "merges metadata + thumbnails" test, add after the reels assertions:

```ts
    const a001 = model.reels.find((r) => r.name === 'A001')
    expect(a001?.stats).toEqual({ clipCount: 1, totalSizeBytes: 50, totalDurationSeconds: 5 })
    const customStats = model.reels.find((r) => r.name === 'CUSTOM')?.stats
    expect(customStats).toEqual({ clipCount: 1, totalSizeBytes: 100, totalDurationSeconds: 10 })
```
(Recall the fixture: `A001/one.mov` (100 bytes, 10 s, reelName CUSTOM) leaves reel `A001` holding only `A001/two.mov` (50 bytes, 5 s).)

- [ ] **Step 2: Run to verify it fails**

Run: `cd web/packages/core && bun test report-model`
Expected: FAIL — `stats` undefined on reels.

- [ ] **Step 3: Implement**

In `model.ts`: add the `ReelStats` interface; add `stats: ReelStats` to `Reel`; in `buildReportModel`'s reel mapping, compute per-reel sums:

```ts
  const reels = detectReels(reportClips).map((reel) => {
    const clips = reel.clips.map(({ reelName: _drop, ...clip }) => clip)
    let totalSizeBytes = 0
    let totalDurationSeconds = 0
    for (const clip of clips) {
      totalSizeBytes += clip.sizeBytes
      totalDurationSeconds += clip.metadata.durationSeconds ?? 0
    }
    return {
      name: reel.name,
      clips,
      stats: { clipCount: clips.length, totalSizeBytes, totalDurationSeconds },
    }
  })
```
Export `ReelStats` from the barrel.

- [ ] **Step 4: Green + gates + commit**

`bun test` all green (43). All four gates exit 0.

```bash
git add web/packages/core
git commit -m "feat(core): per-reel aggregate stats on the report model"
```

---

## Task 2: Core CSV generator (TDD)

**Files:**
- Create: `web/packages/core/src/export/csv.ts`
- Modify: `web/packages/core/src/index.ts`
- Test: `web/packages/core/test/csv.test.ts`

**Interfaces:**
- Produces (exact, used by Task 3):

```ts
export const CSV_COLUMNS: readonly string[] // exactly spec §12.2 order:
// reel, fileName, relativePath, startTimecode, width, height, codec, frameRate,
// durationSeconds, sizeBytes, colorSpace, camera, iso, whiteBalance, lens,
// focalLength, aperture, shutter, gamma, thumbnailOutcome
export function aggregateThumbnailOutcome(frames: readonly ThumbnailFrame[]): ThumbnailOutcome
// [] → 'NotAttempted'; any frame Success → 'Success'; else the first frame's outcome
export function generateReportCsv(report: ReportModel): string
// header + one row per clip (reels in model order, clips in reel order),
// RFC 4180: CRLF line endings, quote fields containing , " or newline
// (doubling embedded quotes), undefined → empty field, numbers unformatted
```

- [ ] **Step 1: Write the failing test**

Create `web/packages/core/test/csv.test.ts`:

```ts
import { describe, expect, test } from 'bun:test'
import { aggregateThumbnailOutcome, CSV_COLUMNS, generateReportCsv } from '../src/export/csv'
import type { ReportModel } from '../src/report/model'
import type { ThumbnailFrame } from '../src/thumbs/model'

const frame = (outcome: ThumbnailFrame['outcome']): ThumbnailFrame => ({
  positionRatio: 0.1,
  timestampSeconds: 0,
  outcome,
})

describe('aggregateThumbnailOutcome', () => {
  test('empty means never attempted', () => {
    expect(aggregateThumbnailOutcome([])).toBe('NotAttempted')
  })
  test('any success wins', () => {
    expect(aggregateThumbnailOutcome([frame('SeekFailed'), frame('Success')])).toBe('Success')
  })
  test('otherwise the first failure is reported', () => {
    expect(aggregateThumbnailOutcome([frame('SeekFailed'), frame('DecodeFailed')])).toBe('SeekFailed')
  })
})

const model: ReportModel = {
  cover: {},
  stats: { cardCount: 1, clipCount: 2, rawCount: 0, totalDurationSeconds: 15, totalSizeBytes: 150 },
  raw: [],
  reels: [
    {
      name: 'A001',
      stats: { clipCount: 2, totalSizeBytes: 150, totalDurationSeconds: 15 },
      clips: [
        {
          id: 'A001/one.mov',
          fileName: 'one.mov',
          relativePath: 'A001/one.mov',
          extension: '.mov',
          sizeBytes: 100,
          metadata: {
            width: 1920,
            height: 1080,
            codec: 'ProRes',
            frameRate: 25,
            durationSeconds: 10,
            startTimecode: '10:20:30:00',
            camera: 'Cam "A", unit 1',
          },
          thumbnails: [frame('Success')],
        },
        {
          id: 'A001/two.mov',
          fileName: 'two.mov',
          relativePath: 'A001/two.mov',
          extension: '.mov',
          sizeBytes: 50,
          metadata: { durationSeconds: 5 },
          thumbnails: [],
        },
      ],
    },
  ],
}

describe('generateReportCsv', () => {
  test('header matches the spec column order exactly', () => {
    const header = generateReportCsv(model).split('\r\n')[0]
    expect(header).toBe(CSV_COLUMNS.join(','))
    expect(CSV_COLUMNS).toEqual([
      'reel', 'fileName', 'relativePath', 'startTimecode', 'width', 'height', 'codec',
      'frameRate', 'durationSeconds', 'sizeBytes', 'colorSpace', 'camera', 'iso',
      'whiteBalance', 'lens', 'focalLength', 'aperture', 'shutter', 'gamma',
      'thumbnailOutcome',
    ])
  })

  test('rows: values, RFC 4180 quoting, blanks for missing, CRLF', () => {
    const lines = generateReportCsv(model).split('\r\n')
    expect(lines).toHaveLength(3) // header + 2 clips, no trailing newline
    expect(lines[1]).toBe(
      'A001,one.mov,A001/one.mov,10:20:30:00,1920,1080,ProRes,25,10,100,,"Cam ""A"", unit 1",,,,,,,,Success',
    )
    expect(lines[2]).toBe('A001,two.mov,A001/two.mov,,,,,,5,50,,,,,,,,,,NotAttempted')
  })
})
```

- [ ] **Step 2: Run to verify it fails**

Run: `cd web/packages/core && bun test csv`
Expected: FAIL — module does not exist.

- [ ] **Step 3: Implement `web/packages/core/src/export/csv.ts`**

```ts
import type { ReportModel } from '../report/model'
import type { ThumbnailFrame, ThumbnailOutcome } from '../thumbs/model'

// Spec §12.2 — exact column order. Note: deliberately no "card" column; if one
// is ever added it derives from the top-level folder, NOT the reel name.
export const CSV_COLUMNS: readonly string[] = [
  'reel', 'fileName', 'relativePath', 'startTimecode', 'width', 'height', 'codec',
  'frameRate', 'durationSeconds', 'sizeBytes', 'colorSpace', 'camera', 'iso',
  'whiteBalance', 'lens', 'focalLength', 'aperture', 'shutter', 'gamma',
  'thumbnailOutcome',
]

export function aggregateThumbnailOutcome(frames: readonly ThumbnailFrame[]): ThumbnailOutcome {
  if (frames.length === 0) return 'NotAttempted'
  if (frames.some((f) => f.outcome === 'Success')) return 'Success'
  return frames[0]?.outcome ?? 'NotAttempted'
}

function field(value: unknown): string {
  if (value === undefined || value === null) return ''
  const text = String(value)
  return /[",\r\n]/.test(text) ? `"${text.replaceAll('"', '""')}"` : text
}

export function generateReportCsv(report: ReportModel): string {
  const lines = [CSV_COLUMNS.join(',')]
  for (const reel of report.reels) {
    for (const clip of reel.clips) {
      const m = clip.metadata
      lines.push(
        [
          reel.name, clip.fileName, clip.relativePath, m.startTimecode, m.width, m.height,
          m.codec, m.frameRate, m.durationSeconds, clip.sizeBytes, m.colorSpace, m.camera,
          m.iso, m.whiteBalance, m.lens, m.focalLength, m.aperture, m.shutter, m.gamma,
          aggregateThumbnailOutcome(clip.thumbnails),
        ]
          .map(field)
          .join(','),
      )
    }
  }
  return lines.join('\r\n')
}
```

Barrel: `export { aggregateThumbnailOutcome, CSV_COLUMNS, generateReportCsv } from './export/csv'`.

- [ ] **Step 4: Green + gates + commit**

`bun test` all green (48). All four gates exit 0.

```bash
git add web/packages/core
git commit -m "feat(core): RFC-4180 CSV report generator"
```

---

## Task 3: App exporter registry, save helper, CSV exporter, buttons

**Files:**
- Create: `web/app/src/features/export/exporter.ts`, `save.ts`, `csv-exporter.ts`, `export-buttons.tsx`
- Modify: `web/app/src/features/report/report-workspace.tsx` (render `<ExportButtons model={model} />`; also swap the inline per-reel reduce for `reel.stats.totalSizeBytes`)

**Interfaces:**
- Produces (exact, used by Task 5):

```ts
// exporter.ts
export interface Exporter {
  id: string
  label: string
  extension: string   // 'pdf' | 'csv' (no dot)
  mime: string
  generate(report: ReportModel<Blob>): Promise<Blob>
}
export const exporters: Exporter[]                 // registry; Task 5 pushes the PDF exporter
export async function runExport(exporter: Exporter, report: ReportModel<Blob>): Promise<void>
// generate → saveBlob(blob, reportFileName(report, exporter.extension)); rethrows generate errors

// save.ts
export function reportFileName(report: ReportModel<Blob>, extension: string): string
// `${slug(cover.projectTitle) || 'luna-report'}-${cover.date ?? today}.${extension}`
// slug: lowercase, spaces→'-', strip anything not [a-z0-9-]
export async function saveBlob(blob: Blob, fileName: string, mime: string): Promise<void>
// showSaveFilePicker path when available (AbortError → silent return); else anchor download
```

- [ ] **Step 1: Create `save.ts`**

```ts
export function reportFileName(projectTitle: string | undefined, date: string | undefined, extension: string): string {
  const slug = (projectTitle ?? '')
    .toLowerCase()
    .replaceAll(/\s+/g, '-')
    .replaceAll(/[^a-z0-9-]/g, '')
  const day = date ?? new Date().toISOString().slice(0, 10)
  return `${slug || 'luna-report'}-${day}.${extension}`
}

export async function saveBlob(blob: Blob, fileName: string, mime: string): Promise<void> {
  if ('showSaveFilePicker' in window) {
    try {
      const handle = await window.showSaveFilePicker({
        suggestedName: fileName,
        types: [{ description: fileName.split('.').pop()?.toUpperCase() ?? 'File', accept: { [mime]: [`.${fileName.split('.').pop()}`] } }],
      })
      const writable = await handle.createWritable()
      await writable.write(blob)
      await writable.close()
      return
    } catch (err) {
      if (err instanceof DOMException && err.name === 'AbortError') return // user cancelled
      throw err
    }
  }
  const url = URL.createObjectURL(blob)
  const anchor = document.createElement('a')
  anchor.href = url
  anchor.download = fileName
  anchor.click()
  URL.revokeObjectURL(url)
}
```
(Note the simpler `reportFileName(projectTitle, date, extension)` signature — primitives in, no model coupling. `accept` typing: the WICG types cover `showSaveFilePicker`; if the `accept` record's computed key fights the type, build the object in a `const` first and adapt minimally, reporting what you did.)

- [ ] **Step 2: Create `exporter.ts`**

```ts
import type { ReportModel } from '@luna-web/core'
import { reportFileName, saveBlob } from './save'

export interface Exporter {
  id: string
  label: string
  extension: string
  mime: string
  generate(report: ReportModel<Blob>): Promise<Blob>
}

// Ordered registry — mirrors the desktop's ICameraSupport modularity (spec §8.9).
// csv registers below; pdf registers in pdf-exporter.ts (Task 5).
export const exporters: Exporter[] = []

export async function runExport(exporter: Exporter, report: ReportModel<Blob>): Promise<void> {
  const blob = await exporter.generate(report)
  await saveBlob(
    blob,
    reportFileName(report.cover.projectTitle, report.cover.date, exporter.extension),
    exporter.mime,
  )
}
```

- [ ] **Step 3: Create `csv-exporter.ts`**

```ts
import { generateReportCsv } from '@luna-web/core'
import { type Exporter, exporters } from './exporter'

export const csvExporter: Exporter = {
  id: 'csv',
  label: 'CSV',
  extension: 'csv',
  mime: 'text/csv',
  generate: async (report) => new Blob([generateReportCsv(report)], { type: 'text/csv;charset=utf-8' }),
}

exporters.push(csvExporter)
```

- [ ] **Step 4: Create `export-buttons.tsx`**

```tsx
import type { ReportModel } from '@luna-web/core'
import { useState } from 'react'
import { Button } from '@/components/ui/button'
import { exporters, runExport } from './exporter'
import './csv-exporter'

export function ExportButtons({ report }: { report: ReportModel<Blob> }) {
  const [busy, setBusy] = useState<string | null>(null)
  const [error, setError] = useState<string | null>(null)

  return (
    <div className="flex items-center gap-2">
      {exporters.map((exporter) => (
        <Button
          key={exporter.id}
          disabled={busy !== null}
          onClick={() => {
            setError(null)
            setBusy(exporter.id)
            runExport(exporter, report)
              .catch((err) => setError(err instanceof Error ? err.message : String(err)))
              .finally(() => setBusy(null))
          }}
        >
          {busy === exporter.id ? 'Exporting…' : `Export ${exporter.label}`}
        </Button>
      ))}
      {error && <span className="text-destructive text-sm">{error}</span>}
    </div>
  )
}
```
(Task 5 adds `import './pdf-exporter'` here so the PDF button appears — note it now.)

- [ ] **Step 5: Wire into the workspace**

In `report-workspace.tsx`: render `<ExportButtons report={model} />` in the header row next to Start over; replace the inline `reel.clips.reduce(...)` size with `formatBytes(reel.stats.totalSizeBytes)`.

- [ ] **Step 6: Gates and commit**

All four gates exit 0.

```bash
git add web/app/src
git commit -m "feat(app): exporter registry, csv export, and save flow"
```

---

## Task 4: PDF image preparation (mime-branched)

**Files:**
- Create: `web/app/src/features/export/pdf-prepare.ts`

**Interfaces:**
- Produces (exact, used by Task 5):

```ts
export interface PdfFrame { dataUrl: string | null; outcome: ThumbnailOutcome }
export interface PdfClip {
  fileName: string; relativePath: string; sizeBytes: number
  metadata: ClipMetadata; frames: PdfFrame[]
}
export interface PdfReel { name: string; stats: ReelStats; clips: PdfClip[] }
export interface PdfReport {
  cover: { projectTitle?: string; productionCompany?: string; dit?: string; director?: string; dp?: string; date: string; logoDataUrl: string | null }
  stats: ReportStats
  reels: PdfReel[]
  rawCount: number
}
export async function prepareReportForPdf(report: ReportModel<Blob>): Promise<PdfReport>
```

- [ ] **Step 1: Implement `pdf-prepare.ts`**

```ts
import type { ClipMetadata, ReelStats, ReportModel, ReportStats, ThumbnailOutcome } from '@luna-web/core'

export interface PdfFrame {
  dataUrl: string | null
  outcome: ThumbnailOutcome
}

export interface PdfClip {
  fileName: string
  relativePath: string
  sizeBytes: number
  metadata: ClipMetadata
  frames: PdfFrame[]
}

export interface PdfReel {
  name: string
  stats: ReelStats
  clips: PdfClip[]
}

export interface PdfReport {
  cover: {
    projectTitle?: string
    productionCompany?: string
    dit?: string
    director?: string
    dp?: string
    date: string
    logoDataUrl: string | null
  }
  stats: ReportStats
  reels: PdfReel[]
  rawCount: number
}

function blobToDataUrl(blob: Blob): Promise<string> {
  return new Promise((resolve, reject) => {
    const reader = new FileReader()
    reader.onload = () => resolve(reader.result as string)
    reader.onerror = () => reject(reader.error ?? new Error('Failed to read image'))
    reader.readAsDataURL(blob)
  })
}

// react-pdf renders JPEG/PNG only. ffmpeg-path frames are already JPEG and
// pass through; mediabunny-path frames are WebP and are re-encoded via canvas.
async function frameToJpegDataUrl(image: Blob, mime: string | undefined): Promise<string> {
  if (mime === 'image/jpeg') return blobToDataUrl(image)
  const bitmap = await createImageBitmap(image)
  try {
    const canvas = new OffscreenCanvas(bitmap.width, bitmap.height)
    const ctx = canvas.getContext('2d')
    if (!ctx) throw new Error('Canvas 2d context unavailable')
    ctx.drawImage(bitmap, 0, 0)
    const jpeg = await canvas.convertToBlob({ type: 'image/jpeg', quality: 0.85 })
    return blobToDataUrl(jpeg)
  } finally {
    bitmap.close()
  }
}

// Logo keeps alpha: re-encode to PNG unless it already is one.
async function logoToPngDataUrl(logo: Blob): Promise<string> {
  if (logo.type === 'image/png') return blobToDataUrl(logo)
  const bitmap = await createImageBitmap(logo)
  try {
    const canvas = new OffscreenCanvas(bitmap.width, bitmap.height)
    const ctx = canvas.getContext('2d')
    if (!ctx) throw new Error('Canvas 2d context unavailable')
    ctx.drawImage(bitmap, 0, 0)
    return blobToDataUrl(await canvas.convertToBlob({ type: 'image/png' }))
  } finally {
    bitmap.close()
  }
}

export async function prepareReportForPdf(report: ReportModel<Blob>): Promise<PdfReport> {
  const reels: PdfReel[] = []
  for (const reel of report.reels) {
    const clips: PdfClip[] = []
    for (const clip of reel.clips) {
      const frames: PdfFrame[] = []
      for (const frame of clip.thumbnails) {
        if (frame.outcome === 'Success' && frame.image) {
          try {
            frames.push({ dataUrl: await frameToJpegDataUrl(frame.image, frame.mime), outcome: frame.outcome })
          } catch {
            frames.push({ dataUrl: null, outcome: 'DecodeFailed' })
          }
        } else {
          frames.push({ dataUrl: null, outcome: frame.outcome })
        }
      }
      clips.push({
        fileName: clip.fileName,
        relativePath: clip.relativePath,
        sizeBytes: clip.sizeBytes,
        metadata: clip.metadata,
        frames,
      })
    }
    reels.push({ name: reel.name, stats: reel.stats, clips })
  }

  let logoDataUrl: string | null = null
  if (report.cover.logo) {
    try {
      logoDataUrl = await logoToPngDataUrl(report.cover.logo)
    } catch {
      logoDataUrl = null // a bad logo must not sink the export
    }
  }

  return {
    cover: {
      projectTitle: report.cover.projectTitle,
      productionCompany: report.cover.productionCompany,
      dit: report.cover.dit,
      director: report.cover.director,
      dp: report.cover.dp,
      date: report.cover.date ?? new Date().toISOString().slice(0, 10),
      logoDataUrl,
    },
    stats: report.stats,
    reels,
    rawCount: report.raw.length,
  }
}
```

- [ ] **Step 2: Gates and commit**

All four gates exit 0 (not yet imported; typecheck must pass).

```bash
git add web/app/src
git commit -m "feat(app): mime-branched pdf image preparation"
```

---

## Task 5: PDF document + exporter

**Files:**
- Create: `web/app/src/features/export/pdf-document.tsx`, `web/app/src/features/export/pdf-exporter.ts`
- Modify: `web/app/src/features/export/export-buttons.tsx` (add `import './pdf-exporter'`)
- Modify: `web/app/package.json` + `web/bun.lock` (via bun add)

**Interfaces:**
- Consumes: Task 4's `PdfReport`, Task 3's registry.
- Produces: `pdfExporter: Exporter` (id 'pdf', mime 'application/pdf'), registered FIRST in the registry (PDF is the primary deliverable — insert at index 0 so its button leads).

- [ ] **Step 1: Add the dependency (latest, via bun)**

Run: `cd web/app && bun add @react-pdf/renderer`

- [ ] **Step 2: Create `pdf-document.tsx`**

Layout per spec §12.1 (A4, 36pt margins, cover header, hero stats, reel sections, clip cards `wrap={false}`, page numbers). Complete component:

```tsx
import { Document, Image, Page, StyleSheet, Text, View } from '@react-pdf/renderer'
import type { PdfClip, PdfReport } from './pdf-prepare'

const styles = StyleSheet.create({
  page: { padding: 36, fontSize: 9, fontFamily: 'Helvetica', color: '#111' },
  coverRow: { flexDirection: 'row', justifyContent: 'space-between', alignItems: 'center', marginBottom: 12 },
  title: { fontSize: 20, fontFamily: 'Helvetica-Bold' },
  coverMeta: { color: '#555', marginTop: 2 },
  logo: { height: 32, objectFit: 'contain' },
  statsRow: { flexDirection: 'row', gap: 16, marginBottom: 16, paddingBottom: 8, borderBottomWidth: 1, borderBottomColor: '#ddd' },
  statValue: { fontSize: 14, fontFamily: 'Helvetica-Bold' },
  statLabel: { color: '#777' },
  reelHeader: { fontSize: 13, fontFamily: 'Helvetica-Bold', marginTop: 12, marginBottom: 6 },
  reelMeta: { color: '#777', fontSize: 9 },
  card: { borderWidth: 1, borderColor: '#ddd', borderRadius: 4, padding: 8, marginBottom: 8 },
  cardHeader: { flexDirection: 'row', justifyContent: 'space-between', marginBottom: 6 },
  fileName: { fontFamily: 'Helvetica-Bold' },
  timecode: { color: '#555' },
  frameRow: { flexDirection: 'row', gap: 4, marginBottom: 6 },
  frame: { width: 160, height: 90, objectFit: 'contain', backgroundColor: '#f2f2f2' },
  framePlaceholder: {
    width: 160, height: 90, backgroundColor: '#f2f2f2',
    alignItems: 'center', justifyContent: 'center', color: '#999',
  },
  metaColumns: { flexDirection: 'row', gap: 24 },
  metaColumn: { flexGrow: 1 },
  metaRow: { flexDirection: 'row', justifyContent: 'space-between', marginBottom: 1 },
  metaLabel: { color: '#777' },
  footer: { position: 'absolute', bottom: 18, left: 36, right: 36, textAlign: 'right', color: '#999', fontSize: 8 },
})

function formatBytes(n: number): string {
  if (n < 1024) return `${n} B`
  const units = ['KB', 'MB', 'GB', 'TB']
  let value = n
  let unit = -1
  while (value >= 1024 && unit < units.length - 1) {
    value /= 1024
    unit += 1
  }
  return `${value.toFixed(1)} ${units[unit]}`
}

function formatDuration(seconds: number): string {
  const total = Math.round(seconds)
  const h = Math.floor(total / 3600)
  const m = Math.floor((total % 3600) / 60)
  const s = total % 60
  return h > 0 ? `${h}:${String(m).padStart(2, '0')}:${String(s).padStart(2, '0')}` : `${m}:${String(s).padStart(2, '0')}`
}

const LEFT_META = [
  ['Resolution', (c: PdfClip) => (c.metadata.width && c.metadata.height ? `${c.metadata.width}×${c.metadata.height}` : undefined)],
  ['Codec', (c: PdfClip) => c.metadata.codec],
  ['Frame rate', (c: PdfClip) => (c.metadata.frameRate !== undefined ? `${c.metadata.frameRate} fps` : undefined)],
  ['Duration', (c: PdfClip) => (c.metadata.durationSeconds !== undefined ? formatDuration(c.metadata.durationSeconds) : undefined)],
  ['Size', (c: PdfClip) => formatBytes(c.sizeBytes)],
  ['Color space', (c: PdfClip) => c.metadata.colorSpace],
] as const

const RIGHT_META = [
  ['Camera', (c: PdfClip) => c.metadata.camera],
  ['ISO', (c: PdfClip) => c.metadata.iso],
  ['White balance', (c: PdfClip) => c.metadata.whiteBalance],
  ['Lens', (c: PdfClip) => c.metadata.lens],
  ['Focal length', (c: PdfClip) => c.metadata.focalLength],
  ['Aperture', (c: PdfClip) => c.metadata.aperture],
  ['Shutter', (c: PdfClip) => c.metadata.shutter],
  ['Gamma', (c: PdfClip) => c.metadata.gamma],
] as const

function ClipCard({ clip }: { clip: PdfClip }) {
  return (
    <View style={styles.card} wrap={false}>
      <View style={styles.cardHeader}>
        <Text style={styles.fileName}>{clip.fileName}</Text>
        {clip.metadata.startTimecode ? <Text style={styles.timecode}>TC {clip.metadata.startTimecode}</Text> : null}
      </View>
      <View style={styles.frameRow}>
        {clip.frames.map((frame, i) =>
          frame.dataUrl ? (
            <Image key={String(i)} src={frame.dataUrl} style={styles.frame} />
          ) : (
            <View key={String(i)} style={styles.framePlaceholder}>
              <Text>{frame.outcome === 'NotAttempted' ? 'RAW' : 'no preview'}</Text>
            </View>
          ),
        )}
      </View>
      <View style={styles.metaColumns}>
        <View style={styles.metaColumn}>
          {LEFT_META.map(([label, get]) => (
            <View key={label} style={styles.metaRow}>
              <Text style={styles.metaLabel}>{label}</Text>
              <Text>{get(clip) ?? '—'}</Text>
            </View>
          ))}
        </View>
        <View style={styles.metaColumn}>
          {RIGHT_META.map(([label, get]) => (
            <View key={label} style={styles.metaRow}>
              <Text style={styles.metaLabel}>{label}</Text>
              <Text>{get(clip) ?? '—'}</Text>
            </View>
          ))}
        </View>
      </View>
    </View>
  )
}

export function ReportDocument({ report }: { report: PdfReport }) {
  const { cover, stats } = report
  return (
    <Document title={cover.projectTitle ?? 'Camera report'} creator="Luna Web">
      <Page size="A4" style={styles.page}>
        <View style={styles.coverRow}>
          <View>
            <Text style={styles.title}>{cover.projectTitle ?? 'Camera report'}</Text>
            <Text style={styles.coverMeta}>
              {[cover.productionCompany, cover.date].filter(Boolean).join(' · ')}
            </Text>
            <Text style={styles.coverMeta}>
              {[
                cover.dit ? `DIT ${cover.dit}` : undefined,
                cover.director ? `Dir ${cover.director}` : undefined,
                cover.dp ? `DP ${cover.dp}` : undefined,
              ]
                .filter(Boolean)
                .join(' · ')}
            </Text>
          </View>
          {cover.logoDataUrl ? <Image src={cover.logoDataUrl} style={styles.logo} /> : null}
        </View>

        <View style={styles.statsRow}>
          <View><Text style={styles.statValue}>{stats.cardCount}</Text><Text style={styles.statLabel}>Cards</Text></View>
          <View><Text style={styles.statValue}>{stats.clipCount}</Text><Text style={styles.statLabel}>Clips</Text></View>
          <View><Text style={styles.statValue}>{formatDuration(stats.totalDurationSeconds)}</Text><Text style={styles.statLabel}>Duration</Text></View>
          <View><Text style={styles.statValue}>{formatBytes(stats.totalSizeBytes)}</Text><Text style={styles.statLabel}>Size</Text></View>
          <View><Text style={styles.statValue}>{report.rawCount}</Text><Text style={styles.statLabel}>RAW</Text></View>
        </View>

        {report.reels.map((reel) => (
          <View key={reel.name}>
            <Text style={styles.reelHeader}>
              {reel.name}{'  '}
              <Text style={styles.reelMeta}>
                {reel.stats.clipCount} clips · {formatBytes(reel.stats.totalSizeBytes)} · {formatDuration(reel.stats.totalDurationSeconds)}
              </Text>
            </Text>
            {reel.clips.map((clip) => (
              <ClipCard key={clip.relativePath} clip={clip} />
            ))}
          </View>
        ))}

        <Text
          style={styles.footer}
          fixed
          render={({ pageNumber, totalPages }) => `Generated by Luna Web · ${pageNumber}/${totalPages}`}
        />
      </Page>
    </Document>
  )
}
```
(The local `formatBytes`/`formatDuration` duplicates are deliberate: `@/lib/format` is app-DOM-adjacent and tiny; keeping the PDF module self-contained avoids importing app UI helpers into the react-pdf render tree. If you prefer importing them and it typechecks cleanly, do so and report.)

- [ ] **Step 3: Create `pdf-exporter.ts`**

```ts
import { pdf } from '@react-pdf/renderer'
import { createElement } from 'react'
import { type Exporter, exporters } from './exporter'
import { ReportDocument } from './pdf-document'
import { prepareReportForPdf } from './pdf-prepare'

export const pdfExporter: Exporter = {
  id: 'pdf',
  label: 'PDF',
  extension: 'pdf',
  mime: 'application/pdf',
  generate: async (report) => {
    const prepared = await prepareReportForPdf(report)
    return pdf(createElement(ReportDocument, { report: prepared })).toBlob()
  },
}

exporters.unshift(pdfExporter) // PDF leads — it is the product's deliverable
```

- [ ] **Step 4: Wire the side-effect import**

In `export-buttons.tsx` add `import './pdf-exporter'` alongside the csv import.

**VERIFY-DON'T-ASSUME:** check the installed `@react-pdf/renderer` types for: `pdf(element).toBlob()` existence; `Text render={({pageNumber,totalPages})}` + `fixed`; `Image src` accepting data-URI strings; `View wrap={false}`. Adapt minimally per its types and report deviations. If the bundler complains about react-pdf's node-oriented entry, use its browser/ESM entry per its package exports and report.

- [ ] **Step 5: Gates and commit**

All four gates exit 0. Note the bundle-size effect of react-pdf in the report (it's large — acceptable; it ships with the app, not per-export).

```bash
git add web/app web/bun.lock
git commit -m "feat(app): react-pdf report exporter"
```

---

## Definition of done (this plan)

- `bun test` green (48 core tests).
- All gates green from `web/`.
- **Manual QA (maintainer, non-blocking, Chromium):**
  1. Full run → workspace → **Export PDF**: save dialog appears; the PDF opens with cover (title/co/DIT/dir/DP/date/logo), hero stats, reel sections with per-reel totals, clip cards (3 thumbnails or placeholders, two metadata columns, TC badge), page numbers; no card split across pages.
  2. Mixed decode paths (mediabunny WebP + ffmpeg JPEG clips) both render in the PDF.
  3. **Export CSV**: opens in Excel/Sheets with the exact §12.2 columns; a filename with commas/quotes round-trips; blanks for missing fields.
  4. Cancel the save dialog → silent, no error.
  5. RAW-bearing card → RAW placeholders in PDF, rawCount in stats.
  6. Filename: `my-project-2026-07-18.pdf` shape; falls back to `luna-report-…` with no title.

## Self-review notes

- **Spec coverage:** §8.9 registry (app-side, ordered, PDF first) ✓; §12.1 layout (A4/36pt, cover, hero, reels, `wrap={false}` cards, page numbers; "no cover page" desktop parity → cover header block) ✓; §12.2 exact columns, RFC 4180, CRLF ✓; §12.3 File System Access save + download fallback + silent cancel ✓.
- **P5 carry-forwards addressed:** date belt-and-braces (prepare + filename) ✓; cover never mutated (prepare builds a new structure) ✓; mime-branch (frameToJpegDataUrl) ✓; per-reel aggregates promoted into core (Task 1) and consumed by workspace/PDF ✓; no "card" column, divergence documented ✓.
- **Placeholders:** none. Tasks 4–5 are staged so each commits green (prepare is standalone; document+exporter+registration land together).
- **Type consistency:** `Exporter`/`exporters`/`runExport` (T3→T5); `PdfReport`/`PdfClip`/`PdfFrame` (T4→T5); `ReelStats` (T1→T3/T5); `generateReportCsv`/`aggregateThumbnailOutcome` (T2→T3).
- **Honest risks:** react-pdf API surface verified-at-implement-time (Step 4 note); its bundle weight acknowledged; `createElement` in pdf-exporter avoids `.tsx` for a non-component module.
