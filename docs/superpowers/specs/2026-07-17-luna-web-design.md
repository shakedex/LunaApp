# Luna Web — Design Spec

**Date:** 2026-07-17
**Branch:** `feature/luna-web`
**Status:** Draft for review
**Author:** brainstormed with Claude

---

## 1. Summary

Luna Web is a fully client-side, browser-based reimplementation of the Luna desktop
app's **generic-codec** camera-report workflow. A user opens a URL, points it at a
camera card / footage folder on their own machine, and gets per-clip thumbnails,
extracted metadata, reel grouping, and a downloadable **PDF** (and **CSV**) camera
report — with nothing uploaded and nothing installed.

It is a separate product from the .NET desktop app, living in the same repository on
its own branch. The desktop app is untouched.

### Why a web version

- **Reach without install.** "Open the URL and use it" — no download, no signing,
  no platform gate beyond the browser.
- **Privacy as a feature.** All decoding and report generation happen on the visitor's
  device. Footage never leaves the machine. This is a genuine trust argument for a
  DIT/camera tool, and it is stated prominently in the UI and docs.

### What it is not

Luna Web is **not** a full replacement for the desktop app. It cannot decode
proprietary RAW formats (see §3), because those depend on closed vendor tools the
desktop downloads and runs as subprocesses — impossible in a browser. Luna Web covers
exactly what the desktop's `GenericCameraSupport` + FFmpeg path covers.

---

## 2. Scope

### In scope (feature parity with the desktop *generic* path)

- Recursive scan of a chosen folder for supported media.
- Per-clip metadata extraction (container + stream + camera metadata where present).
- Per-clip thumbnail extraction (3 frames at 10% / 50% / 90% of duration).
- Reel (camera roll) grouping via embedded metadata and folder-name heuristics.
- Editable report cover / branding fields (project, production company, DIT, director,
  DP, date, logo).
- On-screen results view (the "results page").
- **PDF** export (the primary deliverable) and **CSV** export, behind an extensible
  exporter registry.
- Settings persistence and a "recent sources" list.
- An in-app activity/log view (parity with the desktop log viewer).

### Out of scope (hard constraints, not choices)

- **ARRI `.ari`, Sony X-OCN, Blackmagic `.braw`, RED `.r3d` decoding.** No in-browser
  decoder exists; the desktop only handles these by shelling out to vendor executables.
  These files are *detected* during scan and shown with an explicit "unsupported format"
  notice, never fed garbage to a generic decoder.
- **Velopack auto-update**, native installers, `Process.Start` shell-outs — meaningless
  in a browser.
- **Non-Chromium browsers.** The File System Access API (recursive directory access) is
  Chromium-only. Firefox/Safari get a clean capability gate explaining the requirement,
  not a broken experience.
- **Server-side anything.** There is no backend. No upload endpoint, no account system,
  no server compute.

### Non-goals

