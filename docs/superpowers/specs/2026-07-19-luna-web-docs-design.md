# Luna Web — Docs Site (Milestone 9) Design Spec

> Written 2026-07-19. Scope: build out the Astro Starlight docs site at `apps/docs`
> with the Milestone-9 content set. This is the docs-content milestone only —
> **not** deploy (Milestone 10).

## 1. Summary

`apps/docs` (`@luna-web/docs`) is already scaffolded (Astro Starlight, `base: '/docs'`,
`site: https://luna.ozer2.one`) but near-empty: a single placeholder `index.mdx` and no
sidebar. This milestone fills it in with a **six-page** documentation set covering what
Luna is, its privacy model, supported formats, honest limitations, and an FAQ, plus a
splash landing page. Content is **hand-authored and code-verified** against
`packages/core` so every technical claim is accurate today.

Success = a DIT or camera assistant landing on `/docs` can, in a few minutes, understand
what Luna does, trust the privacy model, know whether their footage is supported, know the
limitations, and get common questions answered — with no inaccurate or stale claims.

## 2. Goals

- One landing page (splash) + five content pages, flat sidebar.
- Every format/handling claim traceable to source in `packages/core`.
- Correct the **stale** limitation language: RAW formats (`.braw`, `.r3d`, `.crm`, `.ari`)
  are **first-class clips**, not "undecodable." (Spec `2026-07-17-luna-web-design.md` §3
  still says "No professional RAW" — that predates the 2026-07-19 "every file is a file"
  backlog and must not be repeated.)
- Zero new dependencies — Starlight built-ins only.
- Keep the maintainer's design track untouched (no edits under `apps/web`,
  `packages/core`, or `apps/web/src/components/ui`).

## 3. Non-goals (explicit)

- **No deploy work** — no Cloudflare Workers, `wrangler`, routes, or CI deploy. That is
  Milestone 10, a separate plan.
- **No new pages beyond the six** — no Exports deep-dive, Metadata-fidelity, Credits, or
  How-it-works walkthrough this pass (the maintainer chose the tight 5-topics set).
- **No auto-generated content** — the formats table is hand-authored (see §4), not emitted
  from a build script that imports core.
- **No changes to `apps/web` or `packages/core`.** Docs only read from core as a *reference
  source* while authoring; they do not import it at build time.

## 4. Content-sourcing approach

**Hand-authored, code-verified.** Pages are written as Markdown/MDX prose and tables. Every
technical claim is grounded by reading the source at authoring time:

- `packages/core/src/media/extensions.ts` — the supported-extension allowlist.
- `packages/core/src/thumbs/router.ts` — `thumbnailRouteFor()`, the authority on how each
  extension gets a thumbnail (decoded / embedded preview / placeholder).
- `packages/core/src/metadata/vendors/registry.ts` — the vendor metadata enrichers.

The Supported-formats page carries a short "source of truth" note pointing at those files so
a future editor knows where to re-verify. (Auto-generation was considered and rejected as
over-engineering for ~16 extensions; unverified hand-authoring was rejected because this
repo is data-driven and the existing spec text is already stale.)

## 5. Ground truth (verified 2026-07-19)

**Supported extensions** (`extensions.ts`, all first-class clips, 16 total):
`.mov .mp4 .mxf .avi .mkv .m4v .mts .m2ts .3gp .webm .wmv .flv .braw .r3d .crm .ari`

**Thumbnail handling** (`thumbnailRouteFor`):

| Route | Handling described in docs | Extensions |
| --- | --- | --- |
| `mediabunny` | Hardware-decoded thumbnails (WebCodecs) | `.mp4` `.m4v` `.mov` `.mkv` `.webm` `.3gp` |
| `ffmpeg` | Software-decoded thumbnails (ffmpeg.wasm — slower) | `.mxf` `.avi` `.mts` `.m2ts` `.wmv` `.flv` |
| `preview` | Embedded / sidecar preview frame | `.crm` (PRVW uuid box), `.r3d` (`.rtn` sidecar) |
| `none` | Metadata + honest placeholder thumbnail | `.braw` `.ari` |

Special cases within `mediabunny`: **ProRes** decodes via `@mediabunny/prores` (TurboRes);
**ProRes RAW** `.mov` (codec `aprn`) is routed to `preview` (embedded frame), not decoded.

**Metadata enrichment** (`registry.ts`, first-match, content-detected): ARRI (`.mov` and
`.mxf`), acquisition (Sony / Canon), BRAW, Panasonic. Camera fields appear only when the
container actually carries them.

**Privacy** (spec §16): no analytics, no telemetry, no remote calls with user data. The
only network requests fetch **program assets** — the app bundle (Cloudflare) and the WASM
engines (jsDelivr). First visit downloads the ffmpeg core (~31 MB) and mediainfo WASM, then
they are cached in the browser; subsequent visits and offline use are instant.

**Browser support** (spec §3): Chromium-based browsers (Chrome, Edge, Brave, Arc) — Luna
relies on the File System Access API and WebCodecs.

## 6. Site configuration (`apps/docs/astro.config.mjs`)

Keep the existing `title`, `description`, `base`, `site`, and GitHub `social` entry. Add:

- `logo: { src: './src/assets/luna-logo-lg.webp' }` — copy the branding mark from
  `docs/branding/luna-logo-lg.webp` (equivalently `Assets/luna-logo-lg.webp`) into
  `apps/docs/src/assets/luna-logo-lg.webp`. Copying an existing asset is non-destructive;
  no branding is created or modified.
- An explicit `sidebar` array in narrative order:
  `Overview`, `Privacy`, `Supported formats`, `Limitations`, `FAQ`.
  The splash index is intentionally **not** in the sidebar.
