# Luna Web — PDF Report Design Spec

**Date:** 2026-07-19
**Status:** Approved direction (this document is the written record for the implementation plan)
**Reference:** the Hedge/Foolcat sample reports at `E:\Coding\wrapport\docs\hedge_pdf` — the
maintainer's chosen benchmark for tone and density.

## 1. Purpose & audience

One PDF serving two readers equally: the production office (it represents the DIT) and post
production (they compare it against the media they actually received). It is a **technical
verification document first**: reels, clips, file names, sizes, and full paths are the primary
content. Camera metadata is supporting detail. Thumbnails are a recognition aid, never the
focus.

**Non-goals:** decoration, "cinematic" flourishes, cover pages, gradient/texture effects.
Restraint is the brief. The document's character comes from typography, the app's exact
palette, and disciplined data layout — nothing else.

## 2. Hard rules (from the maintainer)

- **Every file is a file.** No segregated sections by file kind. A clip that yields no
  metadata or thumbnail renders as a normal band with whatever it has (name, path, size) —
  no warning icons, no "unsupported" framing, no placeholder walls. The PDF has **no RAW
  section and no RAW stat**. (App-side leftovers tracked in
  `docs/superpowers/backlog/2026-07-19-every-file-is-a-file.md`.)
- **No user-typed fields are added for the PDF.** Paths and all per-clip/per-reel data are
  derived automatically from the scan. (The existing cover form is unchanged; this spec adds
  nothing to it.)
- **File names and paths wrap, never truncate.** Post needs exact strings.
- **Nothing fabricated.** Missing values are omitted, never invented; totals sum only what
  exists.

## 3. Visual system

### Palette (the app's Cinema Dark tokens, hex-resolved for react-pdf)

| Role | Value |
| --- | --- |
| Page background | `#151519` |
| Zebra band (alternate clip rows) | `#1D1D22` |
| Path band background | `#26262C` |
| Primary text | `#E7E7EA` |
| Muted text (labels, secondary) | `#9EA0A8` |
| Accent (reel names only) | `#9AD6F2` |

Full-bleed dark pages (screen-first deliverable, per the maintainer's explicit choice). No
borders, no rounded cards, no shadows: separation comes from zebra alternation and spacing,
exactly like the Hedge reference.

### Typography

- **Geist** (400/500/600 static TTFs) — names, labels, headings.
- **Geist Mono** (400/500 static TTFs) — every comparable figure: sizes, durations, frame
  counts, timecodes, fps, resolutions, dates, page numbers, and **all paths**.
- TTFs vendored into the app (react-pdf `Font.register`; no woff2, no variable fonts).
- `Font.registerHyphenationCallback(w => [w])` — no mid-word hyphenation. Paths get
  zero-width-space insertions after each `/` at render time so they break at segment
  boundaries.
- Base sizes: body 8pt, clip name 10pt semibold, reel header 12pt semibold, title 20pt
  semibold. Muted labels inherit size, color `#9EA0A8`.

## 4. Document structure (single continuous flow, A4 portrait)

1. **Header (first page only):**
   - Left: eyebrow `CAMERA REPORT` (muted, letter-spaced, 7pt) → project title (20pt) →
     line 2: production company · date → line 3: `DIT <name> · Director <name> · DP <name>`
     (only fields that exist).
   - Right: production logo if provided (max height ~32pt).
2. **Totals line** (mono, muted labels): `N cards · N clips · <duration> · <size>` — sums
   over **all** files in the report (see §6 raw-merge note).
3. **Source path band:** full-width `#26262C` band, mono: the scan root folder name.
4. **Reel sections**, flowing continuously:
   - Reel header: name in accent `#9AD6F2` semibold left; right-aligned mono
     `N clips · <duration> · <size>`.
   - Reel path band: `#26262C`, mono: `<scanRoot>/<reelFolder>` (reels grouped by embedded
     reelName use the scan root only).
5. **Clip bands** — one per file, zebra-alternating, `wrap={false}` (a band never splits
   across pages):
   - **Left column (~40% width):**
     - File name — 10pt Geist semibold, wraps.
     - Fact lines in the Hedge style — muted inline labels, values in primary; figures mono.
       Each line renders only if it has at least one value; empty items are omitted:
       - `<EXT> · <frames> frames (<duration>) · <size>` — size always exists; frames =
         `round(durationSeconds × frameRate)` only when both exist.
       - `<width>×<height> (<aspect>) · <codec> · <fps> fps`
       - `TC <startTimecode>`
       - `<camera> · ISO <iso> · WB <whiteBalance> · <lens> · <focalLength> · <aperture> ·
         <shutter> · <gamma> · <colorSpace>` (wraps to further lines as needed)
     - **Full path** — 7pt mono, muted, wraps: `<scanRoot>/<relativePath>`. Present on every
       band, always.
   - **Right column (~60%):** a single row of up to 3 thumbnails (the 10%/50%/90% frames),
     each ~110pt wide, aspect-preserved, top-aligned. Frames that failed or don't exist are
     simply not rendered — no placeholder boxes. A clip with zero frames has an empty right
     column and the band stays compact.
6. **Fixed footer** on every page: project title left; right `Luna · <generated date> ·
   <page>/<pages>` — 7pt mono muted.

There is **no RAW section** (§2) and no other appendix.

## 5. Edge cases

- **Metadata `{}`:** band renders name + fact line 1 (extension/size) + path. Nothing else.
- **No thumbnails:** left column only; no placeholders.
- **Very long names/paths:** wrap via segment breaks; band grows vertically.
- **Single reel:** reel header still renders (post reads the reel name off it).
- **Missing cover fields:** header lines collapse; untitled reports fall back to
  `Camera report` (existing behavior).
- **Huge reports (200+ clips):** thumbnails already downscaled to ≤480px JPEG in
  `pdf-prepare.ts` (unchanged); bands are `wrap={false}` so pagination stays clean.

## 6. Scope of change

- `apps/web/src/features/export/pdf-document.tsx` — full rewrite of layout/styles to this
  spec.
- `apps/web/src/features/export/pdf-prepare.ts` —
  - gains `sourceRoot: string` in `PdfReport` (from `scanStore.sourceName`, passed by the
    caller; empty string when null);
  - ~~interim raw-merge~~ **resolved 2026-07-19:** backlog Issues 1–3 landed. `RawNotice` no
    longer exists — `.ari` is a first-class clip, non-media files roll into per-reel/report
    "other files" counts + sizes (`otherFileCount`/`otherFileSizeBytes` on `ReelStats` and
    `ReportStats`), and `totalSizeBytes` is the sum of ALL surfaced files. The PDF renders
    from `ReportModel` as-is; no interim code needed.
  - passes through nothing else new (frame timestamps explicitly rejected — thumbnails stay
    minimal).
- Font assets: static TTFs for Geist 400/500/600 + Geist Mono 400/500 vendored under
  `apps/web/src/features/export/fonts/`, registered once at module load.
- `apps/web/src/features/export/export-buttons.tsx` — only if needed to hand `sourceName`
  to the exporter.
- **Untouched:** `packages/core`, CSV exporter, cover form/store, app screens, workers.

## 7. Testing

Per project convention: no UI test runner. Verification is maintainer QA — export against the
real corpus (multi-reel card, MXF with embedded reel names, `.braw`/`.r3d` clips, clips with
failed metadata, a 100+ clip card) and compare side-by-side with the Hedge samples for
density/readability. Gates (`lint`, `typecheck`, `bun test`, `build`) stay green; core tests
are unaffected (no core changes).
