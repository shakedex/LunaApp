# Luna Web

Client-side, Chromium-only, open-source web port of Luna's generic-codec camera-report
workflow. See `docs/superpowers/specs/2026-07-17-luna-web-design.md` for the design.

Bun workspace: `app` (the tool), `docs` (Starlight), `packages/core` (pure logic).

## Common commands
- `bun install` — install all workspace deps
- `bun --filter app dev` — run the app
- `bun --filter docs dev` — run the docs site
- `bun test` — run core unit tests
- `bun run lint` / `bun run typecheck` — quality gates
