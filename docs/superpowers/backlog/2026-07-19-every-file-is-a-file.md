# Backlog: Every file is a file — remove the remaining "raw notice" segregation

> **RESOLVED 2026-07-19** — all four issues closed in one pass (maintainer chose option (b)
> for Issue 3). `.ari` is a first-class clip; `RawNotice` is deleted; every non-media,
> non-junk file is surfaced as an `OtherFileRef` and rolled into per-reel/report
> "other files" counts + sizes; `totalSizeBytes` (scan summary, report stats, reel stats)
> is the sum of ALL surfaced files. `.rtn` sidecars also count as other files now — their
> bytes were previously invisible to totals, same bug class as Issue 1. Note: the app has
> no exiftool path (that's analysis tooling only) — `.ari` metadata goes through the
> mediainfo worker like every clip; sparse results land as `{}`, which the model treats
> as optional.

**Origin:** 2026-07-19 PDF-report design session. Maintainer directive: a file is a file, a
backup is a backup. Technical data (name, full path, size) matters for every file on the card
and must never be hidden or set apart in a "freak" section. Plan 08 already made
`.braw`/`.r3d`/`.crm` first-class clips; this backlog removes the leftovers of the old
raw-notice design. Each issue below is self-contained and sized for its own agent session.

---

## Issue 1 — Report totals exclude raw-notice files (correctness bug, fix first)

**Where:** `packages/core/src/report/model.ts` (`buildReportModel`), `packages/core/src/scan/model.ts` (`buildScanSummary`).

**Problem:** `stats.totalSizeBytes` (and the scan summary's `totalClipSizeBytes`) sum only
`clips`. Files in `raw: RawNotice[]` (currently `.ari`) contribute nothing. A DIT hands post a
report whose total size does not match the bytes actually delivered. The whole point of the
report is byte-for-byte comparability.

**Fix:** Totals include every surfaced file (clips + raw notices). Decide with tests whether
`clipCount` stays clips-only with a separate `fileCount`, or raw files fold into `clipCount`
once Issue 2 lands (if Issue 2 is done first, this may reduce to deleting `rawCount`).

**Acceptance:** `bun test` cases where a scan containing `.ari` files reports totals equal to
the sum of ALL file sizes. Gates green from repo root.

---

## Issue 2 — Fold `.ari` (and the whole RawNotice path) into first-class clips

**Where:** `packages/core/src/media/extensions.ts`, `packages/core/src/scan/walker.ts`,
`packages/core/src/scan/model.ts`, `packages/core/src/report/model.ts`, and the app's
`raw-section.tsx` / `report-workspace.tsx` / `pdf-*` consumers.

**Problem:** `.ari` is the last extension in `UNSUPPORTED_RAW_EXTENSIONS`. Its files become
`RawNotice`s: no metadata pass, no reel grouping, no thumbnails, rendered in the app as
"Unsupported files (not decodable in browser)" with a warning triangle, and in the PDF as a
bare count. Per the maintainer: the inability to render a thumbnail does not make a file a
second-class citizen — name, full path on disk, and size still matter, and the file belongs in
its reel next to its siblings.

**Fix direction:**
- Move `.ari` into `SUPPORTED_MEDIA_EXTENSIONS`; delete `UNSUPPORTED_RAW_EXTENSIONS`,
  `isKnownRawExtension`, `RawNotice`, and the walker's raw branch.
- Metadata: route `.ari` through the exiftool path (exiftool reads ARRIRAW headers — verify
  against a real `.ari` from the corpus; if nothing useful comes back, `{}` is fine — the model
  already treats metadata as optional).
- Thumbnails: route `'none'` (same honest-placeholder treatment as `.braw`).
- Reels: no special casing — the existing reelName/top-folder grouping just applies.
- UI: delete `RawSection`; remove the RAW stat tile (or repurpose per Issue 1's counts).
- Exports: PDF and CSV render these as normal clips (the 2026-07-19 PDF spec already assumes
  this — clip bands omit missing metadata lines instead of rendering placeholders).

**Acceptance:** scanning a folder with `.ari` files shows them inside their reels in the app
and both exports, with correct sizes and paths; `RawNotice` no longer exists in the codebase;
gates green.

---

## Issue 3 — Unknown extensions are silently dropped (product decision + fix)

**Where:** `packages/core/src/scan/walker.ts` (`walk`: the final else is a silent skip).

**Problem:** Any file that is neither supported media nor `.rtn` — WAV audio, LUTs, ALE/XML
sidecars, PDFs, arbitrary files — is invisible: not a clip, not a notice, not in any count
(only `filesSeen` increments). For a backup-verification deliverable this is the worst
behavior available: the report silently under-reports the card.

**Decision needed from the maintainer (ask before coding):**
- (a) Surface every non-media file as a first-class entry in its reel (fileName, path, size —
  no metadata/thumbs), or
- (b) roll them into per-reel/report "other files" counts + sizes, or
- (c) keep media-only scope but SAY so (explicit "N files on card not listed — media report
  only" line in app + exports).

Option (a) is most consistent with "a file is a file"; junk filtering (`isJunkName`) still
applies either way.

**Acceptance:** whichever option is chosen, a card containing WAV/XML/LUT files produces a
report that either lists them or explicitly accounts for them; nothing is silent; gates green.

---

## Issue 4 — App UI copy still frames these files as defects

**Where:** `apps/web/src/features/scan/raw-section.tsx` (amber `TriangleAlert`, "Unsupported
files (not decodable in browser)"), scan-summary RAW tile copy in
`apps/web/src/features/scan/scan-screen.tsx`.

**Problem:** Warning iconography + "unsupported" framing communicates "something is wrong with
these files." Nothing is wrong with them; the browser just can't paint a preview.

**Fix:** Superseded by Issue 2 (section deleted). If any interim copy survives, it must be
neutral ("preview not available in browser"), never warning-styled. This issue exists so the
framing does not outlive Issue 2 by accident.

---

**Sequencing:** Issue 1 is a small standalone correctness fix and can ship immediately.
Issue 2 subsumes Issue 4. Issue 3 needs a maintainer decision first. None of these block the
PDF redesign (its spec renders whatever `ReportModel` provides and already assumes no
segregated section), but Issues 1–2 determine when the PDF's totals become fully honest.
