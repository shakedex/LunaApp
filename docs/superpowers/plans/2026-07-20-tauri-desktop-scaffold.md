# Tauri Desktop Scaffold Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a Tauri v2 desktop shell (`apps/desktop`) that runs the existing `apps/web` frontend, plus an `isTauri`/`app_version` runtime seam in the web app.

**Architecture:** `apps/desktop` holds only the Rust shell (`src-tauri`); its `tauri.conf.json` points at apps/web's Vite dev server (dev) and `apps/web/dist` (build). The frontend gains one module (`src/lib/platform.ts`) that detects Tauri at runtime and wraps one proof-of-life Rust command.

**Tech Stack:** Tauri 2 (`@tauri-apps/cli`, `@tauri-apps/api`, latest via `bun add`), Rust (stable-msvc), bun workspaces, Vite/React (existing, untouched).

**Spec:** `docs/superpowers/specs/2026-07-20-tauri-desktop-scaffold-design.md`

## Global Constraints

- Stay on `master`. Commit after every task.
- bun-only tooling; deps installed with `bun add` at latest (no version pins in commands).
- Repo root `package.json` must NOT change.
- The desktop package must NOT define scripts named `build` or `typecheck` — root runs `bun --filter '*' build|typecheck` and would invoke cargo on every web build. The installer script is named `bundle`.
- ZeroVer: desktop version starts at `0.1.0`; never propose 1.0.
- Rust toolchain is NOT installed on this machine (verified 2026-07-20). Tasks 1–4 need no Rust. Task 5 gates on it: STOP and hand the interactive rustup install to Shaked — never install it yourself.
- All shell commands below are Git Bash syntax with repo root `E:/Coding/LunaApp` unless a `cd` says otherwise.

---

### Task 1: Platform seam in apps/web

**Files:**
- Create: `apps/web/src/lib/platform.ts`
- Test: `apps/web/src/lib/platform.test.ts`
- Modify: `apps/web/package.json` (dependency added by `bun add`)

**Interfaces:**
- Consumes: nothing from other tasks. The Rust command name `app_version` must match Task 3's `#[tauri::command] fn app_version`.
- Produces: `isTauri(): boolean` and `getDesktopVersion(): Promise<string | null>` from `@/lib/platform` — the seam future native pipelines build on.

- [ ] **Step 1: Add the dependency**

```bash
cd E:/Coding/LunaApp/apps/web && bun add @tauri-apps/api
```

Expected: `@tauri-apps/api` appears in `apps/web/package.json` dependencies at the current latest `^2.x`; `bun.lock` updates.

- [ ] **Step 2: Write the failing test**

Create `apps/web/src/lib/platform.test.ts`:

```ts
import { expect, test } from 'bun:test'
import { getDesktopVersion, isTauri } from './platform'

test('isTauri is false outside a Tauri webview', () => {
  expect(isTauri()).toBe(false)
})

test('getDesktopVersion resolves null outside a Tauri webview', async () => {
  expect(await getDesktopVersion()).toBeNull()
})
```

- [ ] **Step 3: Run test to verify it fails**

```bash
cd E:/Coding/LunaApp/apps/web && bun test src/lib/platform.test.ts
```

Expected: FAIL — cannot resolve `./platform`.

- [ ] **Step 4: Write the implementation**

Create `apps/web/src/lib/platform.ts`:

```ts
import { invoke } from '@tauri-apps/api/core'

/** True when running inside the Luna desktop (Tauri) shell. */
export function isTauri(): boolean {
  return typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window
}

/** Desktop shell version, or null in the browser. */
export async function getDesktopVersion(): Promise<string | null> {
  if (!isTauri()) return null
  return invoke<string>('app_version')
}
```

- [ ] **Step 5: Run test to verify it passes**

```bash
cd E:/Coding/LunaApp/apps/web && bun test src/lib/platform.test.ts
```

Expected: 2 pass. If the import of `@tauri-apps/api/core` itself throws under bun test (top-level `window` access), report it — do not work around it silently.