- (Optional, maintainer's call) replace the default `public/favicon.svg` with a Luna
  favicon. Left out of the required scope; noted only.

No dependency changes. `sharp` (already present) handles the logo image.

## 7. Pages

All content pages live under `apps/docs/src/content/docs/`. Frontmatter uses Starlight's
schema (`title`, `description`). Callouts use Markdown asides (`:::note`, `:::caution`).

### 7.1 `index.mdx` — Landing (splash)

- Frontmatter: `template: splash`, `title: Luna Web`, `hero` with:
  - `tagline`: "Camera reports in your browser. Nothing leaves your device."
  - `image`: the Luna logo asset.
  - `actions`: **Open Luna** → `https://luna.ozer2.one/` (primary); **GitHub** →
    `https://github.com/shakedex/LunaApp` (secondary, `variant: minimal`, github icon).
- Body: a `<CardGrid>` of `<LinkCard>`s to the five pages, each with a one-line summary.
- Not listed in the sidebar; splash template hides the sidebar on this page.

### 7.2 `what-it-is.md` — Overview

What Luna is and who it's for (DITs and camera assistants). The flow at a glance:
pick a folder → recursive scan + pre-scan summary → local metadata + thumbnail extraction →
clips grouped into reels → export **PDF** and/or **CSV** camera report. What a report
contains. Everything runs client-side. Links onward to Privacy and Supported formats.

### 7.3 `privacy.md` — Privacy

Lead with the promise: **your footage never leaves your device.** Explain the only network
traffic is program assets (app from Cloudflare, WASM engines from jsDelivr) — never footage,
metadata, or reports. No analytics, no telemetry. First-visit WASM download (~31 MB ffmpeg
core + mediainfo), cached thereafter → offline-capable. A `:::note` reinforcing that reports
are generated and saved locally.

### 7.4 `supported-formats.md` — Supported formats

The core reference page. Structure:

1. Intro: every file on the card is surfaced; a file is a file (name, path, size always
   recorded), and most get a thumbnail.
2. The handling table from §5 (decoded / software-decoded / embedded preview / placeholder).
3. RAW section — `.braw`, `.r3d`, `.crm`, `.ari` are **first-class clips**: full metadata
   where the format carries it, embedded/sidecar previews where they exist (`.crm`, `.r3d`),
   honest placeholder thumbnails where the browser can't paint a frame (`.braw`, `.ari`).
   Explicitly **not** "undecodable."
4. Metadata enrichment: vendor-aware fields for ARRI, Sony/Canon, BRAW, Panasonic; fields
   appear only when present in the container.
5. A "source of truth" `:::note` pointing at `extensions.ts` / `thumbs/router.ts` /
   `metadata/vendors/registry.ts`.

### 7.5 `limitations.md` — Limitations

Honest limitations, **corrected** for the current build:

1. **Chromium-only** — needs File System Access API + WebCodecs.
2. **Software decode is slower** — the ffmpeg.wasm path (`.mxf`, DNxHD, legacy containers)
   is pure software; large MXF cards on weak machines feel it.
3. **RAW thumbnails** — `.braw` and `.ari` show placeholder thumbnails (no
   browser-paintable frame); `.crm`/`.r3d` show an embedded/sidecar preview when present.
   These are still fully catalogued clips.
4. **Format-dependent metadata** — camera fields appear only when the container carries them.
5. **First-visit download** — the WASM engines download once, then run cached/offline.

A `:::caution` clarifying that "limitation" here never means a file is dropped or hidden.

### 7.6 `faq.md` — FAQ

Q&A covering: Which browsers? Does my footage upload anywhere? Why do some RAW clips have no
thumbnail? Why is a large MXF card slow? Does it work offline? Is it free / open-source
(Apache-2.0)? Ends with a link to the project **Disclaimer** (not affiliated with ARRI,
Blackmagic, Sony, etc.).

## 8. Starlight components used (all built-in, no deps)

- `splash` template + `hero` frontmatter (landing).
- `<Card>`, `<CardGrid>`, `<LinkCard>` from `@astrojs/starlight/components`.
- Markdown asides `:::note` / `:::caution`.
- Standard Markdown tables.

## 9. Verification

- `bun --filter '@luna-web/docs' run typecheck` (`astro check`) — no errors.
- `bun --filter '@luna-web/docs' run build` — clean production build; `dist/` emits
  `index.html` and the five pages.
- `bun --filter '@luna-web/docs' dev` — maintainer visually reviews the splash hero, sidebar
  order, logo, and each page in the browser (manual QA per project convention — no automated
  UI tests).
- Content accuracy self-check: cross-read every format/handling claim against the three core
  source files listed in §4 before closing.

## 10. Out of scope / follow-ups

- Milestone 10 deploy (Workers, routes, CI) — separate plan.
- Optional Luna favicon swap — maintainer's call, not required here.
- Future pages (Exports deep-dive, Metadata fidelity, Credits, How-it-works walkthrough) —
  deferred; not part of this milestone.

## 11. Files touched

- `apps/docs/astro.config.mjs` (edit: logo + sidebar)
- `apps/docs/src/assets/luna-logo-lg.webp` (new: copied branding asset)
- `apps/docs/src/content/docs/index.mdx` (rewrite: splash)
- `apps/docs/src/content/docs/what-it-is.md` (new)
- `apps/docs/src/content/docs/privacy.md` (new)
- `apps/docs/src/content/docs/supported-formats.md` (new)
- `apps/docs/src/content/docs/limitations.md` (new)
- `apps/docs/src/content/docs/faq.md` (new)

Nothing under `apps/web`, `packages/core`, `tools/`, or `.github/` is touched.