- No media copy / offload / verify workflow (the desktop doesn't do this either).
- No editing of extracted per-clip technical metadata (it is read-only, as on desktop).
- No internationalization in v1 (English only).
- No mobile/touch layout target in v1 (desktop Chromium is the target surface).

---

## 3. Honest limitations (surfaced in-app and in docs)

1. **Chromium-only** (Chrome/Edge/Brave/etc.) because of the File System Access API.
2. **No professional RAW** (ARRI/Sony/BRAW/RED) — see §2.
3. **CPU-bound decode.** No native hardware acceleration except where the WebCodecs path
   engages the browser's hardware decoders. Large cards on weak machines will be slow.
4. **First-visit WASM download.** The ffmpeg core (~31 MB) and mediainfo WASM are fetched
   from jsDelivr on first use, then cached in the browser (see §11). Subsequent visits and
   offline use are instant.
5. **Format-dependent metadata.** Camera fields (ISO, white balance, lens, etc.) appear
   only when the container actually carries them, exactly as on the desktop generic path.

---

## 4. Primary user flow

1. User visits the app. Capability gate confirms Chromium + File System Access support.
2. User clicks **Pick folder** (or selects a recent source). Browser permission prompt.
3. App recursively scans the folder, building a clip list (fast; no decoding yet), and
   shows a pre-scan summary (N clips, total size) for confirmation — parity with the
   desktop `QuickScanFolderAsync`.
4. On confirm, the worker pool processes clips concurrently: metadata + 3 thumbnails each,
   emitting live progress and per-clip outcomes.
5. Clips are grouped into reels. The results view populates as clips complete.
6. User fills in cover / branding fields.
7. User exports: **PDF** (download or save via File System Access) and/or **CSV**.

---

## 5. Architecture overview

```
Browser (Chromium), all local
────────────────────────────────────────────────────────────────
  UI thread (React + TanStack Router/Table/Virtual/Form, shadcn/ui)
    │  reads/writes
    ▼
  TanStack Store  ◄──────────────── Persistence (IndexedDB via idb)
    ▲                                   settings, cover defaults,
    │  progress / results               recent directory handles
    │
  Orchestrator (main thread)
    │  Comlink RPC
    ▼
  Worker pool  (size ≈ hardwareConcurrency, capped)
    ├── decode router per clip:
    │     ├── WebCodecs + mp4box.js   (MP4/MOV, browser-supported codecs)
    │     └── ffmpeg.wasm             (MXF/ProRes/DNxHD/other; core cached from jsDelivr)
    ├── metadata: mediainfo.js (chunked) + ffmpeg metadata dict, merged
    └── thumbnail encode: OffscreenCanvas → WebP (screen) + JPEG (PDF)
    │
    ▼
  packages/core (pure TS, no DOM): ReelDetector, ReportModel, Exporters (PDF/CSV), types
```

Data source is the local filesystem via the File System Access API. Files are **streamed
/ range-read**, never loaded whole: `mediainfo.js` reads chunks via `file.slice`, and
ffmpeg.wasm mounts the `File` via `WORKERFS` and seeks. This is what makes 100s-of-GB
cards and multi-GB clips viable.

**No `SharedArrayBuffer` / COOP / COEP required.** WebCodecs needs neither, and we use the
single-threaded ffmpeg core (parallelism comes from running several worker instances, not
from in-core threads). This keeps hosting on any static host trivial.

---

## 6. Repository layout

The existing .NET desktop app stays exactly where it is at the repo root. All web work
lives under `web/`, a Bun workspace monorepo.

```
LunaApp/                          existing .NET desktop app (unchanged)
  web/
    package.json                  private root; workspaces: ["app","docs","packages/*"]
    bunfig.toml
    biome.json                    lint + format config (shared)
    tsconfig.base.json            shared strict TS config
    app/                          Vite + React SPA — the tool
      index.html
      vite.config.ts
      wrangler.toml               Cloudflare Worker (static assets) for the app
      src/
        main.tsx
        routes/                   TanStack Router route tree
        features/                 scan, decode, results, export, settings, activity
        components/               shadcn/ui components (owned) + app components
        workers/                  decode + metadata workers (Comlink)
        store/                    TanStack Store
        persistence/              IndexedDB (idb) wrappers
        lib/                      capability gate, logger, utils
    docs/                         Astro Starlight — documentation
      astro.config.mjs
      wrangler.toml               Cloudflare Worker (static assets) for docs
      src/content/docs/
    packages/
      core/                       pure TS, no DOM — Node/bun-testable
        src/
          model/                  Clip, Reel, ReportModel, ThumbnailFrame, enums
          reels/                  reel detection algorithm
          exporters/              exporter registry, csv exporter, pdf document model
          metadata/               field mapping / normalization (pure)
          index.ts
        test/                     bun test specs
```

Rationale: pure, browser-independent logic (reel detection, report shaping, CSV, field
normalization, exporter contracts) lives in `packages/core` so it is unit-testable with
`bun test` and reusable. Everything that touches the DOM, workers, File System Access,
WASM, or React lives in `app`.

---

## 7. Tech stack

All dependencies are permissive-licensed except the ffmpeg core, which is GPL and
acceptable because Luna Web is open-source.

### Toolchain

| Concern | Choice | Notes / alternative considered |
|---|---|---|
| Runtime + package manager + task runner | **Bun** | workspaces, `bun --filter '*' <script>`, runs Vite & Astro. Alt: pnpm + Node |
| Unit test runner | **`bun test`** | Jest-compatible, built into Bun. No Vitest. |
| Language | **TypeScript** (strict) | `tsconfig.base.json` shared |
| App bundler / dev server | **Vite** + `@vitejs/plugin-react` | — |
| Lint + format | **Biome** | single fast tool. Alt: ESLint + Prettier |
| CI | **GitHub Actions** + `oven-sh/setup-bun` | lint, typecheck, `bun test`, build |
| Dependency updates | **Renovate** | keeps deps current over time |

### App runtime

| Layer | Choice | License |
|---|---|---|
| UI | React 19 + react-dom | MIT |
| Routing | **TanStack Router** | MIT |
| State | **TanStack Store** | MIT |
| Data grid (results) | **TanStack Table** | MIT |
| List virtualization | **TanStack Virtual** | MIT |
| Forms (cover, settings) | **TanStack Form** | MIT |
| Components | **shadcn/ui** (owned code, on Radix UI) | MIT |
| Icons | **lucide-react** | ISC |
| Styling | **Tailwind CSS v4** (`@tailwindcss/vite`) | MIT |
| Worker RPC | **Comlink** | Apache-2.0 |
| Persistence wrapper | **idb** | ISC |

> TanStack Query is intentionally **not** used: there is no server state to cache;
> wrapping local WASM decode in Query adds nothing.

### Media / engine

| Job | Choice | License | Note |
|---|---|---|---|
| Demux + HW decode (MP4/MOV) | **WebCodecs** + **mp4box.js** | mp4box BSD-3 | browser-native, no WASM |
| Decode (MXF/ProRes/DNxHD/other) | **ffmpeg.wasm** (`@ffmpeg/ffmpeg`) | wrapper MIT | stock `@ffmpeg/core` GPL-2.0, from jsDelivr + cache |
| Metadata | **mediainfo.js** | wrapper MIT, MediaInfoLib BSD-2 | chunked reads |
| PDF export | **@react-pdf/renderer** | MIT | programmatic document |
| CSV export | hand-rolled in `packages/core` | — | no dependency |

---

## 8. Modules

Each module has a single purpose, a defined interface, and is understandable in
isolation.

### 8.1 Capability gate (`app/src/lib/capabilities`)
Detects `window.showDirectoryPicker`, `VideoDecoder` (WebCodecs), and WASM support. If
missing, renders a full-screen explainer ("Luna Web needs a Chromium browser") instead of
the app. Exposes a `Capabilities` object consumed by the decode router (e.g. which codecs
WebCodecs can handle via `VideoDecoder.isConfigSupported`).

### 8.2 FolderScanner (`app/src/features/scan`)
Takes a `FileSystemDirectoryHandle`, recursively walks entries, filters by the extension
allowlist (§10.1), skips entries that are unreadable. Produces a `ClipRef[]` (handle +
relative path + size + extension). Does **no** decoding. Also computes the pre-scan
summary (count, total size). Emits progress for large trees.

### 8.3 Orchestrator + Worker pool (`app/src/features/decode`, `app/src/workers`)
Owns a pool of workers (size ≈ `navigator.hardwareConcurrency`, capped — default cap 4 —
to bound memory, configurable in settings). Schedules clips onto free workers. Each worker
exposes a Comlink interface: `processClip(clipRef, options) → ClipResult`. Emits per-clip
lifecycle events (`queued → processing → done|failed`) into the store. A worker crash is
isolated to its current clip; that clip is retried once on a fresh worker, then marked
failed.

### 8.4 Decode router (in-worker) (`app/src/workers/decode`)
Per clip, selects a path (see §10.2):
- **WebCodecs path**: mp4box.js parses the MP4/MOV, locates samples near each target
  timestamp, feeds `EncodedVideoChunk`s (from the preceding keyframe) to a `VideoDecoder`,
  captures the target `VideoFrame`.
- **ffmpeg path**: mounts the `File` via `WORKERFS`, seeks to each target timestamp,
  extracts the frame. The ffmpeg core is loaded from jsDelivr and served from the browser
  cache after first fetch.
Both hand a decoded frame to the thumbnail encoder.

### 8.5 Thumbnail encoder (`app/src/workers/decode`)
Draws each decoded frame to an `OffscreenCanvas` scaled to 1280 px wide (aspect
preserved), then encodes **WebP** (quality ~0.85) for on-screen display and **JPEG** for
PDF embedding (react-pdf accepts JPEG/PNG, not WebP). Returns `ThumbnailFrame`s (see §9).

### 8.6 Metadata (`app/src/workers/metadata`, mapping in `packages/core/metadata`)
Runs `mediainfo.js` (chunked `readChunk` via `file.slice`) and reads the ffmpeg format /
stream metadata dictionary, then merges and normalizes into the `ClipMetadata` shape. The
normalization/field-mapping logic (pure) lives in `packages/core` and is unit-tested; the
I/O lives in `app`.

### 8.7 ReelDetector (`packages/core/reels`) — pure
Ports the desktop `ReelDetectionService`: group clips by embedded reel/roll name from
metadata; fall back to folder-name heuristics (e.g. `A001`, `REEL_01`). Deterministic and
fully unit-tested. Input: `Clip[]`. Output: `Reel[]`.

### 8.8 ReportModel (`packages/core/model`) — pure
The single normalized structure every exporter and the results view consume: cover fields,
hero stats, and reels → clips → thumbnails + metadata. Built by a pure function from
`Clip[]` + cover fields. Unit-tested. **`cardCount`** is defined as the number of
top-level subfolders directly under the picked root that contain media (a camera card is
conventionally one such folder); if media sits at the root with no such subfolders,
`cardCount` is 1.

### 8.9 Exporters (`packages/core/exporters`)
```ts
interface Exporter {
  id: string
  label: string
  extension: string
  mime: string
  generate(report: ReportModel, opts?: unknown): Promise<Blob>
}
```
A registry (array/map) mirroring the desktop `ICameraSupport` modularity. v1 ships:
- **PdfExporter** — builds a `@react-pdf/renderer` document (§12.1). Lives in `app` (needs
  react-pdf) but consumes the core `ReportModel` and implements the core `Exporter`
  interface.
- **CsvExporter** — pure, in `packages/core` (§12.2).
Future exporters (JSON, checksum/MHL manifest) register without touching existing code.

### 8.10 Results UI (`app/src/features/results`)
- **Route tree** (§8.13) with the main results route, settings, credits, activity.
- **TanStack Table** renders the per-clip metadata grid (sortable/filterable), with
  **TanStack Virtual** for hundreds of clips. shadcn's data-table pattern wraps it.
- A card/gallery view for the thumbnail-forward layout, also virtualized.
- Live progress surface driven by store events.

### 8.11 Store (`app/src/store`) — TanStack Store
Holds: scan state, clip list + per-clip status/results, reels, cover fields, processing
progress, and settings (hydrated from persistence). UI subscribes via TanStack Store's
React bindings. Pure derived selectors (hero stats, filtered/sorted views) computed from
store state.

### 8.12 Persistence (`app/src/persistence`) — IndexedDB via idb
Stores: `settings` (one record), `recentSources` (up to 10 `FileSystemDirectoryHandle`s +
display path + last-used timestamp). Directory handles require IndexedDB (they are
structured-cloneable but not localStorage-serializable). On reuse, re-request permission
via `handle.requestPermission()`. Schema versioning + migration in §14.

### 8.13 Router (`app/src/routes`) — TanStack Router
Client-side routes:
- `/` — the tool (pick folder → scan → results). Sub-states (empty / scanning / processing
  / results) are store-driven, not separate routes.
- `/settings` — settings form.
- `/credits` — credits / about (parity with desktop Credits).
- `/activity` — the in-app log/activity view (§8.14).
No auth guards (no accounts). A single not-found route. The app must **not** define a
`/docs` route — that path is routed at the Cloudflare edge to the separate docs Worker
(§18), so the SPA never sees it. A "Docs" link in the app is a plain anchor to `/docs`.

### 8.14 Logger + activity view (`app/src/lib/logger`, `/activity` route)
Parity with the desktop's Serilog + log viewer, adapted for the browser: a lightweight
logger writes structured entries to an in-memory ring buffer (and `console`), surfaced in
the `/activity` route with level filters. Optionally the user can download the current log
as a text file. No remote log sink.

---

## 9. Core data model (`packages/core/model`)

```ts
type ThumbnailOutcome =
  | 'Success' | 'NoDecoder' | 'SeekFailed'
  | 'DecodeFailed' | 'ContainerOpenFailed' | 'NotAttempted'

interface ThumbnailFrame {
  positionRatio: number        // 0.1 | 0.5 | 0.9
  timestampSeconds: number
  webp?: Blob                  // for on-screen
  jpeg?: Blob                  // for PDF embedding
  outcome: ThumbnailOutcome
}

interface ClipMetadata {
  // standard (reliable)
  width?: number; height?: number
  codec?: string
  frameRate?: number
  durationSeconds?: number
  sizeBytes: number
  colorSpace?: string
  // camera (present only if carried by the container)
  camera?: string; iso?: string; whiteBalance?: string
  lens?: string; focalLength?: string; aperture?: string
  shutter?: string; gamma?: string
  // timecode
  startTimecode?: string
  reelName?: string            // embedded, drives reel detection
}

interface Clip {
  id: string
  relativePath: string
  fileName: string
  extension: string
  metadata: ClipMetadata
  thumbnails: ThumbnailFrame[]
  status: 'queued' | 'processing' | 'done' | 'failed'
  decodePath?: 'webcodecs' | 'ffmpeg' | 'none'
  notice?: string              // e.g. unsupported RAW format
}

interface Reel { name: string; clips: Clip[] }

interface CoverFields {
  projectTitle?: string; productionCompany?: string
  dit?: string; director?: string; dp?: string
  date?: string; logo?: Blob
}

interface ReportModel {
  cover: CoverFields
  stats: { cardCount: number; clipCount: number
           totalDurationSeconds: number; totalSizeBytes: number }
  reels: Reel[]
}
```

---

## 10. Decode & scan details

### 10.1 Extension allowlist
Scanned (from desktop): `.mov .mp4 .mxf .avi .mkv .m4v .mts .m2ts .3gp .webm .wmv .flv`.
Detected-but-unsupported (shown with a notice, not decoded): `.r3d .braw .ari`. Hidden /
system entries skipped.

### 10.2 Router decision (per clip)
1. If extension is a pro-RAW type (`.r3d/.braw/.ari`) → `decodePath = none`, `notice`
   set, `ThumbnailOutcome = NoDecoder`.
2. Else if container is MP4/MOV **and** `VideoDecoder.isConfigSupported(codec)` is true
   → **WebCodecs** path.
3. Else → **ffmpeg.wasm** path (covers MXF, ProRes, DNxHD, and anything WebCodecs rejects).

### 10.3 Thumbnail parameters (from desktop)
- Count: **3** frames. Positions: **10% / 50% / 90%** of duration.
- Width: **1280 px** (aspect preserved). Screen encode WebP q≈0.85; PDF encode JPEG.
- Per-clip decode timeout: **30 s** (mirrors desktop `FfmpegThumbnailTimeout`); on timeout
  → `SeekFailed`/`DecodeFailed` as appropriate. Per-clip metadata timeout: **5 min**.

### 10.4 Seeking
Seek to nearest keyframe at/just before each target timestamp, decode forward to the
target. ffmpeg path reuses the desktop's robust MXF strategy conceptually (byte seek →
backward PTS seek fallbacks); WebCodecs path uses mp4box sample tables.

