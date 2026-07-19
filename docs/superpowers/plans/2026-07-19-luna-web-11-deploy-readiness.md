# Luna Web — Plan 11: Deploy Readiness (Cloudflare Workers) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make the repo deployable through Cloudflare's dashboard Git integration (Workers Builds): two assets-only Workers (`luna-web`, `luna-web-docs`) with config-as-code routes on `luna.ozer2.one`, the nested-outDir fix for `/docs`, the Cloudflare Vite plugin on the app, and a `DEPLOY.md` dashboard handoff. **No deploying happens in this plan.**

**Architecture:** Per the approved spec `docs/superpowers/specs/2026-07-19-luna-web-deploy-readiness-design.md`: both Workers are pure static-asset Workers (no `main`, zero runtime code). The app Worker uses `@cloudflare/vite-plugin`, which emits a flattened output `wrangler.json` into the build output that the dashboard's default deploy command (`npx wrangler deploy`) auto-locates. The docs Worker is a plain static mount whose asset tree mirrors the `/docs` URL path via Astro `outDir: './dist/docs'`.

**Tech Stack:** Existing toolchain + two new dev deps: `wrangler` (both apps) and `@cloudflare/vite-plugin` (app only), both official Cloudflare packages, installed at latest via `bun add -d`.

## Global Constraints

- **NEVER deploy.** `wrangler deploy`, `wrangler login`, `wrangler whoami`, or any command that talks to the Cloudflare API is FORBIDDEN. Verification uses only local commands: `bun run build`, `bunx vite preview`, `bunx wrangler dev` (local static-asset serving; needs no auth).
- **Never hand-write a dependency version.** Both new deps enter via `bun add -d` (installs latest). Record resolved versions in the task report.
- **Bun only.** Gates from the REPO ROOT before AND after every task: `bun run lint && bun run typecheck && bun test && bun run build` — all green. Test count must not decrease.
- **Never touch** `apps/web/src/components/ui/`, `tools/`, `docs/superpowers/backlog/`.
- **Stage by EXPLICIT file paths.** `git status` first; if a file you must modify has uncommitted maintainer changes, STOP and report. Plan 10 (Settings v2) is mid-flight on this branch — its files (app/core source) must not overlap with yours; if they do, STOP.
- TypeScript stays 6.0.3. `.gitattributes` pins LF. Environment: Windows, Git Bash tool, `cd /e/Coding/LunaApp` first (cwd drifts), bun 1.3.14, CRLF warnings benign.
- **VERIFY-DON'T-ASSUME:** the two spec VERIFY items (vite-plugin with assets-only config; nested `404.html` resolution) have explicit verification steps below with specified fallbacks. If reality contradicts a step, follow the fallback and report the deviation — never invent a third path silently.

---

### Task 1: App Worker readiness (`apps/web`) — wrangler config + Cloudflare Vite plugin

**Files:**
- Create: `apps/web/wrangler.jsonc`
- Modify: `apps/web/vite.config.ts`
- Modify: `apps/web/package.json` (via `bun add -d` only)
- Modify: `.gitignore` (repo root)
- Modify: `bun.lock` (via `bun add -d` only)

