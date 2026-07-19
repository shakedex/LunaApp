---
title: Changelog
description: Notable changes to Luna Web, newest first.
---

All notable changes to Luna Web. This format is based on
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/); Luna Web follows
[ZeroVer](https://0ver.org/) — versions are `0.MINOR.PATCH` and there will never be a 1.0.

## [Unreleased]

## [0.12.0] - 2026-07-20

### Added

- Scan a folder of footage entirely in your browser — files never leave your device.
- Camera reports grouped into reels, with per-reel and overall totals (clips, duration, size).
- Metadata extraction across common cinema formats, with vendor enrichment for ARRI, Sony, Canon, Panasonic, and RED.
- RAW clip support (ARRIRAW, Canon RAW, R3D, Blackmagic RAW, and more) with embedded-preview thumbnails.
- Thumbnail generation with an automatic FFmpeg fallback for clips the primary decoder can't handle.
- Export reports to PDF and CSV.
- Report Library: save a finished report to your browser and reopen it later from the Reports page, with storage usage shown and per-report delete.
- Persisted settings: worker concurrency cap, report defaults, and an optional thumbnail toggle.
- Operation-grouped activity log for each processing run.
- Deploys as a Cloudflare Worker (app and docs) via the dashboard git integration.

[unreleased]: https://github.com/shakedex/LunaApp/compare/v0.12.0...HEAD
[0.12.0]: https://github.com/shakedex/LunaApp/releases/tag/v0.12.0