---

## 11. WASM loading & caching

`@ffmpeg/core` (~31 MB) and the mediainfo WASM are fetched from **jsDelivr** on first use
(`toBlobURL` pattern) and stored in the **Cache API** (or OPFS) so later visits load from
cache — instant and offline-capable. Because these are fetched from a CDN rather than
served as Cloudflare static assets, the 25 MiB per-asset Workers limit never applies. A
first-load progress indicator covers the initial download. Fetching program binaries from
a CDN does not affect the "footage never leaves your device" guarantee — no user data is
transmitted.

---

## 12. Exports

### 12.1 PDF (`@react-pdf/renderer`)
Programmatic document (not derived from the on-screen HTML). Layout:
- **Cover**: project title, production company, DIT, director, DP, date, logo.
- **Hero stats**: card count, clip count, total duration, total size.
- **Per-reel sections**, each a heading followed by **clip cards**.
- **Clip card**: filename + start-timecode badge, the 3 JPEG thumbnails, and two metadata
  columns — (Resolution, Codec, Frame rate, Duration, Size, Color space) and (Camera, ISO,
  White balance, Lens, Focal length, Aperture, Shutter, Gamma). Missing fields render blank.
- Page numbers in the footer. Each clip card uses `wrap={false}` so a card never splits
  across a page — the direct analog of the desktop's QuestPDF `ShowEntire()`.