**Interfaces:**
- Consumes: existing `apps/web` build (`tsr generate && tsc -b --noEmit && vite build`).
- Produces (Task 3 relies on): Worker name **`luna-web`**; route `luna.ozer2.one/*` (zone `ozer2.one`); build via `bun run build`, deploy via default `npx wrangler deploy` run in `apps/web` (auto-locates the plugin's output `wrangler.json` in the build output).

- [ ] **Step 1: Coordination check**

Run: `cd /e/Coding/LunaApp && git status --short`
Expected: none of the files listed above appear modified. If `apps/web/vite.config.ts`, `apps/web/package.json`, or `.gitignore` show uncommitted changes, STOP and report BLOCKED.

- [ ] **Step 2: Add dev dependencies (latest, bun-resolved)**

Run: `cd /e/Coding/LunaApp/apps/web && bun add -d wrangler @cloudflare/vite-plugin`
Expected: both land in `apps/web/package.json` `devDependencies` with `^`-ranged latest versions; `bun.lock` updates. Record the resolved versions in your report. Requirement from the spec: wrangler must be **≥ 3.98** (it will be — current major is 4.x).

- [ ] **Step 3: Create `apps/web/wrangler.jsonc`**

```jsonc
{
  "$schema": "../../node_modules/wrangler/config-schema.json",
  "name": "luna-web",
  "compatibility_date": "2026-07-19",
  // Assets-only Worker: no `main`. The Cloudflare Vite plugin populates
  // `assets.directory` in the flattened output wrangler.json at build time.
  "assets": {
    "not_found_handling": "single-page-application"
  },
  // Config-as-code routing (approved decision). Applied by the dashboard's
  // deploy step; requires zone ozer2.one in the connected Cloudflare account.
  "routes": [{ "pattern": "luna.ozer2.one/*", "zone_name": "ozer2.one" }],
  // Keep production off workers.dev, but keep preview URLs for non-prod branches.
  "workers_dev": false,
  "preview_urls": true
}
```

If `$schema` resolution fails in your editor tooling, leave it — it is an editor nicety, not load-bearing.

- [ ] **Step 4: Wire the plugin into `apps/web/vite.config.ts`**

Full new file content (only the import and the plugins array entry change):

```ts
import { readFileSync } from 'node:fs'
import { fileURLToPath, URL } from 'node:url'
import { cloudflare } from '@cloudflare/vite-plugin'
import tailwindcss from '@tailwindcss/vite'
import { tanstackRouter } from '@tanstack/router-plugin/vite'
import react from '@vitejs/plugin-react'
import { defineConfig } from 'vite'

const pkg = JSON.parse(readFileSync(new URL('./package.json', import.meta.url), 'utf8')) as {
  version: string
}

export default defineConfig({
  plugins: [
    tanstackRouter({ target: 'react', autoCodeSplitting: true }),
    react(),
    tailwindcss(),
    cloudflare(),
  ],
  resolve: { alias: { '@': fileURLToPath(new URL('./src', import.meta.url)) } },
  define: { __APP_VERSION__: JSON.stringify(pkg.version) },
})
```

- [ ] **Step 5: Ignore wrangler local state**

In the repo-root `.gitignore`, extend the first block:

```
# Dependencies & build output
node_modules/
dist/
.astro/
.wrangler/
*.tsbuildinfo
*.log
```

(The only change is the added `.wrangler/` line.)

- [ ] **Step 6: Build and verify the plugin output (spec VERIFY item 1)**

Run: `cd /e/Coding/LunaApp/apps/web && bun run build`
Expected: build succeeds. Then inspect:

```bash
find dist -maxdepth 2 -name 'wrangler.json' -o -maxdepth 2 -name 'index.html' | sort
```

Expected: a generated `wrangler.json` somewhere under `dist/` and the client `index.html` (the plugin may nest client assets, e.g. `dist/client/`). Then:

```bash
cat "$(find dist -name 'wrangler.json' | head -1)"
```

Expected: `"name": "luna-web"`, a populated `"assets": { "directory": ... }` pointing at the client build output, `"not_found_handling": "single-page-application"`, and the `routes` carried through.

**FALLBACK (only if the build fails complaining the config needs `main` or `assets.directory`):** add `"directory": "./dist"` inside the `assets` block of `apps/web/wrangler.jsonc`, rebuild, re-verify. Consult `node_modules/@cloudflare/vite-plugin/README.md` for the assets-only shape if that still fails, apply the minimal compliant change, and report the deviation as DONE_WITH_CONCERNS.

- [ ] **Step 7: Verify SPA fallback locally**

```bash
cd /e/Coding/LunaApp/apps/web
(bunx vite preview --port 4173 >/tmp/vite-preview.log 2>&1 &) && sleep 5
curl -s -o /dev/null -w '%{http_code}\n' http://localhost:4173/
curl -s http://localhost:4173/settings | head -5
kill %1 2>/dev/null || pkill -f 'vite preview' || true
```

Expected: `200` for `/`; the `/settings` response is the SPA's `index.html` (contains `<div id="root">` / the Luna Web `<title>`), NOT a 404 — proving `single-page-application` fallback is live through the plugin's preview (which serves the built Worker in workerd). If `/settings` returns 404, report DONE_WITH_CONCERNS with the exact response — do not improvise a different serving path.

- [ ] **Step 8: Gates from repo root**

Run: `cd /e/Coding/LunaApp && bun run lint && bun run typecheck && bun test && bun run build`
Expected: all green, test count unchanged.

- [ ] **Step 9: Commit (explicit paths)**

```bash
cd /e/Coding/LunaApp && git status --short
git add apps/web/wrangler.jsonc apps/web/vite.config.ts apps/web/package.json .gitignore bun.lock
git commit -m "feat(web): app Worker deploy readiness - wrangler config + Cloudflare Vite plugin"
```

---

### Task 2: Docs Worker readiness (`apps/docs`) — nested outDir + wrangler config

**Files:**
- Create: `apps/docs/wrangler.jsonc`
- Modify: `apps/docs/astro.config.mjs`
- Modify: `apps/docs/package.json` (via `bun add -d`; possibly the `build` script — fallback only)
- Modify: `bun.lock` (via `bun add -d` only)

**Interfaces:**
- Consumes: existing Starlight scaffold (`base: '/docs'`, near-empty content — content is milestone 9, NOT this plan).
- Produces (Task 3 relies on): Worker name **`luna-web-docs`**; route `luna.ozer2.one/docs*` (zone `ozer2.one`); build via `bun run build`, deploy via default `npx wrangler deploy` run in `apps/docs` (reads `apps/docs/wrangler.jsonc` directly — no Vite plugin here).

- [ ] **Step 1: Coordination check**

Run: `cd /e/Coding/LunaApp && git status --short`
Expected: `apps/docs/astro.config.mjs` and `apps/docs/package.json` are clean. The maintainer recently touched `astro.config` (migrated to `.ts`, then back to `.mjs` — current committed truth is `.mjs`); if it shows uncommitted changes or an `astro.config.ts` reappears, STOP and report BLOCKED.

- [ ] **Step 2: Add wrangler dev dependency**

Run: `cd /e/Coding/LunaApp/apps/docs && bun add -d wrangler`
Expected: `wrangler` in `apps/docs/devDependencies` at the same latest version as Task 1; `bun.lock` updates.

- [ ] **Step 3: Nest the build output (the `/docs` fix)**

Full new `apps/docs/astro.config.mjs` (only the `outDir` line is added):

```js
import starlight from '@astrojs/starlight'
import { defineConfig } from 'astro/config'

export default defineConfig({
  site: 'https://luna.ozer2.one',
  base: '/docs',
  // Astro's `base` prefixes URLs only — it does NOT nest the emitted files.
  // The static-assets Worker matches URL paths literally against the asset
  // directory, so the tree must mirror the /docs path (Cloudflare's documented
  // "serving a subdirectory" pattern). outDir does the nesting.
  outDir: './dist/docs',
  integrations: [
    starlight({
      title: 'Luna Web',
      description: 'Camera reports in your browser. Nothing leaves your device.',
      social: [
        {
          icon: 'github',
          label: 'GitHub',
          href: 'https://github.com/shakedex/LunaApp'
        },
      ],
    }),
  ],
})
```

Preserve the file's existing formatting/quoting elsewhere byte-for-byte.

- [ ] **Step 4: Create `apps/docs/wrangler.jsonc`**

```jsonc
{
  "$schema": "../../node_modules/wrangler/config-schema.json",
  "name": "luna-web-docs",
  "compatibility_date": "2026-07-19",
  // Assets-only Worker: no `main`, no Vite plugin — Astro owns the build.
  // directory is ./dist (NOT ./dist/docs): the tree inside it mirrors the
  // /docs URL path, so luna.ozer2.one/docs/x resolves dist/docs/x literally.
  "assets": {
    "directory": "./dist",
    "not_found_handling": "404-page"
  },
  "routes": [{ "pattern": "luna.ozer2.one/docs*", "zone_name": "ozer2.one" }],
  "workers_dev": false,
  "preview_urls": true
}
```

- [ ] **Step 5: Build and verify the nesting**

Run: `cd /e/Coding/LunaApp/apps/docs && bun run build`
Expected: succeeds. Then:

```bash
ls dist            # expected: exactly one entry: docs/
ls dist/docs | head    # expected: index.html, 404.html, _astro/, favicon.svg, ...
test -f dist/docs/index.html && test -f dist/docs/404.html && echo NESTING-OK
```

Expected: `NESTING-OK`. If `404.html` is missing, report NEEDS_CONTEXT (Starlight generates a 404 route by default; its absence means something upstream changed — do not fabricate one).

- [ ] **Step 6: Serve locally and verify routing + 404 (spec VERIFY item 2)**

```bash
cd /e/Coding/LunaApp/apps/docs
(bunx wrangler dev --port 8788 >/tmp/wrangler-docs.log 2>&1 &) && sleep 8
curl -s -o /dev/null -w '%{http_code}\n' http://localhost:8788/docs/              # expect 200
ASSET="$(ls dist/docs/_astro | head -1)"
curl -s -o /dev/null -w '%{http_code}\n' "http://localhost:8788/docs/_astro/$ASSET"  # expect 200
curl -s -o /dev/null -w '%{http_code}\n' http://localhost:8788/docs/definitely-missing/  # expect 404
curl -s http://localhost:8788/docs/definitely-missing/ | head -20   # inspect the body
pkill -f 'wrangler dev' || true
```

Expected: `200`, `200`, `404` — and the 404 body is Starlight's styled 404 page (contains its markup), proving the nested `dist/docs/404.html` is resolved.

**FALLBACK (only if the 404 body is empty/null — nested 404.html NOT resolved):** Workers assets resolved the 404 page only at the assets root. Change the docs build script in `apps/docs/package.json` to also place a root copy:

```json
"build": "astro build && cp dist/docs/404.html dist/404.html"
```

Re-run Steps 5–6 (Step 5's `ls dist` expectation becomes `docs/` + `404.html`). Report the deviation as DONE_WITH_CONCERNS either way — which branch reality took is exactly what the spec wants recorded.

- [ ] **Step 7: Gates from repo root**

Run: `cd /e/Coding/LunaApp && bun run lint && bun run typecheck && bun test && bun run build`
Expected: all green. (`bun run build` runs the docs build with the new outDir — confirm it still passes at root.)

- [ ] **Step 8: Commit (explicit paths)**

```bash
cd /e/Coding/LunaApp && git status --short
git add apps/docs/wrangler.jsonc apps/docs/astro.config.mjs apps/docs/package.json bun.lock
git commit -m "feat(docs): docs Worker deploy readiness - nested /docs outDir + wrangler config"
```

---

### Task 3: `DEPLOY.md` handoff + spec §18 amendment

**Files:**
- Create: `DEPLOY.md` (repo root)
- Modify: `docs/superpowers/specs/2026-07-17-luna-web-design.md` (§18 only)

**Interfaces:**
- Consumes: Worker names/settings exactly as produced by Tasks 1–2 (`luna-web` / `luna-web-docs`, `bun run build`, default `npx wrangler deploy`, roots `apps/web` / `apps/docs`). If Task 2 took the root-404-copy fallback, its build command in this doc stays `bun run build` (the script change is internal) — no edits needed here.
- Produces: the maintainer-facing dashboard runbook; a truthful spec §18.

- [ ] **Step 1: Coordination check**

Run: `cd /e/Coding/LunaApp && git status --short`
Expected: the spec file is clean. STOP if not.

- [ ] **Step 2: Create `DEPLOY.md`**

```markdown
# Deploying Luna Web — Cloudflare Workers Builds (dashboard Git integration)

Two Workers, one repo, one host. Deploys run **only** through Cloudflare's dashboard
Git integration (Workers Builds). Nothing in this repo — and no CI job — ever runs
`wrangler deploy`; the configs under `apps/*/wrangler.jsonc` are the source of truth
for names, routes, and asset behavior, and the dashboard applies them on every push.

## Prerequisites

- The `ozer2.one` zone lives in the connected Cloudflare account (routes reference it
  by `zone_name`).
- The GitHub repo `shakedex/LunaApp` is authorized for the Cloudflare GitHub App.

## Worker settings (one Workers Builds connection per Worker)

In the dashboard: **Workers & Pages → Create → Workers → Import a repository**, pick
`shakedex/LunaApp`, then configure each Worker:

| Setting | `luna-web` (app) | `luna-web-docs` (docs) |
|---|---|---|
| Root directory | `apps/web` | `apps/docs` |
| Build command | `bun run build` | `bun run build` |
| Deploy command | `npx wrangler deploy` (default) | `npx wrangler deploy` (default) |
| Non-production branch deploy | `npx wrangler versions upload` (default) | `npx wrangler versions upload` (default) |
| Build watch paths | `apps/web/**`, `packages/core/**`, `bun.lock` | `apps/docs/**`, `bun.lock` |
| Production branch | `master` | `master` |
| API token | auto-generated default | auto-generated default |

## How it fits together

- **Bun is auto-detected** from `bun.lock`; `bun install` run inside a workspace
  directory installs the whole workspace, so `@luna-web/core` resolves for the app
  build.
- **App Worker**: the Cloudflare Vite plugin emits a flattened `wrangler.json` into
  the build output; the default `npx wrangler deploy` auto-locates it. SPA fallback
  (`single-page-application`) serves `index.html` for deep links; the router's
  not-found page handles the rest.
- **Docs Worker**: plain static assets from `apps/docs/dist`, whose tree is nested as
  `dist/docs/**` (Astro `outDir`) to mirror the `/docs` URL path.
- **Routing**: declared in the configs — `luna.ozer2.one/docs*` → `luna-web-docs`,
  `luna.ozer2.one/*` → `luna-web`. Cloudflare matches the more specific `/docs*`
  route first, so the SPA never sees `/docs` requests.
- **Preview URLs**: pushes to non-production branches upload a version and get a
  preview URL (`preview_urls: true`); the production site stays off `workers.dev`
  (`workers_dev: false`).
- **The ~31 MB ffmpeg core is not deployed** — it rides jsDelivr + the Cache API at
  runtime (see spec §12). The largest deployed asset (~2.5 MB mediainfo wasm) is far
  under the 25 MiB/file Workers limit.

## Local verification (no deploy)

```bash
# App: build + preview the built Worker (SPA fallback live)
cd apps/web && bun run build && bunx vite preview

# Docs: build + serve the static Worker locally
cd apps/docs && bun run build && bunx wrangler dev
# then check /docs/ and /docs/<missing> (styled 404)
```

## Versioning (ZeroVer)

Luna Web follows **ZeroVer (0ver)**: the major version stays 0 permanently; releases
are `0.MINOR.PATCH` (MINOR for features, PATCH for fixes). A release is a bump of
`apps/web/package.json` `version` — it surfaces in the app header
(`__APP_VERSION__`) and the PDF footer stamp. There will never be a 1.0.

## First-deploy checklist

1. Connect both Workers per the table above; push (or "Retry build") on `master`.
2. Both builds green in Workers Builds.
3. `https://luna.ozer2.one` serves the app; a deep link (e.g. `/settings`) loads.
4. `https://luna.ozer2.one/docs/` serves the docs; `/docs/<garbage>` shows the
   styled 404.
5. Both Workers show their route in the dashboard (applied from config).
6. Push a throwaway branch → preview URL appears on the build.
```

- [ ] **Step 3: Amend spec §18**

In `docs/superpowers/specs/2026-07-17-luna-web-design.md` §18, make exactly four edits (verify the current text matches before editing; if it drifted, STOP and report):

Edit 1 — the App bullet:

```
OLD:
- **App**: Vite build → static assets → Cloudflare Worker via `wrangler.toml` `assets`
  binding. No special headers needed (no cross-origin isolation).
NEW:
- **App**: Vite build (with `@cloudflare/vite-plugin`) → static assets → Cloudflare
  Worker configured by `apps/web/wrangler.jsonc` (assets-only, SPA fallback). No
  special headers needed (no cross-origin isolation).
```

Edit 2 — the Docs bullet:

```
OLD:
- **Docs**: Astro Starlight build → static → a **separate** Cloudflare Worker, built with
  Astro `base: '/docs'` so all asset URLs and internal links are `/docs`-prefixed.
NEW:
- **Docs**: Astro Starlight build → static → a **separate** Cloudflare Worker
  (`apps/docs/wrangler.jsonc`), built with Astro `base: '/docs'` so URLs are
  `/docs`-prefixed AND `outDir: './dist/docs'` so the emitted tree mirrors the
  `/docs` path (static-assets Workers match URL paths literally against the asset
  directory).
NEW (only if Task 2 took the root-404-copy fallback — append to the bullet):
  The docs build also copies `404.html` to the assets root for `not_found_handling`.
```

Edit 3 — the CI/CD bullet:

```
OLD:
- **CI/CD**: GitHub Actions builds and deploys both Workers via Wrangler on merge to the
  branch's mainline.
NEW:
- **Deploys**: Cloudflare **Workers Builds** (dashboard Git integration) — one
  connection per Worker with per-Worker root directories (`apps/web`, `apps/docs`);
  routing is config-as-code in the two `wrangler.jsonc` files. Nothing in the repo or
  CI runs `wrangler deploy`; GitHub Actions remains quality gates only. See
  `DEPLOY.md` for the exact dashboard settings.
```

Edit 4 — append a new bullet at the end of the §18 bullet list (after the Deploys
bullet inserted by Edit 3):

```
NEW (appended bullet):
- **Versioning**: **ZeroVer (0ver)** — the major version stays 0 permanently;
  releases are `0.MINOR.PATCH` (MINOR for features, PATCH for fixes). A release is a
  bump of `apps/web/package.json` `version`, surfaced in the app header
  (`__APP_VERSION__`) and the PDF footer stamp. There will never be a 1.0.
```

- [ ] **Step 4: Gates from repo root**

Run: `cd /e/Coding/LunaApp && bun run lint && bun run typecheck && bun test && bun run build`
Expected: all green (docs/markdown changes only — this is a regression check).

- [ ] **Step 5: Commit (explicit paths)**

```bash
cd /e/Coding/LunaApp && git status --short
git add DEPLOY.md docs/superpowers/specs/2026-07-17-luna-web-design.md
git commit -m "docs: DEPLOY.md dashboard runbook + spec §18 Workers Builds amendment"
```

---

## Definition of done

- All four gates green from repo root after every task; test count unchanged; zero changes under `apps/web/src/components/ui/`, `tools/`, `docs/superpowers/backlog/`; no Plan-10 file overlap.
- Both spec VERIFY items resolved with the branch reality took recorded in the ledger (plugin assets-only behavior; nested vs root 404 resolution).
- No Cloudflare API contact of any kind occurred.
- Final whole-plan review (opus), fixes + re-review, ledger close.

### Maintainer QA checklist (manual)

1. **Dashboard connect** (your side, per `DEPLOY.md`): both Workers import, build green, deploy applies the routes from config.
2. `luna.ozer2.one` serves the app; deep link `/settings` loads (SPA fallback); garbage URL shows the in-app 404 page.
3. `luna.ozer2.one/docs/` serves the Starlight landing page; `/docs/<garbage>` shows the styled 404 with a real 404 status.
4. App still fully works when served by the Worker (scan → process → export) — nothing about the runtime changed, this is a smoke pass.
5. A throwaway branch push produces a preview URL; `master` stays the only production deploy.
6. Local dev unchanged: `bun run dev` in `apps/web` still boots and hot-reloads normally with the Cloudflare plugin active.

## Self-review notes

- Spec coverage: §2.2 app config + §2.3 plugin (T1), §2.1/§2.4 docs nesting + config (T2), §3 deps (T1/T2, bun-resolved), §4 DEPLOY.md + §7 spec amendment (T3), §5 edge cases encoded as T1 Step 7 / T2 Step 6 verifications, §6 local verification embedded per task. Both spec VERIFY items have explicit steps + fallbacks (T1 S6, T2 S6). Maintainer-dictated additions folded in post-review: ZeroVer versioning policy (T3: DEPLOY.md section + spec §18 Edit 4); single-Worker consolidation was proposed, pushed back on, and REJECTED by the maintainer — two Workers stand.
- Type/name consistency: `luna-web` / `luna-web-docs` identical across T1/T2 configs, T3 DEPLOY.md, and QA; ports 4173/8788 used once each; `compatibility_date` 2026-07-19 in both configs.
- Deliberate choices: `assets.directory` omitted in the app input config (plugin populates it — documented Cloudflare behavior) with an in-task fallback; `$schema` uses the hoisted root `node_modules` path (editor nicety only); docs Worker deliberately has no Vite plugin; DEPLOY.md lives at repo root (visible next to README, outside the gitignored `docs/*` carve-out — `DEPLOY.md` is not under `docs/`).
