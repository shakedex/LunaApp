# Retire .NET Desktop App & Promote Luna Web to Repo Root — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Archive the .NET/Avalonia desktop app (tag + archive branch, zero history loss), remove it from the working tree, and restructure the repo so the Bun workspace (web app + Astro docs + core package) *is* the repo.

**Architecture:** Two-phase: (1) preserve — commit untracked .NET-era docs, create `archive/dotnet` branch and `dotnet-final` tag pointing at the best final .NET state (the `feature/luna-quality-pass` tip, which has ~10 unreleased bugfixes on top of master); (2) remove & restructure — `git rm` all .NET code, `git mv` the `web/` workspace to repo root as `apps/web`, `apps/docs`, `packages/core`, rewrite README/NOTICE/DISCLAIMER/.gitignore/CI for a web-first repo. Existing GitHub releases and tags are **never deleted** (installed desktop apps poll them via Velopack; leaving them intact is safe — users simply see no further updates).

**Tech Stack:** git (mv/rm/branch/tag), Bun workspaces, Biome, GitHub Actions, `gh` CLI.

## Audit Summary (what exists today)

| Area | Finding |
| --- | --- |
| .NET app | Lives at **repo root**: `LunaApp.csproj`, `LunaApp.sln`, `Program.cs`, `App.axaml(.cs)`, `ServiceRegistration.cs`, `ViewLocator.cs`, `app.manifest`, `build.ps1`, dirs `Models/` `Services/` `Styles/` `ViewModels/` `Views/` `LunaApp.Tests/` (~90 tracked files) |
| Web workspace | `web/` Bun workspace: `web/app` (Vite + React 19 + TanStack), `web/docs` (Astro Starlight, site `https://luna.ozer2.one`, base `/docs`), `web/packages/core`, `web/tools` (analysis scripts). 135 tracked files. |
| CI | `.github/workflows/ci.yml` + `release.yml` are .NET-only (delete). `web-ci.yml` runs bun lint/typecheck/test/build with `working-directory: web` (becomes the main CI). |
| Binary bloat in git | `tools/ffmpeg/win-x64/*.dll` (7 FFmpeg DLLs) and `Assets/install-splash.png` are tracked. They stay in history (no rewrite) but leave the tree. |
| Branches | `feature/luna-web` (current, active). `feature/luna-quality-pass` (worktree at `.worktrees/quality-pass`) holds ~10 .NET bugfix commits **not on master** — the best final .NET state. Only `master` exists on origin. |
| Untracked .NET-era docs | `docs/superpowers/{plans,specs}/2026-04-27-*` and `2026-05-24-luna-quality-pass*` (6 files) plus ignored `docs/CAMERA_SUPPORT.md` — commit before removal so history keeps them. |
| Local-only junk (never tracked) | `bin/` (2.7 GB), `publish/` (587 MB), `obj/` (30 MB), `docs/sdk/` (197 MB), `tools/arri/` (140 MB), sample media in `docs/` (.braw/.mxf/.mov), `art.log`. Optional manual cleanup at the end. |
| Shared assets | `Assets/luna-logo*` used by `web/tools/make-og.py` (OG image) and useful for future Tauri/Electron — **keep**. `docs/file_proccessing/` vendor-metadata research useful for web raw support — **keep**. `docs/branding/luna-logo.psd` — **keep**. |
| Legal docs | README/NOTICE/DISCLAIMER all describe the desktop app (FFmpeg DLLs, QuestPDF, Velopack, code signing) — rewrite for the web stack (ffmpeg.wasm LGPL, mediainfo.js, @react-pdf/renderer). |

### Target layout

```
apps/web/          ← was web/app        (@luna-web/app — Vite React app)
apps/docs/         ← was web/docs       (@luna-web/docs — Astro Starlight)
packages/core/     ← was web/packages/core (@luna-web/core)
tools/analysis/    ← was web/tools      (throwaway clip-analysis scripts)
docs/              ← unchanged (superpowers plans/specs, vendor research, branding)
Assets/            ← logos only (splash removed)
package.json, bun.lock, bunfig.toml, biome.json, tsconfig.base.json  ← from web/
.github/workflows/ci.yml  ← renamed web-ci.yml, runs from root
README.md, LICENSE, NOTICE, DISCLAIMER.md  ← rewritten for Luna Web
```

## Global Constraints