- Failed thumbnails render a placeholder tile; the clip still appears with its metadata.

### 12.2 CSV (pure, `packages/core`)
One row per clip; thumbnails omitted. Columns: `reel, fileName, relativePath,
startTimecode, width, height, codec, frameRate, durationSeconds, sizeBytes, colorSpace,
camera, iso, whiteBalance, lens, focalLength, aperture, shutter, gamma, thumbnailOutcome`.
RFC-4180 quoting. Suitable for Excel and Google Sheets.

### 12.3 Delivery
Exports produce a `Blob`. Default: browser download. When File System Access write
permission is available, offer "Save to…" to a chosen location.

---

## 13. Metadata field mapping & fidelity

`packages/core/metadata` maps merged mediainfo + ffmpeg fields to `ClipMetadata`.
Standard fields (width/height, codec, frameRate, durationSeconds, sizeBytes, colorSpace)
are reliable across supported formats. Camera fields are populated only when present in the
container's metadata for that format — no fabrication, missing → blank. Precedence when
both sources disagree: mediainfo for container/stream descriptors, ffmpeg dict for
tag-style camera metadata; ties resolved by a documented precedence list in code.

---

## 14. Settings schema, versioning & migrations

IndexedDB database `luna-web`, object stores `settings` and `recentSources`, `version: 1`.

