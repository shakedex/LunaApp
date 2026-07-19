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