- [ ] **Step 6: Web app still typechecks**

```bash
cd E:/Coding/LunaApp/apps/web && bun run typecheck
```

Expected: exit 0.

- [ ] **Step 7: Commit**

```bash
cd E:/Coding/LunaApp && git add apps/web/src/lib/platform.ts apps/web/src/lib/platform.test.ts apps/web/package.json bun.lock && git commit -m "feat(web): tauri runtime detection seam"
```

---

### Task 2: Scaffold apps/desktop via tauri init

**Files:**
- Create: `apps/desktop/package.json`
- Create (generated): `apps/desktop/src-tauri/*` (`Cargo.toml`, `build.rs`, `tauri.conf.json`, `.gitignore`, `src/main.rs`, `src/lib.rs`, `capabilities/default.json`, `icons/*`)
- Modify: `bun.lock`

**Interfaces:**
- Consumes: nothing.
- Produces: the `src-tauri` project Task 3 configures; package scripts `dev` (`tauri dev`), `bundle` (`tauri build`), `tauri` (raw CLI).

- [ ] **Step 1: Create the workspace package**

Create `apps/desktop/package.json`:

```json
{
  "name": "@luna-web/desktop",
  "private": true,
  "version": "0.1.0",
  "type": "module",
  "scripts": {
    "dev": "tauri dev",
    "bundle": "tauri build",
    "tauri": "tauri"
  }
}
```

- [ ] **Step 2: Install the Tauri CLI**

```bash
cd E:/Coding/LunaApp/apps/desktop && bun add -d @tauri-apps/cli
```

Expected: devDependency at current latest `^2.x`; `bun.lock` updates; bun links the workspace.

- [ ] **Step 3: Run tauri init non-interactively**

```bash
cd E:/Coding/LunaApp/apps/desktop && bun run tauri init --ci \
  --app-name luna-desktop \
  --window-title "Luna" \
  --frontend-dist ../../web/dist \
  --dev-url http://localhost:5173 \
  --before-dev-command "bun run --cwd ../web dev" \
  --before-build-command "bun run --cwd ../web build"
```

Expected: `apps/desktop/src-tauri/` created containing `Cargo.toml`, `build.rs`, `tauri.conf.json`, `src/main.rs`, `src/lib.rs`, `capabilities/default.json`, `icons/` (default Tauri icons), and a `.gitignore` covering `/target`. (`--frontend-dist` is relative to `src-tauri/`, so `../../web/dist` = `apps/web/dist` — already verified as the web build output, `index.html` at its root.)

- [ ] **Step 4: Verify the generated file set**

```bash
cd E:/Coding/LunaApp && git status --short apps/desktop && ls apps/desktop/src-tauri
```

Expected: only files under `apps/desktop/`. No root-level files created or modified (besides `bun.lock` from Step 2).

- [ ] **Step 5: Normalize formatting for biome**

```bash
cd E:/Coding/LunaApp && bunx biome check --write apps/desktop && bun run lint
```

Expected: lint exits 0 (generated JSON reformatted if needed; Rust files are ignored by biome).

- [ ] **Step 6: Root pipelines unaffected**

```bash
cd E:/Coding/LunaApp && bun run typecheck
```

Expected: exit 0 — web and docs typecheck; desktop (no `typecheck` script) is skipped. If bun instead errors on the missing script, add `"typecheck": "echo skip"` to `apps/desktop/package.json` scripts and rerun.

- [ ] **Step 7: Commit**

```bash
cd E:/Coding/LunaApp && git add apps/desktop bun.lock && git commit -m "feat(desktop): scaffold tauri shell via tauri init"
```

---

### Task 3: Configure the shell and add the app_version command

**Files:**
- Modify: `apps/desktop/src-tauri/tauri.conf.json` (full replacement below)
- Modify: `apps/desktop/src-tauri/src/lib.rs` (full replacement below)