```ts
interface Settings {
  schemaVersion: number         // for migrations
  theme: 'dark' | 'light' | 'system'
  workerPoolCap: number         // default 4
  coverDefaults: Partial<CoverFields>   // logo stored as Blob
  defaultExport: { pdf: boolean; csv: boolean }
}
```

Migrations run in IndexedDB's `onupgradeneeded`, keyed off `schemaVersion`, each migration
a pure transform from vN to vN+1. Unknown/newer versions are handled defensively (load
defaults rather than crash).

---

## 15. Error handling

- **Per-clip**: typed `ThumbnailOutcome`. A failed clip appears in results and report with
  a placeholder + whatever metadata was read. Never aborts the run.
- **Worker crash**: isolated to the current clip; retried once on a fresh worker; then
  `failed`.
- **Permission denied / handle stale**: prompt to re-pick; the recent-source entry is
  marked stale.
- **Unsupported browser**: global capability gate (§8.1).
- **WASM fetch failure** (CDN unreachable): clear error with retry; app remains usable for
  already-cached engines.
- All errors are logged to the activity view (§8.14).

---

## 16. Privacy & analytics

**No analytics, no telemetry, no remote calls with user data — by default and by design.**
Unlike some comparable tools that collect aggregate stats, Luna Web sends nothing. The only
network requests are fetching program assets (app bundle from Cloudflare, WASM engines from
jsDelivr). This is stated in the UI and docs as a core promise.

