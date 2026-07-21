---
title: Changelog
description: Notable changes to Luna Web, newest first.
---

All notable changes to Luna Web. This format is based on
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/); Luna Web follows
[ZeroVer](https://0ver.org/) — versions are `0.MINOR.PATCH` and there will never be a 1.0.

## [Unreleased]

### Changed

- Clips that need the ffmpeg decoder (MXF, AVI, and anything WebCodecs can't read) now generate thumbnails several at a time instead of one after another, so processing a card of them uses the whole machine. How many run at once follows the worker cap in Settings.

### Fixed

- A clip whose thumbnail takes too long to generate now falls back to its embedded preview, or to a no-preview tile, and stays in the report — it is no longer marked as a failed clip.
- Pressing "Start over" during thumbnailing now frees the decoders right away instead of leaving them running in the background.

## [0.14.1] - 2026-07-21

### Fixed

- Scanning a project folder that wraps everything in a single subfolder (like `CAMERA/`) no longer lumps most clips into one reel named after that subfolder — reel folders like `A001` are now recognized at any depth, and the Cards count reflects the folders inside the wrapper instead of showing 1.

## [0.14.0] - 2026-07-20

### Added

- Switch a report between card view and a compact table view — one row per clip with timecode, resolution, codec, frame rate, ISO, white balance, lens, and size.
- Recent folders on the scan screen have quick actions: scan again, open the folder's saved report (when one exists), and remove — as icon buttons with tooltips.
- While processing, queued clips are listed by name so you can see what's still waiting, not just a count.
- The reel list stays docked while you scroll a report, highlights the reel you're in, and jumps to a reel on click.
- Hover or keyboard-focus a cut-off metadata value (long lens names) to see the full text.
- The header nav highlights the page you're on.

### Changed

- File sizes use decimal units to match their KB/MB/GB labels (and how cards are sold) — the same footage now reads slightly larger than before, on screen and in exported PDF/CSV.
- The report header leads with the project title, date, and totals.
- While processing, finished clips collapse into a compact strip with a count, keeping the clips still being worked on in view; failed clips stay listed.
- Reports with hundreds of clips scroll smoothly — only the clip cards on screen are rendered.
- Clip metadata flows across both columns of a card, so clips with little camera metadata no longer leave a half-empty column.
- The logo field in Settings is a drop-and-preview well instead of a bare file picker.
- Deleting a saved report or clearing local data asks in a proper dialog.
- The Reports page and saved reports show loading placeholders instead of a blank page.
- Dimmed text is brighter, meeting accessibility contrast throughout.

### Fixed

- The report date no longer shows tomorrow's date when working near midnight (it used UTC; it now uses your local date).
- Clips without a usable preview show a labeled image placeholder instead of blank gray boxes — in the processing strip and the report's frame viewer.
- The metadata phase no longer shows thumbnail-shaped boxes; clips appear as compact name chips with their status while metadata is read.
- The logo field in Settings is a compact drop well that sits at form height instead of a large panel.

## [0.13.0] - 2026-07-20

### Added

- Sony MXF clips report the camera model (VENICE, VENICE 2, BURANO, FX6) read from the file itself — no XML sidecar needed.
- Sony X-OCN clips show "X-OCN" as their codec instead of a raw format identifier.

### Fixed

- Clips without a file extension no longer show a wrong file-type label in the PDF.
- Sony FX6 clips no longer report "Mem" (the recorder software) as the camera.

## [0.12.0] - 2026-07-20

### Added

- Scan a folder of footage entirely in your browser — files never leave your device.
- Camera reports grouped into reels, with per-reel and overall totals (clips, duration, size).
- Metadata extraction across common cinema formats, with vendor enrichment for ARRI, Sony, Canon, Blackmagic, and Panasonic.
- RAW clip support (ARRIRAW, Canon RAW, R3D, Blackmagic RAW, and more) with embedded-preview thumbnails.
- Thumbnail generation with an automatic FFmpeg fallback for clips the primary decoder can't handle.
- Export reports to PDF and CSV.
- Report Library: save a finished report to your browser and reopen it later from the Reports page, with storage usage shown and per-report delete.
- Persisted settings: worker concurrency cap, report defaults, and an optional thumbnail toggle.
- Operation-grouped activity log for each processing run.
- Deploys as a Cloudflare Worker (app and docs) via the dashboard git integration.

[unreleased]: https://github.com/shakedex/LunaApp/compare/v0.14.1...HEAD
[0.14.1]: https://github.com/shakedex/LunaApp/compare/v0.14.0...v0.14.1
[0.14.0]: https://github.com/shakedex/LunaApp/compare/v0.13.0...v0.14.0
[0.13.0]: https://github.com/shakedex/LunaApp/compare/v0.12.0...v0.13.0
[0.12.0]: https://github.com/shakedex/LunaApp/releases/tag/v0.12.0