- All work happens on `feature/luna-web` (the active branch). No history rewrite, no `push --force`.
- **Never delete** existing GitHub releases, the `v*` release tags, or `origin/master`. Installed desktop apps poll releases via Velopack; intact-but-frozen is the correct end state.
- Bun-only tooling (`bun install`, `bun run …`); no npm/yarn/pnpm. No new dependencies are added by this plan.
- Get Shaked's approval before any `git push` and before the outward-facing `gh` edits in Task 8.
- Task 9 (local disk cleanup) is optional and destructive — Shaked runs it manually, never automated.
- Quality gate after every restructure step: `bun run lint && bun run typecheck && bun test && bun run build` must pass.

---

### Task 1: Preserve untracked .NET-era docs in history

**Files:**
- Add: `docs/superpowers/plans/2026-04-27-lunar-scan-overlay.md`
- Add: `docs/superpowers/plans/2026-04-27-velopack-msi-and-camera-support-prompt.md`
- Add: `docs/superpowers/plans/2026-05-24-luna-quality-pass.md`
- Add: `docs/superpowers/specs/2026-04-27-lunar-scan-overlay-design.md`
- Add: `docs/superpowers/specs/2026-04-27-velopack-msi-and-camera-support-prompt-design.md`
- Add: `docs/superpowers/specs/2026-05-24-luna-quality-pass-design.md`
- Add (force, currently gitignored): `docs/CAMERA_SUPPORT.md`

**Interfaces:**
- Produces: a commit on `feature/luna-web` containing every .NET-era doc, so Task 3's deletions still leave everything recoverable from history.

- [ ] **Step 1: Verify the six plan/spec files are the only relevant untracked files**

Run: `git status --porcelain`
Expected: exactly the six `docs/superpowers/...` files with `??` status (plus this plan file itself).

- [ ] **Step 2: Stage them, force-adding the ignored camera-support doc**

```bash
git add docs/superpowers/plans/2026-04-27-lunar-scan-overlay.md \
        docs/superpowers/plans/2026-04-27-velopack-msi-and-camera-support-prompt.md \
        docs/superpowers/plans/2026-05-24-luna-quality-pass.md \
        docs/superpowers/specs/2026-04-27-lunar-scan-overlay-design.md \
        docs/superpowers/specs/2026-04-27-velopack-msi-and-camera-support-prompt-design.md \
        docs/superpowers/specs/2026-05-24-luna-quality-pass-design.md \
        docs/superpowers/plans/2026-07-19-retire-dotnet-restructure.md
git add -f docs/CAMERA_SUPPORT.md
```

- [ ] **Step 3: Commit**

```bash
git commit -m "docs: preserve .NET-era plans, specs, and camera-support doc before archival"
```

### Task 2: Create archive refs and dissolve the quality-pass worktree

**Files:** none (git refs only)

**Interfaces:**
- Consumes: branch `feature/luna-quality-pass` at `b0027d8` (worktree `.worktrees/quality-pass`).
- Produces: branch `archive/dotnet` and tag `dotnet-final`, both at `b0027d8` — the permanent, discoverable home of the desktop app.

- [ ] **Step 1: Confirm the quality-pass tip is master + only .NET fixes**

Run: `git log --oneline master..feature/luna-quality-pass`
Expected: ~10 commits, all `fix(...)`/`perf(...)`/`feat(M3)` .NET quality-pass work (top: `b0027d8 fix(L2): atomic AppSettings.Save via temp+rename`). If anything web-related appears here, STOP and ask Shaked.

- [ ] **Step 2: Create the archive branch and tag**

```bash
git branch archive/dotnet feature/luna-quality-pass
git tag -a dotnet-final feature/luna-quality-pass -m "Final state of the Luna .NET/Avalonia desktop app. Superseded by Luna Web. Includes the unreleased quality-pass fixes."
```

- [ ] **Step 3: Verify refs**

Run: `git log --oneline -1 archive/dotnet dotnet-final`
Expected: both show `b0027d8`.

- [ ] **Step 4: Remove the worktree and the now-redundant feature branch**

```bash
git worktree remove .worktrees/quality-pass
git branch -D feature/luna-quality-pass
```

Expected: `git worktree list` shows only the main tree; `git branch` shows `archive/dotnet`, `feature/luna-web`, `master`.