---

## 17. Testing strategy

- **`bun test`** over `packages/core` only — the pure logic that must be correct:
  reel detection, report-model shaping, CSV generation/quoting, metadata field mapping &
  precedence, extension allowlist. These are deterministic and need no browser.
- **No automated UI / e2e tests.** All user-facing flows (folder pick, decode, results,
  export) are verified manually by the maintainer. (Explicit decision.)
- CI runs lint (Biome), typecheck (`tsc --noEmit`), `bun test`, and a production build of
  `app` and `docs`.

---

## 18. Build & deployment

**Domain:** the app is published at **`luna.ozer2.one`**, with documentation served from
the **`/docs` path on the same host** (not a subdomain). This requires `ozer2.one` to be a
Cloudflare-managed zone.

- **Monorepo tasks** via Bun: `bun --filter '*' build`, `bun --filter app dev`, etc.
- **App**: Vite build → static assets → Cloudflare Worker via `wrangler.toml` `assets`
  binding. No special headers needed (no cross-origin isolation).
- **Docs**: Astro Starlight build → static → a **separate** Cloudflare Worker, built with
  Astro `base: '/docs'` so all asset URLs and internal links are `/docs`-prefixed.
- **Path routing (two Workers, one host)**: Cloudflare **Workers routes** split traffic by
  path on `luna.ozer2.one` — `luna.ozer2.one/docs*` → docs Worker, `luna.ozer2.one/*` →
  app Worker. Cloudflare resolves the more specific `/docs*` route first, so the app SPA
  never receives `/docs` requests (they are handled at the edge). The two Workers keep
  independent builds and deploys; only the route patterns tie them to the shared hostname.
