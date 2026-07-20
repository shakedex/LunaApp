# Luna Desktop — Tauri Scaffold Design

**Date:** 2026-07-20
**Status:** Approved

## Goal

Add a Tauri v2 desktop shell to the Luna Web monorepo that runs the existing
`apps/web` frontend unchanged, plus a minimal runtime-detection seam so future
work can swap browser/WASM pipelines (mediainfo, ffmpeg) for native Rust
commands. Work happens on `master`.

## Direction (decided)

- **Shared frontend, native backend:** one frontend codebase (`apps/web`);
  the desktop app loads it and the frontend detects Tauri at runtime.
- **Layout:** new `apps/desktop` workspace holds only the Rust shell
  (`src-tauri`); its config points across the monorepo at `apps/web`.
- **Scope of this pass:** working shell + `isTauri`/`app_version` seam.
  No pipeline replacement yet.
- **Platforms:** Windows (NSIS) verified now; macOS bundle config set
  correctly now but untested until a mac build is wanted.

## Scaffolding method

`bunx @tauri-apps/cli init` run non-interactively with flags
(`--app-name`, `--window-title`, `--dev-url`, `--frontend-dist`,
`--before-dev-command`, `--before-build-command`, `--ci`). This is the
official CLI for adding Tauri to an existing frontend. Rejected:
`bun create tauri-app` (generates a throwaway frontend template) and
hand-writing `src-tauri` (drifts from current defaults, violates CLI-first).

## Files

New, all under `apps/desktop/`:

| File | Purpose |
| --- | --- |
| `package.json` | `@luna-web/desktop`; scripts `dev`/`build` → Tauri CLI; devDep `@tauri-apps/cli` (latest) |
| `src-tauri/Cargo.toml` | Rust crate manifest |
| `src-tauri/tauri.conf.json` | `devUrl: http://localhost:5173`; `frontendDist` → apps/web build output (`../../web/dist/client`, exact path verified during implementation); before-commands run apps/web bun scripts; product name + bundle identifier valid for Windows and macOS |
| `src-tauri/src/main.rs`, `src/lib.rs` | Entry point + one `#[tauri::command] app_version` returning the app version |
| `src-tauri/build.rs` | Generated Tauri build script |
| `src-tauri/capabilities/default.json` | Generated permission manifest |
| `src-tauri/icons/*` | Generated from `Assets/luna-logo.webp` via `tauri icon` (convert to PNG first if the CLI requires it) |
| `src-tauri/.gitignore` | Ignores `target/` |

New in `apps/web`:

- `src/lib/platform.ts` — `isTauri()` plus `getDesktopVersion(): Promise<string | null>`
  wrapping the `app_version` command; resolves `null` outside Tauri.

Changed:

- `apps/web/package.json` — adds `@tauri-apps/api` (latest)
- `bun.lock`

Untouched: root `package.json` (the `apps/*` glob already covers the new
workspace), vite config, Cloudflare deploy, `apps/docs`. No root-level Rust
workspace files; the crate is self-contained.

## Error handling

- `getDesktopVersion()` never throws on web: it checks `isTauri()` before
  invoking and returns `null`.
- Missing Rust toolchain (MSVC) is a hard prerequisite: implementation stops
  and hands Shaked the rustup install (interactive CLI is his to run).

## Testing / verification

- `bun run dev` in `apps/desktop` opens a window showing the live web app.
- Inside the window, `app_version` returns the desktop version string.
- `bun run build` in `apps/desktop` produces an NSIS installer.
- `apps/web` dev, build, and typecheck remain green; web bundle must not
  break when `@tauri-apps/api` is absent at runtime (browser).

## Out of scope

- Any native pipeline work (mediainfo/ffmpeg replacements)
- macOS/Linux builds and CI build matrix
- Auto-update, code signing, release integration with `bun run release`