- [ ] **Step 5: Push the archive refs (requires Shaked's approval)**

```bash
git push origin archive/dotnet dotnet-final
```

### Task 3: Remove the .NET app from the tree

**Files:**
- Delete (tracked): `App.axaml`, `App.axaml.cs`, `Program.cs`, `ServiceRegistration.cs`, `ViewLocator.cs`, `app.manifest`, `LunaApp.csproj`, `LunaApp.sln`, `build.ps1`, `Models/`, `Services/`, `Styles/`, `ViewModels/`, `Views/`, `LunaApp.Tests/`, `tools/ffmpeg/`, `Assets/install-splash.png`, `docs/INSTALLATION.md`, `docs/VELOPACK_AUTO_UPDATE.md`, `docs/CAMERA_SUPPORT.md`, `.vscode/tasks.json`, `.github/workflows/ci.yml`, `.github/workflows/release.yml`

**Interfaces:**
- Consumes: Task 1's commit (so `docs/CAMERA_SUPPORT.md` is in history before deletion) and Task 2's refs (so the app is browsable at `archive/dotnet`).
- Produces: a tree whose only build system is the Bun workspace in `web/`. Kept on purpose: `Assets/luna-logo*` (OG-image script + future Tauri/Electron), `docs/branding/`, `docs/file_proccessing/`, `docs/superpowers/`, `LICENSE`, `NOTICE`, `DISCLAIMER.md`, `README.md` (rewritten in Task 6).

- [ ] **Step 1: Remove all .NET code, tests, tooling, and desktop-only docs**

```bash
git rm -r App.axaml App.axaml.cs Program.cs ServiceRegistration.cs ViewLocator.cs \
  app.manifest LunaApp.csproj LunaApp.sln build.ps1 \
  Models Services Styles ViewModels Views LunaApp.Tests \
  tools/ffmpeg Assets/install-splash.png \
  docs/INSTALLATION.md docs/VELOPACK_AUTO_UPDATE.md docs/CAMERA_SUPPORT.md \
  .vscode/tasks.json .github/workflows/ci.yml .github/workflows/release.yml
```

- [ ] **Step 2: Verify no .NET sources remain tracked**

Run: `git ls-files | grep -iE '\.(cs|axaml|csproj|sln|ps1)$'`
Expected: no output.

Run: `git ls-files | grep -vE '^(web/|docs/|Assets/|\.github/)' `
Expected: only `.gitignore`, `DISCLAIMER.md`, `LICENSE`, `NOTICE`, `README.md`.

- [ ] **Step 3: Commit**

```bash
git commit -m "chore: remove .NET desktop app (archived at archive/dotnet, tag dotnet-final)"
```

### Task 4: Promote the Bun workspace to repo root

**Files:**
- Move: `web/app` → `apps/web`; `web/docs` → `apps/docs`; `web/packages/core` → `packages/core`; `web/tools` → `tools/analysis`
- Move: `web/package.json` → `package.json`; `web/biome.json` → `biome.json`; `web/bun.lock` → `bun.lock`; `web/bunfig.toml` → `bunfig.toml`; `web/tsconfig.base.json` → `tsconfig.base.json`
- Delete: `web/README.md` (content folds into the root README in Task 6)
- Modify: `package.json` (workspaces globs), `apps/web/tsconfig.app.json:2`, `apps/web/tsconfig.node.json:2` (extends depth), `biome.json` (includes paths)
- Modify: `.github/workflows/web-ci.yml` → rename to `.github/workflows/ci.yml`, run from root

**Interfaces:**
- Consumes: the post-Task-3 tree.
- Produces: `bun install` / `bun run lint|typecheck|test|build` working from repo root; package names (`@luna-web/app`, `@luna-web/docs`, `@luna-web/core`) unchanged, so `bun --filter '@luna-web/app' dev` etc. keep working.

- [ ] **Step 1: Move everything with git mv (preserves history)**

```bash
mkdir apps
git mv web/app apps/web
git mv web/docs apps/docs
git mv web/packages/core packages/core
git mv web/tools tools/analysis
git mv web/package.json package.json
git mv web/biome.json biome.json
git mv web/bun.lock bun.lock
git mv web/bunfig.toml bunfig.toml
git mv web/tsconfig.base.json tsconfig.base.json
git rm web/README.md
```

- [ ] **Step 2: Update workspace globs in `package.json`**

```json
  "workspaces": [
    "apps/*",
    "packages/*"
  ],
```

(Everything else in `package.json` stays as-is.)

- [ ] **Step 3: Fix tsconfig extends depth in the moved app**

In both `apps/web/tsconfig.app.json` and `apps/web/tsconfig.node.json`, line 2:

```json
  "extends": "../../tsconfig.base.json",
```

(`packages/core/tsconfig.json` already uses `../../tsconfig.base.json` — unchanged. `apps/docs/tsconfig.json` extends `astro/tsconfigs/strict` — unchanged.)

- [ ] **Step 4: Fix Biome scope in `biome.json`**

```json
  "files": {
    "ignoreUnknown": false,
    "includes": ["**", "!apps/docs", "!docs", "!**/*.css", "!**/*.svg", "!apps/web/src/components/ui"]
  },
```

(`!apps/docs` was `!docs` = the Astro package; the new `!docs` excludes the repo-level docs folder, which contains tracked JSON/MD that Biome must not touch.)

- [ ] **Step 5: Rename and re-root the CI workflow**

```bash
git mv .github/workflows/web-ci.yml .github/workflows/ci.yml
```

Replace its full contents with:

```yaml
name: ci

on:
  push:
    branches: [master, feature/luna-web]
  pull_request:

jobs:
  quality:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: oven-sh/setup-bun@v2
        with:
          bun-version: latest
      - run: bun install --frozen-lockfile
      - run: bun run lint
      - run: bun run typecheck
      - run: bun test
      - run: bun run build
```

(The `working-directory: web` default and `paths: ['web/**']` filters are gone — the workspace is the repo now.)

- [ ] **Step 6: Delete the leftover `web/` husk and reinstall at root**

The `git mv` calls leave untracked debris behind (`web/node_modules`, `web/app/dist`, tsbuildinfo files):

```bash
rm -rf web
bun install
```

Expected: install succeeds, lockfile unchanged (`git diff --stat bun.lock` → empty).

- [ ] **Step 7: Run all quality gates from root**

Run: `bun run lint && bun run typecheck && bun test && bun run build`
Expected: all four pass. If typecheck fails, the error will name a bad relative path — fix the specific `extends`/import it names, don't guess.

- [ ] **Step 8: Grep for stale `web/` path references**

Run: `git grep -n 'web/app\|web/docs\|web/packages\|web/tools' -- ':!docs/superpowers'`
Expected: no output (historical plans/specs are allowed to keep old paths).

- [ ] **Step 9: Commit**

```bash
git add -A
git commit -m "chore: promote bun workspace to repo root (apps/web, apps/docs, packages/core)"
```

### Task 5: Rewrite `.gitignore` for a web-first repo

**Files:**
- Modify: `.gitignore` (replace all ~500 lines)

**Interfaces:**
- Consumes: Task 4's layout.
- Produces: a lean ignore file that still shields the huge local-only dirs (`docs/sdk`, `tools/arri`, sample media, leftover `bin/obj/publish`) until Task 9 deletes them.

- [ ] **Step 1: Replace `.gitignore` contents entirely**

```gitignore
# Dependencies & build output
node_modules/
dist/
.astro/
*.tsbuildinfo
*.log

# OS / editor
.DS_Store
Thumbs.db
*.swp
.idea/

# Agent tooling & local worktrees
.claude/
.superpowers/
.worktrees/

# Local-only reference material: sample media, vendor SDKs, scratch output.
# Plans/specs stay under version control; the rest of docs/ stays local.
# (Files already tracked inside docs/ — file_proccessing, branding — remain
# tracked; new files there need `git add -f`.)
docs/*
!docs/superpowers/
tools/arri/
old/

# Legacy .NET build output still on disk — safe to delete, see the
# retire-dotnet plan's final task.
bin/
obj/
publish/
```

- [ ] **Step 2: Verify nothing tracked became ignored and nothing huge became visible**

Run: `git status --porcelain`
Expected: only the `.gitignore` modification. No `bin/`, `docs/sdk/`, `tools/arri/`, or media files appear as untracked.

Run: `git ls-files -i -c --exclude-standard`
Expected: only the intentionally force-tracked `docs/` files (file_proccessing, branding, INSTALLATION-era leftovers now deleted) — i.e. nothing new/surprising.

- [ ] **Step 3: Re-run Biome (it reads .gitignore via `vcs.useIgnoreFile`)**

Run: `bun run lint`
Expected: pass.

- [ ] **Step 4: Commit**

```bash
git add .gitignore
git commit -m "chore: replace .NET gitignore with web-first ignore rules"
```

### Task 6: Rewrite README for Luna Web

**Files:**
- Modify: `README.md` (full replacement)

**Interfaces:**
- Consumes: Task 4's layout and commands.
- Produces: the repo's front page describing Luna Web, with a pointer to the archived desktop app.

- [ ] **Step 1: Replace `README.md` contents entirely**

````markdown
<div align="center">

<img src="Assets/luna-logo-lg.webp" alt="Luna" width="160" />

# Luna

**Camera reports in your browser. Nothing leaves your device.**

Luna ingests camera media entirely client-side, extracts metadata and
thumbnails, and generates production-ready camera reports — no upload,
no install, no account.

[![License: Apache 2.0](https://img.shields.io/badge/license-Apache%202.0-blue)](LICENSE)
![Runs in: Chromium](https://img.shields.io/badge/runs%20in-Chromium-4285F4)

[Open Luna](https://luna.ozer2.one) · [Docs](https://luna.ozer2.one/docs) · [Disclaimer](DISCLAIMER.md)

</div>

---

## What Luna Does

Luna is built for DITs and camera assistants. Point it at a card or a folder
of clips — everything runs locally in the browser:

- Detects reels and groups clips per camera roll
- Extracts per-clip metadata (camera, lens, ISO, shutter, color space, frame rate, duration, …)
- Generates evenly-spaced thumbnails with accurate seeking
- Produces clean, sharable camera reports with PDF and HTML export
- Raw formats (ARRIRAW, BRAW, Sony RAW) get metadata-aware handling; generic
  codecs (ProRes, H.264/265, DNxHD, …) get the full decode pipeline

**Browser support:** Chromium-based browsers (Chrome, Edge, Arc, Brave) — Luna
relies on the File System Access API and WebCodecs.

## Repo Layout

Bun workspace:

| Path | Package | What it is |
| --- | --- | --- |
| `apps/web` | `@luna-web/app` | The tool — Vite + React 19 + TanStack Router |
| `apps/docs` | `@luna-web/docs` | Docs site — Astro Starlight |
| `packages/core` | `@luna-web/core` | Pure logic: scanning, reels, metadata, report model |
| `tools/analysis` | — | Throwaway clip-analysis scripts |

## Develop

Requires [Bun](https://bun.sh).

```bash
bun install
bun --filter '@luna-web/app' dev    # run the app
bun --filter '@luna-web/docs' dev   # run the docs site
bun test                            # core unit tests
bun run lint && bun run typecheck   # quality gates
bun run build                       # build everything
```

## Tech Stack

- [React 19](https://react.dev/) + [Vite](https://vite.dev/) + [TanStack Router/Store/Form](https://tanstack.com/)
- [Tailwind CSS 4](https://tailwindcss.com/) + [Base UI](https://base-ui.com/) + shadcn-style components
- [mediabunny](https://github.com/vanilagy/mediabunny) + WebCodecs — decode & thumbnails
- [mediainfo.js](https://mediainfo.js.org/) + [ffmpeg.wasm](https://ffmpegwasm.netlify.app/) — container/codec metadata
- [@react-pdf/renderer](https://react-pdf.org/) — PDF reports
- [Astro Starlight](https://starlight.astro.build/) — documentation site

## The Desktop App (retired)

Luna started life as a .NET/Avalonia desktop app. It has been retired in
favor of the web version and is preserved in full:

- Code: branch [`archive/dotnet`](https://github.com/shakedex/LunaApp/tree/archive/dotnet) / tag `dotnet-final`
- Installers: existing [releases](https://github.com/shakedex/LunaApp/releases) remain downloadable but receive no further updates

## Contributing

PRs welcome. File an issue first so the design can be discussed, and keep
changes scoped. By submitting a contribution, you agree it is licensed under
the same [Apache License 2.0](LICENSE) as the rest of the project.

## License

Luna is licensed under the [Apache License 2.0](LICENSE). See [NOTICE](NOTICE)
for required attribution. The name "Luna" and the logo are not licensed for
use in derivative branding.

## Trademarks & Disclaimer

Luna is an independent project and is **not affiliated with, endorsed by, or
sponsored by** ARRI, Blackmagic Design, Sony, or any other camera
manufacturer. All product names, logos, and brands are property of their
respective owners. See [DISCLAIMER.md](DISCLAIMER.md).

---

<div align="center">

Built by [Shaked Lipszyc](https://github.com/shakedex)

</div>
````

- [ ] **Step 2: Verify no dead relative links**

Run: `grep -oE '\]\((docs/|Assets/)[^)]+' README.md`
Expected: every listed path exists on disk (`Assets/luna-logo-lg.webp`, `DISCLAIMER.md`, `LICENSE`, `NOTICE` — check each with `ls`).

- [ ] **Step 3: Commit**

```bash
git add README.md
git commit -m "docs: rewrite README for Luna Web"
```

### Task 7: Rewrite NOTICE and DISCLAIMER for the web stack

**Files:**
- Modify: `NOTICE` (full replacement)
- Modify: `DISCLAIMER.md` (full replacement)

**Interfaces:**
- Consumes: dependency list in `apps/web/package.json`.
- Produces: legal docs that describe what actually ships (a static web bundle), not FFmpeg DLLs and NuGet packages.

- [ ] **Step 1: Verify license names before writing them down**

Run from repo root:

```bash
bun -e "for (const p of ['@ffmpeg/ffmpeg','mediainfo.js','mediabunny','@react-pdf/renderer','react','@base-ui/react','comlink','idb','lucide-react','tailwindcss','astro']) { try { console.log(p, '→', require(p + '/package.json').license) } catch { console.log(p, '→ (resolve manually in node_modules)') } }"
```

Use the printed license identifiers in the next two steps — if any differ from what's written below, use the printed value.

- [ ] **Step 2: Replace `NOTICE` contents entirely**

```text
Luna — Camera Reports in Your Browser
Copyright 2026 Shaked Lipszyc

This product bundles third-party open-source software installed from npm.
Each dependency's license text ships inside its package; see
apps/web/package.json for the full list. Notable runtime components:

FFmpeg (WebAssembly build, via @ffmpeg/ffmpeg + @ffmpeg/core)
    License: GNU Lesser General Public License v2.1+
    The wasm binary is loaded at runtime and can be replaced by the user
    with any compatible LGPL build. Source: https://ffmpeg.org/

MediaInfo (WebAssembly build, via mediainfo.js)
    License: BSD-2-Clause. Source: https://mediaarea.net/MediaInfo

mediabunny / @mediabunny/prores        MPL-2.0
@react-pdf/renderer                    MIT
React, TanStack, Base UI, Tailwind CSS MIT
Geist / Geist Mono fonts               SIL Open Font License 1.1

The retired .NET desktop app (branch archive/dotnet, tag dotnet-final)
carries its own NOTICE describing the components it bundled.
```

- [ ] **Step 3: Replace `DISCLAIMER.md` contents entirely**

```markdown
# Disclaimer

## License

Luna is licensed under the [Apache License 2.0](LICENSE). See [NOTICE](NOTICE) for the third-party attribution that must accompany any redistribution.

## No Warranty

Luna is provided **"AS IS"**, without warranty of any kind, express or implied, including but not limited to the warranties of merchantability, fitness for a particular purpose, title, and noninfringement (Apache 2.0, §7).

In no event shall the authors or contributors be liable for any direct, indirect, incidental, special, or consequential damages arising out of the use of or inability to use the software (Apache 2.0, §8).

You are solely responsible for verifying the accuracy of any report generated by Luna against your source media before relying on it.

## Local Processing

Luna Web runs entirely in your browser. Your media never leaves your device: clips are read via the File System Access API and processed client-side (WebCodecs, WebAssembly). Luna does not upload footage, metadata, or reports to any server.

## Trademarks

Luna is an independent open-source project. It is **not affiliated with, endorsed by, sponsored by, or otherwise connected to** any of the following companies or their products:

- **ARRI** — ARRI ALEXA, ARRIRAW, ART, and the ARRI logo are trademarks of Arnold & Richter Cine Technik GmbH & Co. Betriebs KG.
- **Blackmagic Design** — Blackmagic, BRAW, Blackmagic RAW, URSA, and the Blackmagic Design logo are trademarks of Blackmagic Design Pty. Ltd.
- **Sony** — Sony, VENICE, CineAlta, and the Sony logo are trademarks of Sony Corporation.
- **Apple** — Apple, macOS, ProRes, and the Apple logo are trademarks of Apple Inc.

All other product names, logos, and brands are property of their respective owners. Use of these names, logos, and brands does not imply endorsement.

## No Vendor SDKs

Luna Web does not bundle, require, or integrate any proprietary camera-vendor SDK. Raw camera formats are handled with metadata-aware parsing of open container structures; generic codecs are decoded with open-source components (see [NOTICE](NOTICE)).

## Open-Source Components

Key redistributed components in the web bundle:

| Component | License | Notes |
| --- | --- | --- |
| FFmpeg (wasm, `@ffmpeg/ffmpeg` + `@ffmpeg/core`) | LGPL v2.1+ | Loaded at runtime; replaceable by the user |
| MediaInfo (`mediainfo.js`) | BSD-2-Clause | Container metadata |
| mediabunny (+ `@mediabunny/prores`) | MPL-2.0 | Demux/decode & thumbnails |
| `@react-pdf/renderer` | MIT | PDF report generation |
| React, TanStack, Base UI, Tailwind CSS, Astro | MIT | App & docs framework |
| Geist / Geist Mono | OFL 1.1 | Typography |

## The Retired Desktop App

The original .NET/Avalonia desktop app is preserved at branch `archive/dotnet` (tag `dotnet-final`). Existing installers remain downloadable from GitHub Releases but are unmaintained, receive no updates, and are unsigned — install at your own risk. Its own DISCLAIMER (covering vendor SDK integration, bundled FFmpeg DLLs, QuestPDF, and code signing) lives alongside that archived code.

## Reporting Issues

For bugs or security concerns, please open an issue on the [GitHub repository](https://github.com/shakedex/LunaApp/issues).
```

- [ ] **Step 4: Commit**

```bash
git add NOTICE DISCLAIMER.md
git commit -m "docs: rewrite NOTICE and DISCLAIMER for the web stack"
```

### Task 8: Outward-facing deprecation notices (requires Shaked's approval per action)

**Files:** none (GitHub state via `gh`)

**Interfaces:**
- Consumes: pushed `feature/luna-web` (or its merge to master) so links resolve.
- Produces: repo metadata pointing at the web app; latest release marked as final. **Nothing is deleted.**

- [ ] **Step 1: Push the branch (approval required)**

```bash
git push origin feature/luna-web
```

- [ ] **Step 2: Update repo description & homepage (approval required)**

```bash
gh repo edit shakedex/LunaApp --description "Camera reports in your browser. Nothing leaves your device." --homepage "https://luna.ozer2.one"
```

- [ ] **Step 3: Append a discontinuation note to the latest release (approval required)**

Fetch the current body, append — do not replace:

```bash
gh release view --json tagName,body --jq .tagName   # note the tag
gh release view --json body --jq .body > /tmp/release-body.md
cat >> /tmp/release-body.md <<'EOF'

---

> **⚠️ Final release.** The Luna desktop app is retired and will receive no
> further updates. Luna now lives in your browser: https://luna.ozer2.one
> The desktop source is preserved at the `archive/dotnet` branch.
EOF
gh release edit <tag-from-above> --notes-file /tmp/release-body.md
```

Expected: release page shows the appended note; installer assets untouched.

- [ ] **Step 4: Confirm nothing was deleted**

Run: `gh release list`
Expected: same release count as before this task.

### Task 9: Local disk cleanup (OPTIONAL — Shaked runs manually, not the agent)

**Files:** local-only, never tracked. ~3.7 GB reclaimable.

- [ ] **Step 1 (manual): Delete .NET build output and leftovers**

```powershell
Remove-Item -Recurse -Force bin, obj, publish, art.log
```

- [ ] **Step 2 (manual, only if no longer needed for reference): vendor SDKs & sample media**

Keep these if the web raw-format work still consults them; otherwise:

```powershell
Remove-Item -Recurse -Force docs\sdk, tools\arri, docs\old, docs\examples, docs\output-analysis
Remove-Item -Force docs\*.braw, docs\*.mxf, docs\*.mov, docs\B005C039_201101OZM01.xml
```

(The `docs/file_proccessing/` research and `docs/branding/` stay — they're tracked and useful.)

---

## Explicitly Out of Scope

- **History rewrite** (purging FFmpeg DLLs/PSD from old commits): not worth invalidating every clone and existing ref; the tree is clean going forward.
- **Deleting releases/tags/`master`**: never — Velopack-installed desktop apps poll GitHub Releases.
- **Renaming the `@luna-web/*` package scope** or the GitHub repo: pure churn, no benefit now; revisit if/when a Tauri/Electron app joins as `apps/desktop`.
- **Merging `feature/luna-web` → `master`**: happens through the normal flow when the web work is ready, not as part of this chore.