- **Free plan headroom**: 100k requests/day and 20k files/Worker are ample for a
  client-side app (a handful of requests per session; the heavy WASM is CDN + cached).
- **CI/CD**: GitHub Actions builds and deploys both Workers via Wrangler on merge to the
  branch's mainline.

---

## 19. App versioning

Semantic version in `app/package.json`, surfaced in the UI (Credits/footer) and stamped
into generated reports (small "Generated by Luna Web vX.Y.Z" line). Docs changelog page in
Starlight.

---

## 20. Milestones (high-level — a detailed plan follows separately)

1. **Scaffold**: Bun workspace, `app` (Vite+React+TanStack+Tailwind+shadcn), `docs`
   (Starlight), `packages/core`, Biome, TS config, CI. App boots behind the capability gate.
2. **Scan**: File System Access folder pick, recursive scan, allowlist, pre-scan summary,
   recent sources (IndexedDB).
3. **Core (pure)**: model, reel detection, CSV exporter, metadata mapping — with `bun test`.
4. **Metadata**: mediainfo.js + ffmpeg dict in a worker; populate clips.
5. **Decode**: worker pool + Comlink; ffmpeg.wasm path (WORKERFS) first, then WebCodecs
   path + router; thumbnail encode.
6. **Results UI**: store, table+virtual grid, card view, live progress, cover form.
7. **Exports**: PDF (react-pdf) + CSV wired to the registry; download / save.
8. **Settings + activity**: settings form + persistence + migrations; activity/log view.
9. **Docs**: Starlight content (what it is, privacy, supported formats, limitations, FAQ).
10. **Deploy**: two Cloudflare Workers on `luna.ozer2.one` split by Workers routes
    (`/docs*` → docs Worker, `/*` → app Worker), CI deploy.

---

## 21. Open questions / future work

- **WebCodecs codec coverage** varies by Chromium build; the router already falls back to
  ffmpeg, so this is a performance matter, not correctness.
- **Future exporters**: JSON, checksum/MHL manifest (per-clip hashing is pure compute and a
  natural web addition), and a shareable HTML bundle if ever wanted.
- **Per-clip checksums**: not in v1 scope, but the exporter registry and core model leave
  room for it.
- **If Luna Web ever goes proprietary/commercial**: revisit the GPL ffmpeg core (swap for a
  custom LGPL decode-only build). Not needed while open-source.
```
