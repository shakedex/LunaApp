# Luna Web — Plan 08: RAW Clips & Embedded Previews Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** RAW formats become first-class clips: `.braw`/`.r3d` (and new `.crm`) get full metadata (activating the BRAW enricher that currently never fires — they were stuck in the raw-notice list) and thumbnails wherever a free preview exists — `.crm` via its `PRVW` box (2048×1080), ProRes RAW `.mov` via its tail `moov/udta` poster (1920×1012), RED via the `.rtn` sidecar (720×405) — all through a zero-dependency ISO-BMFF box reader built to the maintainer's verified offsets. BRAW/ARRIRAW/ARRICORE show honest placeholders. Nothing decodes RAW essence.

**Architecture:** All parsing is pure core over `BlobLike` (header walks + one small slice, never `mdat`): a box walker handling 32-bit/64-bit (`largesize`)/to-EOF sizes, the `.crm` preview-uuid extraction, the tail-`moov` walk for ProRes RAW, the `.rtn` REDTHUMBNAIL parse, and a JPEG scanner **with SOF-dimension sanity validation** (FINDINGS' caveat: `FF D8 FF` occurs by chance in compressed essence — never trust an unvalidated hit). Routing becomes content-aware: `thumbnailRouteFor(extension, codec?)` adds a `'preview'` path, with ProRes RAW detected from the metadata pass's codec (the two-pass architecture means codec is known before thumbnails run). The scan walker learns to collect `.rtn` sidecars and associate them by basename. The app gains a third, lightweight preview queue in the thumbnail pass; extracted JPEGs are downscaled to the standard 1280px WebP so memory and PDF behavior match every other clip.

**Tech Stack:** zero new dependencies (the box reader is in-repo, per the maintainer's vetted package list).

**Requirements source:** `web/tools/FINDINGS.md` — "Verified box offsets", "Browser box-reader recipe", the `.rtn` header spec, the ARRICORE/ARRIRAW section, and the ⚠️ SOF-sanity caveat. `web/tools/box-offsets.mjs` is the reference implementation (read-only).

## Global Constraints

- **Zero new dependencies.** Core stays DOM-free (`BlobLike` reads; `Uint8Array` parsing; no DOMParser/canvas in core — the WebP re-encode is app-side).
- **Never read `mdat`.** Walks jump over it by declared size (64-bit `largesize` handled — the Ronin `mdat` is 5.6 GB). Reads are headers + one bounded slice (cap any scanned slice at 16 MB; reject bigger with a clear null).
- **Every JPEG candidate is validated** by parsing SOF dimensions and requiring sane bounds (1 ≤ dim ≤ 30000) before use — per FINDINGS' caveat. Preference: declared boxes first (crm uuid path), scan as fallback.
- **Reclassification semantics:** `.braw`, `.r3d` move OUT of `UNSUPPORTED_RAW_EXTENSIONS` into `SUPPORTED_MEDIA_EXTENSIONS`; `.crm` is added as supported. `.ari` stays a raw notice (single-frame ARRIRAW stills — unchanged). The summary's RAW tile therefore counts only `.ari`; UI copy updates accordingly.
- **Fidelity:** a missing preview (fresh on-set R3D without `.rtn`; BRAW; ARRICORE/ARRIRAW MXF) → `NoDecoder` placeholder, never a fabricated frame. RAW essence is never decoded.
- **Routing:** `'preview'` for `.crm`/`.r3d` by extension; ProRes RAW by CODEC from `metadataById` (exact matching string verified against the S006 dump — data-driven, don't guess); `.braw` → `'none'` (clip with metadata, placeholder thumb). The existing mediabunny→ffmpeg cascade is the safety net if detection misses.
- **Tests:** `bun test`, core only, with SYNTHESIZED byte fixtures (helpers build boxes/JPEGs in-test — no binary files in the repo). App stage is maintainer-QA against the real corpus.
- **Gates green after every task.** Never modify `web/tools/`. No changes under `web/app/src/components/ui/` (the maintainer's parallel design track owns it). The .NET app is never touched.

---

## File Structure

```
web/packages/core/src/
  media/extensions.ts          MODIFY: reclassify braw/r3d, add crm
  thumbs/router.ts             MODIFY: thumbnailRouteFor(extension, codec?) with 'preview'
  preview/jpeg.ts              NEW: jpegDimensions, findValidJpegs (SOF-validated)
  preview/boxes.ts             NEW: box walker over BlobLike (32/64-bit/to-EOF, mdat-skipping)
  preview/extract.ts           NEW: extractCrmPreview, extractMovTailPreview, extractRtnJpeg
  scan/walker.ts               MODIFY: collect .rtn sidecars, associate by basename
  scan/model.ts                MODIFY: ClipRef gains previewSidecar?: FileHandleLike
  index.ts                     MODIFY: export new modules
web/packages/core/test/
  jpeg.test.ts                 NEW
  boxes.test.ts                NEW
  extract.test.ts              NEW
  walker.test.ts               MODIFY: sidecar association cases
  thumbs.test.ts               MODIFY: routing cases
web/app/src/features/process/
  run-thumbnails.ts            MODIFY: third 'preview' queue (light async lanes)
  preview-frame.ts             NEW: jpeg bytes → downscaled 1280px WebP ThumbnailFrame
web/app/src/features/scan/
  scan-screen.tsx              MODIFY: summary copy (RAW tile = .ari only now)
```

---

## Task 1: Reclassification + content-aware routing (core, TDD)

**Files:**
- Modify: `src/media/extensions.ts`, `src/thumbs/router.ts`, `src/index.ts`
- Test: extend `test/extensions.test.ts`, `test/thumbs.test.ts`

**Interfaces (exact):**

```ts
// extensions.ts — new reality:
// SUPPORTED_MEDIA_EXTENSIONS gains '.braw', '.r3d', '.crm'
// UNSUPPORTED_RAW_EXTENSIONS shrinks to ['.ari']

// router.ts
export type ThumbnailRoute = 'mediabunny' | 'ffmpeg' | 'preview' | 'none'
export const PRORES_RAW_CODEC_PATTERN: RegExp // matches the S006 dump's exact Video.Format/CodecID — VERIFY from web/tools/out/*S006*.json / *RONIN*; candidates 'ProRes RAW'/'aprn'; never guess
export function thumbnailRouteFor(extension: string, codec?: string): ThumbnailRoute
// '.crm' | '.r3d' → 'preview'
// '.braw' → 'none' (clip with metadata, placeholder thumbnail)
// mediabunny-set extension WITH codec matching PRORES_RAW_CODEC_PATTERN → 'preview'
// else: existing behavior (mediabunny set → 'mediabunny', ffmpeg set → 'ffmpeg', unknown → 'none')
// keep `decodePathFor` as a deprecated alias delegating to thumbnailRouteFor(ext) so
// existing callers compile; migrate callers in Task 4.
```

TDD: red tests for every routing row above + reclassification assertions (`isSupportedMediaExtension('.braw') === true`, `isKnownRawExtension('.braw') === false`, `.ari` unchanged) + the data-driven codec pattern verified against the real dump (implementer reads `web/tools/out/` for the S006/RONIN Video.Format string and cites it in the test comment). Existing tests referencing the old lists get updated deliberately (call out each change in the report — this is an intentional semantic change, not test-weakening).

Gates + commit: `feat(core): raw clips reclassification and preview-aware thumbnail routing`

---

## Task 2: JPEG utilities + ISO-BMFF box walker (core, TDD)

**Files:**
- Create: `src/preview/jpeg.ts`, `src/preview/boxes.ts`
- Modify: `src/index.ts`
- Test: `test/jpeg.test.ts`, `test/boxes.test.ts`

**Interfaces (exact):**

```ts
// jpeg.ts
export function jpegDimensions(bytes: Uint8Array): { width: number; height: number } | null
// walks JPEG markers from SOI; reads any SOF0–SOF15 (except DHT/JPG/DAC C4/C8/CC); null when absent/malformed
export interface JpegCandidate { offset: number; length: number; width: number; height: number }
export function findValidJpegs(bytes: Uint8Array): JpegCandidate[]
// scans for FF D8 FF … FF D9 pairs; a candidate is returned ONLY if jpegDimensions
// succeeds AND 1 <= dims <= 30000 (FINDINGS ⚠️: essence bytes fake SOI hits)

// boxes.ts
export interface BoxHeader { type: string; start: number; size: number; headerSize: number; uuid?: Uint8Array }
export async function readBoxHeaderAt(blob: BlobLike, offset: number): Promise<BoxHeader | null>
// 8-byte header; size===1 → read 8 more for 64-bit largesize; size===0 → to-EOF;
// type 'uuid' → read the 16-byte extended id into .uuid
export async function walkTopLevelBoxes(
  blob: BlobLike,
  visit: (box: BoxHeader) => 'continue' | 'stop' | Promise<'continue' | 'stop'>,
): Promise<void>
// jumps by size; NEVER slices box bodies itself (mdat safety lives here)
export async function findChildBox(
  blob: BlobLike, container: BoxHeader, childType: string,
): Promise<BoxHeader | null>
// walks the container's children (container payload = after its header)
```

TDD with synthesized fixtures — include these exact test helpers so fixtures stay byte-precise:

```ts
function u32(n: number): number[] { return [n >>> 24, (n >>> 16) & 0xff, (n >>> 8) & 0xff, n & 0xff] }
function box(type: string, ...payload: number[][]): number[] {
  const body = payload.flat()
  return [...u32(8 + body.length), ...[...type].map((c) => c.charCodeAt(0)), ...body]
}
function minimalJpeg(width: number, height: number): number[] {
  return [
    0xff, 0xd8, // SOI
    0xff, 0xc0, 0x00, 0x11, 0x08, // SOF0, len 17, precision 8
    (height >> 8) & 0xff, height & 0xff, (width >> 8) & 0xff, width & 0xff,
    0x03, 0x01, 0x11, 0x00, 0x02, 0x11, 0x00, 0x03, 0x11, 0x00, // 3 components
    0xff, 0xd9, // EOI
  ]
}
function fakeBlobOf(bytes: number[]): BlobLike { /* BlobLike over a Uint8Array — slice/arrayBuffer views */ }
```

Required cases: jpegDimensions on minimalJpeg(2048,1080) and null on truncated/garbage; findValidJpegs rejecting a fake `FF D8 FF` with insane SOF dims (e.g. 3900×56032 — FINDINGS' real-world example) while accepting a valid neighbor; box walk over `ftyp`+32-bit boxes; a `size===1` largesize `mdat` jump (fixture can declare a large size while the fake blob only materializes headers — the walker must not read the body); to-EOF (`size===0`) terminal box; uuid box exposing the 16-byte id; findChildBox locating `udta` inside `moov`.

Gates + commit: `feat(core): iso-bmff box walker and sanity-validated jpeg utilities`

---

## Task 3: Preview extractors + walker sidecar collection (core, TDD)

**Files:**
- Create: `src/preview/extract.ts`
- Modify: `src/scan/walker.ts`, `src/scan/model.ts` (`previewSidecar?: FileHandleLike` on ClipRef), `src/index.ts`
- Test: `test/extract.test.ts`, extend `test/walker.test.ts`

**Interfaces (exact):**

```ts
// extract.ts
export interface EmbeddedPreview { jpeg: Uint8Array; width: number; height: number }
export const CRM_PREVIEW_UUID: Uint8Array // ea f4 2b 5e 1c 98 4b 88 b9 fb b7 dc 40 6e 4d 16 (FINDINGS)
export async function extractCrmPreview(blob: BlobLike): Promise<EmbeddedPreview | null>
// walk top-level; match the uuid box by id; slice the box (≤16MB cap), primary: JPEG at
// box start + 56 (uuid 16 + PRVW header 40); fallback within the same slice: largest findValidJpegs hit
export async function extractMovTailPreview(blob: BlobLike): Promise<EmbeddedPreview | null>
// walk top-level (jumping the 64-bit mdat) to moov; findChildBox(moov,'udta'); slice udta
// (≤16MB cap); LARGEST valid JPEG wins (448×240 vs 1920×1012 — FINDINGS)
export function extractRtnJpeg(bytes: Uint8Array): EmbeddedPreview | null
// 'REDTHUMBNAIL' ascii header (12) + 2 bytes + u32 LE jpeg length at offset 14, JPEG at 18
// (FINDINGS); validate via jpegDimensions; fallback: findValidJpegs largest

// walker.ts additions
// - collects `.rtn` files during the walk (they are NOT clips, NOT counted in filesSeen…
//   actually: count them in filesSeen like other non-media files, but never as clips)
// - after the walk, associates: a clip with extension '.r3d' gets previewSidecar = the
//   FileHandleLike of an .rtn whose lowercased basename-without-extension matches the
//   clip's, from the SAME directory (prefix match on relativePath dir)
```

TDD: extract.ts cases built from Task 2's helpers (a synthetic crm layout: `ftyp` + filler + uuid[CRM_PREVIEW_UUID]{40 filler + minimalJpeg(2048,1080)} → extract succeeds with dims; wrong uuid → null; a synthetic tail-moov mov: `ftyp` + largesize mdat + `moov{udta{ minimalJpeg(448,240) + filler + minimalJpeg(1920,1012) }}` → the larger wins; rtn: exact 18-byte header framing + a garbage-header fallback case). Walker cases: `A001C001.r3d` + `A001C001.rtn` in the same dir associate; different dir or different basename → no association; `.rtn` never appears as a clip.

Gates + commit: `feat(core): embedded preview extractors and rtn sidecar association`

---

## Task 4: App preview queue + WebP normalization

**Files:**
- Create: `web/app/src/features/process/preview-frame.ts`
- Modify: `web/app/src/features/process/run-thumbnails.ts`

**Interfaces (exact):**

```ts
// preview-frame.ts
export async function previewToFrame(preview: EmbeddedPreview): Promise<ThumbnailFrame<Blob>>
// Uint8Array → Blob('image/jpeg') → createImageBitmap → OffscreenCanvas downscale to
// ≤1280px wide (aspect preserved; no upscale) → WebP q0.85 Blob;
// frame: { positionRatio: 0.5, timestampSeconds: 0, image, mime: 'image/webp', outcome: 'Success' }
// (single poster frame — variable frame counts are already supported downstream)
```

`run-thumbnails.ts` changes:
- Routing switches to `thumbnailRouteFor(clip.extension, metadataById[clip.id]?.codec)` (migrating off the deprecated alias); split now yields THREE queues: mediabunny, ffmpeg, preview.
- Preview queue: `runPool` with trivial lanes (`createLane: () => ({})`), `concurrency: 2`, 30 s timeout per clip. Per clip: `.crm` → `extractCrmPreview(await clip.file...)` — careful: extractors take `BlobLike`; pass `clip.file.getFile()` result; `.r3d` → `clip.previewSidecar ? extractRtnJpeg(new Uint8Array(await (await clip.previewSidecar.getFile()).arrayBuffer())) : null`; ProRes-RAW-codec clips → `extractMovTailPreview(...)`. Null result → `failClip`-style terminal with outcome semantics: push a single `{ positionRatio: 0.5, timestampSeconds: 0, outcome: 'NoDecoder' }` frame via `finishClip` (status 'done', a placeholder frame — NOT 'failed'; absence of a free preview is expected, not an error).
- `.braw` (route 'none' but a real clip): give it the same single `NoDecoder` placeholder frame immediately (no queue) so rows/PDF show 'no preview' tiles rather than empty strips; thumbStatus seeds must include preview+braw clips (denominator correctness).
- The run-token/guarded-update discipline and the pool-failure sweeps extend to the new queue exactly like the ffmpeg queue (cascadedIds not needed — no cascade into preview).

Gates + commit: `feat(app): embedded-preview thumbnail queue for raw clips`

---

## Task 5: UI copy + verification sweep

**Files:**
- Modify: `web/app/src/features/scan/scan-screen.tsx`

Changes:
- Summary phase RAW notice copy: now only `.ari` (and unknowns) live in `raw[]` — reword to "N ARRIRAW frame file(s) detected — not listed as clips". The stats tile label stays but its meaning is documented in the copy.
- Confirm the summary's clip count naturally includes braw/r3d/crm now (no code change expected — assert in QA).
- Sweep: grep the app for `decodePathFor` (must be fully migrated), and for stale copy mentioning R3D/BRAW as "cannot be decoded in a browser — they will be listed without thumbnails" — the summary hint should now say previews come from embedded posters/sidecars where available.

Gates + commit: `feat(app): raw-clip aware summary copy`

---

## Definition of done (this plan)

- `bun test` green (76 prior + ~20 new across jpeg/boxes/extract/walker/routing).
- All gates green; zero changes under `web/app/src/components/ui/`.
- **Manual QA (maintainer, Chromium, real corpus):**
  1. Scan the corpus: `.braw`/`.r3d`/`.crm` appear as CLIPS with metadata (BRAW camera block finally live in-app); RAW tile counts only `.ari`.
  2. S001 `.crm` → real 2048×1080-sourced thumbnail (downscaled), metadata dims/fps/TC.
  3. S006 Ronin ProRes RAW → 1920×1012-sourced poster thumbnail; scan stays fast (headers only over the 5.6 GB file).
  4. KOMODO `.r3d` + `.rtn` → sunflower thumbnail; an `.r3d` WITHOUT `.rtn` → 'no preview' tile, clip intact.
  5. BRAW → metadata-rich row with 'no preview' tile; ARRICORE/ARRIRAW MXF unchanged (placeholder, model+TC metadata).
  6. PDF + CSV: raw clips appear in reels with their single frames/placeholders; CSV thumbnailOutcome column reflects Success/NoDecoder correctly.

## Self-review notes

- **Requirements provenance:** every offset/uuid/header constant traces to FINDINGS' verified tables; the SOF-sanity rule is a direct FINDINGS caveat; the S006 codec string is a data-driven verify-not-guess gate; `box-offsets.mjs` named as read-only reference.
- **The enricher-activation fix:** reclassification is what makes P7's BRAW enricher reachable in-app — called out explicitly so the reviewer verifies metadata now flows for `.braw`.
- **Semantics changes owned:** summary raw-count meaning changes; existing tests updated deliberately with per-change callouts required in reports.
- **Placeholders:** none; all fixtures synthesized in-test with exact byte helpers.
- **Type consistency:** `EmbeddedPreview`/`ThumbnailRoute`/`previewSidecar`/`previewToFrame` names fixed here; Task 4 consumes Tasks 1–3's exact exports.
- **Out of scope:** LibRaw/full debayer (rejected per FINDINGS), exiftool in-app (native-only), BRAW thumbnails (no free path), Canon `.mp4` `Metas` blob, sidecar `.ale`/`.xml` enrichment, the maintainer's design track files.