**Interfaces:**
- Consumes: the generated scaffold from Task 2. `src/main.rs` stays as generated (it calls the lib's `run()`).
- Produces: Rust command `app_version` (returns the `version` from `tauri.conf.json` as a string) — invoked by Task 1's `getDesktopVersion()`; window label stays `main` (matches `capabilities/default.json`).

- [ ] **Step 1: Replace tauri.conf.json**

Write `apps/desktop/src-tauri/tauri.conf.json` (keep the generated `$schema` line if it differs):

```json
{
  "$schema": "https://schema.tauri.app/config/2",
  "productName": "Luna",
  "version": "0.1.0",
  "identifier": "com.lunaweb.desktop",
  "build": {
    "beforeDevCommand": "bun run --cwd ../web dev",
    "devUrl": "http://localhost:5173",
    "beforeBuildCommand": "bun run --cwd ../web build",
    "frontendDist": "../../web/dist"
  },
  "app": {
    "windows": [
      {
        "title": "Luna",
        "width": 1440,
        "height": 900,
        "minWidth": 1024,
        "minHeight": 700
      }
    ],
    "security": {
      "csp": null
    }
  },
  "bundle": {
    "active": true,
    "targets": ["nsis", "app", "dmg"],
    "icon": [
      "icons/32x32.png",
      "icons/128x128.png",
      "icons/128x128@2x.png",
      "icons/icon.icns",
      "icons/icon.ico"
    ]
  }
}
```

(Windows bundles NSIS; `app`/`dmg` are macOS targets, skipped on Windows — present so a later mac build needs no config change. Identifier is a pre-release placeholder; revisit before any signed release.)

- [ ] **Step 2: Replace src/lib.rs**

Write `apps/desktop/src-tauri/src/lib.rs`:

```rust
#[tauri::command]
fn app_version(app: tauri::AppHandle) -> String {
  app.package_info().version.to_string()
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
  tauri::Builder::default()
    .invoke_handler(tauri::generate_handler![app_version])
    .run(tauri::generate_context!())
    .expect("error while running tauri application");
}
```

Keep the crate/lib names exactly as `tauri init` generated them in `Cargo.toml` and `src/main.rs` — do not rename.

- [ ] **Step 3: Lint check**

```bash
cd E:/Coding/LunaApp && bun run lint
```

Expected: exit 0. (Rust compilation is verified in Task 5 once the toolchain exists.)

- [ ] **Step 4: Commit**

```bash
cd E:/Coding/LunaApp && git add apps/desktop/src-tauri && git commit -m "feat(desktop): point shell at apps/web and add app_version command"
```

---

### Task 4: App icons from the Luna logo

**Files:**
- Modify: `apps/desktop/src-tauri/icons/*` (regenerated)

**Interfaces:**
- Consumes: `Assets/luna-logo-lg.webp` (317×333 — will be padded square and upscaled), `Assets/luna-logo.ico`, `Assets/luna-logo.icns`.
- Produces: the icon files `tauri.conf.json`'s `bundle.icon` list references.

- [ ] **Step 1: Convert the logo to a square 1024px PNG**

```bash
cd E:/Coding/LunaApp && bunx --yes sharp-cli -i Assets/luna-logo-lg.webp -o "$TEMP/luna-icon-1024.png" resize 1024 1024 --fit contain --background "rgba(0,0,0,0)"
```

Expected: `$TEMP/luna-icon-1024.png` exists, 1024×1024 with transparent padding. If sharp-cli's flags reject this form, run `bunx sharp-cli --help` and adjust only the flag spelling — the operation (contain-fit resize to 1024² with transparent background, PNG out) is fixed.

- [ ] **Step 2: Generate the icon set**

```bash
cd E:/Coding/LunaApp/apps/desktop && bun run tauri icon "$TEMP/luna-icon-1024.png"
```

Expected: `src-tauri/icons/` repopulated (32x32.png, 128x128.png, 128x128@2x.png, icon.icns, icon.ico, Square*.png, icon.png).

- [ ] **Step 3: Use the hand-made ico/icns for Windows and macOS**

The upscaled PNGs are fine for taskbar sizes, but the repo already has purpose-made desktop icons — use them for the two files that matter most:

```bash
cd E:/Coding/LunaApp && cp Assets/luna-logo.ico apps/desktop/src-tauri/icons/icon.ico && cp Assets/luna-logo.icns apps/desktop/src-tauri/icons/icon.icns
```

- [ ] **Step 4: Commit**

```bash
cd E:/Coding/LunaApp && git add apps/desktop/src-tauri/icons && git commit -m "feat(desktop): luna app icons"
```

---

### Task 5: Rust toolchain gate and dev-run verification

**Files:**
- Create (generated on first build): `apps/desktop/src-tauri/Cargo.lock`

**Interfaces:**
- Consumes: everything from Tasks 1–4.
- Produces: a verified-running shell; `Cargo.lock` committed.

- [ ] **Step 1: Toolchain gate**

```bash
cargo --version && rustc --version
```

Expected: both print versions. **If either is missing: STOP.** Tell Shaked: install Rust via `winget install Rustup.Rustup` (or https://rustup.rs), accept the default `stable-msvc` toolchain — Visual Studio 2022 is already installed, but rustup-init will say if the "Desktop development with C++" workload is missing. Then reopen the terminal and resume here. Do NOT install it yourself; the installer is interactive and Shaked runs interactive CLIs.

- [ ] **Step 2: Compile check**

```bash
cd E:/Coding/LunaApp/apps/desktop/src-tauri && cargo check
```

Expected: exit 0 after dependency download (several minutes first run). Fix any compile error in `lib.rs` before proceeding.

- [ ] **Step 3: Run the app in dev**

Ensure nothing else is on port 5173, then:

```bash
cd E:/Coding/LunaApp/apps/desktop && bun run dev
```

Expected: Vite starts (`http://localhost:5173`), a native window titled **Luna** opens showing the Luna Web UI.

- [ ] **Step 4: Verify the app_version command inside the window**

In the app window: right-click → Inspect → Console (devtools are enabled in dev builds), then run:

```js
await window.__TAURI_INTERNALS__.invoke('app_version')
```

Expected: `"0.1.0"`. This proves the frontend↔Rust bridge; `getDesktopVersion()` wraps this same invoke. (If driving this agentically, the native-devtools MCP tools can screenshot/inspect the window; otherwise ask Shaked to run the console line.)

- [ ] **Step 5: Stop dev, commit Cargo.lock**

Close the window / Ctrl-C the dev process.

```bash
cd E:/Coding/LunaApp && git add apps/desktop/src-tauri/Cargo.lock && git commit -m "chore(desktop): commit cargo lockfile"
```

---

### Task 6: Installer build and regression sweep

**Files:**
- None new (build outputs are gitignored).

**Interfaces:**
- Consumes: everything prior.
- Produces: verified NSIS installer + green repo pipelines. Scaffold complete.

- [ ] **Step 1: Build the installer**

```bash
cd E:/Coding/LunaApp/apps/desktop && bun run bundle
```

Expected: apps/web builds first (beforeBuildCommand), then a release compile (long first run), ending with an installer at `apps/desktop/src-tauri/target/release/bundle/nsis/Luna_0.1.0_x64-setup.exe`. Confirm the file exists. Do NOT run the installer.

- [ ] **Step 2: Repo-wide regression sweep**

```bash
cd E:/Coding/LunaApp && bun run lint && bun run typecheck && bun test && bun run build
```

Expected: all exit 0; `bun test` includes the two platform tests from Task 1; root `build` does NOT trigger cargo (desktop has no `build` script).

- [ ] **Step 3: Commit any stragglers**

```bash
cd E:/Coding/LunaApp && git status --short
```

Expected: clean except pre-existing unrelated changes (`tools/analysis/FINDINGS.md`, `docs/superpowers/backlog/`). If scaffold-related files remain, add and commit them with `chore(desktop): scaffold follow-ups`.
