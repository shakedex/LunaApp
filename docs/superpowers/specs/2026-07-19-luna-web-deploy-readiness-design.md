# Luna Web — Deploy Readiness design (Cloudflare Workers via dashboard Git integration)

**Date:** 2026-07-19
**Status:** Proposed (awaiting maintainer review)
**Amends:** spec §18 (Build & deployment) of `2026-07-17-luna-web-design.md`

## 1. Goal and non-goals

Make the repo deployable through Cloudflare's **dashboard Git integration (Workers Builds)**.
The maintainer connects the GitHub repo to their Cloudflare account manually in the
dashboard; Cloudflare builds and deploys on push. **Nothing in this repo or its CI ever runs
`wrangler deploy`.**

Non-goals: actually deploying; DNS/zone work; docs content (milestone 9); CI changes
(`ci.yml` stays quality-gates-only).

Approved decisions (this session):

- Deploy-readiness only — dashboard-driven deploys, never wrangler CLI to remote.
- Production routes are **declared in the wrangler configs** (versioned), not clicked
  together in the dashboard.
- Docs serving uses the **nested-outDir** approach (Approach A below), keeping both Workers
  pure static-asset Workers with zero runtime code.

## 2. Architecture

Two assets-only Workers (no `main`, no Worker script), matching spec §18's two-Worker,
one-host layout:

| Worker | Config | Serves | Route |
|---|---|---|---|
| `luna-web` | `apps/web/wrangler.jsonc` | Vite SPA build (`apps/web/dist`) | `luna.ozer2.one/*` |
| `luna-web-docs` | `apps/docs/wrangler.jsonc` | Starlight build (`apps/docs/dist`) | `luna.ozer2.one/docs*` |

Cloudflare resolves the more specific `/docs*` route first, so the SPA never sees `/docs`
requests (unchanged from spec §18).

### 2.1 The `/docs` path problem and its fix (Approach A)

Astro's `base: '/docs'` prefixes URLs only — the build still emits `dist/index.html` at the
root. A static-assets Worker matches URL paths **literally** against the asset directory, so
`/docs/` would 404. Cloudflare's documented pattern for serving under a subpath ("Serving a
subdirectory", requires wrangler ≥ 3.98) is a directory structure that **mirrors the URL
path**. Fix: set Astro `outDir: './dist/docs'` so the build emits `dist/docs/**`, and point
the Worker's `assets.directory` at `./dist`. `/docs/guides/x/` then finds
`dist/docs/guides/x/index.html` by plain path matching. No runtime code.

Rejected alternatives: a `run_worker_first` prefix-stripping Worker script (runtime code and
edge cases for a one-line config fix); a single merged Worker (couples docs deploys to app
deploys, against spec §18).

### 2.2 App Worker (`apps/web/wrangler.jsonc`)

```jsonc
{
  "$schema": "node_modules/wrangler/config-schema.json",
  "name": "luna-web",
  "compatibility_date": "<recent date at plan time>",
  "assets": {
    "directory": "./dist",
    "not_found_handling": "single-page-application"
  },
  "routes": [{ "pattern": "luna.ozer2.one/*", "zone_name": "ozer2.one" }],
  "workers_dev": false,
  "preview_urls": true
}
```

- `single-page-application`: unknown paths fall back to `index.html`; the TanStack Router
  `notFoundComponent` (Plan 9) renders the 404 experience.
- `workers_dev: false` keeps the production site off `luna-web.<subdomain>.workers.dev`;
  `preview_urls: true` keeps preview URLs available for non-production branch builds
  (Workers Builds uploads versions for non-prod branches by default).
- Exact field set is finalized at plan time against the installed
  `node_modules/wrangler/config-schema.json`.

### 2.3 Vite integration (`@cloudflare/vite-plugin`)

`cloudflare()` is added to `apps/web/vite.config.ts` (existing plugins — tanstackRouter →
react → tailwindcss — keep their order; the Cloudflare plugin appends). The plugin reads
`wrangler.jsonc` as the *input* config and, at build, emits a flattened *output*
`wrangler.json` into `dist/` referencing the build artifacts — which is exactly what the
dashboard's default deploy command (`npx wrangler deploy`) auto-locates. Dev keeps working
through `vite` with Workers-runtime parity for asset serving.

**VERIFY at plan time:** plugin behavior with an assets-only config (no `main`) — dev server
boots, the app's module workers (mediainfo/mediabunny/ffmpeg) still function under `vite
dev`, and the emitted output config deploys the assets correctly (checked via `vite build`
output inspection + `wrangler dev` on the build).

### 2.4 Docs Worker (`apps/docs/wrangler.jsonc`)

```jsonc
{
  "$schema": "node_modules/wrangler/config-schema.json",
  "name": "luna-web-docs",
  "compatibility_date": "<recent date at plan time>",
  "assets": {
    "directory": "./dist",
    "not_found_handling": "404-page"
  },
  "routes": [{ "pattern": "luna.ozer2.one/docs*", "zone_name": "ozer2.one" }],
  "workers_dev": false,
  "preview_urls": true
}
```

Plus the one-line `astro.config.mjs` change: `outDir: './dist/docs'`.

No Vite plugin here — Astro owns its build; the Worker is a plain static mount. The
`@astrojs/cloudflare` adapter is *not* used (it targets SSR; docs are fully static).

**VERIFY at plan time:** Starlight emits `404.html` under `outDir` → `dist/docs/404.html`.
Confirm with `wrangler dev` whether `not_found_handling: "404-page"` resolves the nearest
`404.html` (nested location works) or only an assets-root one (then add a tiny root
`404.html` copy step to the docs build script).

## 3. Dependencies (vetted, added via `bun add -d`, latest)

| Package | Where | Why | Vetting |
|---|---|---|---|
| `wrangler` | `apps/web`, `apps/docs` | config validation, `wrangler dev` verification, dashboard deploy command resolves it per-Worker root | official Cloudflare, weekly releases, v4.x |
| `@cloudflare/vite-plugin` | `apps/web` | dev/build integration + output config emission | official Cloudflare, actively released alongside wrangler |

`wrangler` goes into **both** app packages because Workers Builds runs the deploy command in
each Worker's root directory — the binary must resolve there (hoisting would likely work,
but per-package is explicit and survives hoisting changes).

`.gitignore` gains `.wrangler/` (local dev state).

## 4. Dashboard handoff — `DEPLOY.md`

A short `DEPLOY.md` at the repo root documents the manual, maintainer-run dashboard setup —
two Workers Builds connections to the same repo:

| Setting | `luna-web` | `luna-web-docs` |
|---|---|---|
| Root directory | `apps/web` | `apps/docs` |
| Build command | `bun run build` | `bun run build` |
| Deploy command | `npx wrangler deploy` (default) | `npx wrangler deploy` (default) |
| Build watch paths | `apps/web/**`, `packages/core/**`, `bun.lock` | `apps/docs/**`, `bun.lock` |
| Branch | `master` | `master` |

Notes captured in the doc: Cloudflare auto-detects Bun from `bun.lock`; `bun install` run in
a workspace subdirectory installs the whole workspace (root lockfile), so `packages/core`
resolves; routes are applied automatically from the configs on first deploy (zone
`ozer2.one` must be in the account — it is); non-production branches run
`npx wrangler versions upload` → preview URLs; the auto-generated API token default is fine.

## 5. Error handling / edge cases

- **App deep links** (`/settings`, `/activity`, shared URLs): served `index.html` by SPA
  fallback; router resolves or renders `notFoundComponent`.
- **`/docs/<missing>`**: Starlight's 404 page with a real 404 status (see §2.4 VERIFY).
- **Asset size limits (free plan)**: largest shipped asset is the ~2.5 MB mediainfo wasm —
  far under the 25 MiB/asset limit; the ~31 MB ffmpeg core stays on jsDelivr + Cache API by
  design (spec §12) and is unaffected. File count ≪ 20k.
- **No COOP/COEP headers needed** (spec §5): no `_headers` file, nothing to configure.

## 6. Testing & QA

- Repo gates unchanged and must stay green: `bun run lint && bun run typecheck && bun test
  && bun run build` (no runtime code is added; `bun test` count unchanged).
- Local verification (agent-run): `bun run build`, then `wrangler dev` in each app dir —
  app: `/` loads, a deep link returns `index.html`; docs: `/docs/` serves the landing page,
  `/docs/<missing>` returns the 404 page, asset URLs under `/docs/_astro/*` resolve.
- Maintainer QA checklist (dashboard side, after connecting): both Workers build green in
  Workers Builds; `luna.ozer2.one` serves the app; `luna.ozer2.one/docs/` serves docs;
  routes visible on both Workers; a test branch push produces a preview URL.

## 7. Spec §18 amendment

Replace the CI/CD bullet ("GitHub Actions builds and deploys both Workers via Wrangler…")
with: deploys run through **Cloudflare Workers Builds** (dashboard Git integration), one
connection per Worker with per-Worker root directories; routing is config-as-code in the
two `wrangler.jsonc` files; GitHub Actions remains quality gates only. Also record the
nested `outDir: './dist/docs'` mechanism next to the existing `base: '/docs'` note.

## 8. Coordination

Files touched: `apps/web/wrangler.jsonc` (new), `apps/web/vite.config.ts`,
`apps/web/package.json`, `apps/docs/wrangler.jsonc` (new), `apps/docs/package.json`,
`apps/docs/astro.config.mjs`, `.gitignore`, `DEPLOY.md` (new), spec §18. Plan 10
(Settings v2 + Operations, mid-flight at Task 1) touches app/core source files — no overlap
expected; standard rules apply (git status before staging; stop on dirty shared files;
never touch `apps/web/src/components/ui/`, `tools/`, `docs/superpowers/backlog/`).
