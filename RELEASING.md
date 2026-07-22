# Releasing Luna Web

Luna Web follows **[ZeroVer](https://0ver.org/)**: the major version stays `0` forever,
releases are `0.MINOR.PATCH` (MINOR for features, PATCH for fixes), and **there is never a
1.0**. The product version lives in [`apps/web/package.json`](apps/web/package.json) and
surfaces in the app header, the Credits page, and the PDF report footer stamp (via
`__APP_VERSION__`).

The changelog is hand-curated in
[`apps/docs/src/content/docs/changelog.md`](apps/docs/src/content/docs/changelog.md) using
the [Keep a Changelog](https://keepachangelog.com/en/1.1.0/) format, so it publishes to the
docs site at `/docs/changelog/`. The root [`CHANGELOG.md`](CHANGELOG.md) is just a pointer.

## Day to day

As you land user-facing work, add a bullet under `## [Unreleased]` in the docs changelog,
grouped as `### Added` / `### Changed` / `### Fixed` / `### Removed`. Write for the person
reading the report (a DIT), not for the commit log — concrete, plain, no internal framing.

## Cutting a release

From the repo root, with a clean working tree and at least one entry under `[Unreleased]`:

```bash
bun run release minor   # a new feature set
bun run release patch   # bug fixes only
```

The script:

1. Reads the current version from `apps/web/package.json` and computes the next one
   (`minor` → `0.(minor+1).0`, `patch` → `0.minor.(patch+1)`). It **refuses `major`** and
   anything that would reach `1.0.0`.
2. Writes the bumped version back to `apps/web/package.json`.
3. Promotes `## [Unreleased]` in the changelog to `## [x.y.z] - <today>`, leaves a fresh
   empty `[Unreleased]`, and updates the compare links.
4. Commits `chore(release): vx.y.z` (staging only those two files), creates the annotated
   tag `vx.y.z`, and **pushes** (`git push --follow-tags`).

That single command is the whole release. Preview without writing anything:

```bash
bun run release --dry-run minor
```

Cut a release but hold the push (e.g. to eyeball the commit first):

```bash
bun run release minor --no-push   # then: git push --follow-tags
```

## Deployment is automatic

Once the tag is pushed, Cloudflare Workers Builds rebuilds the app and docs from `master`
via the dashboard git integration (see [`DEPLOY.md`](DEPLOY.md)) — the new version and the
changelog go live on their own. Nothing in this repo runs `wrangler deploy`.

## Desktop releases

`apps/desktop` versions independently of the web app, also under ZeroVer.

```bash
bun run release:desktop <minor|patch> [--dry-run] [--no-push]
```

This bumps `apps/desktop/package.json`, `src-tauri/Cargo.toml`,
`src-tauri/tauri.conf.json`, and `src-tauri/Cargo.lock`, promotes
`apps/desktop/CHANGELOG.md`'s `[Unreleased]` section, commits, and tags
`desktop-v<version>` — a namespace kept distinct from the web app's `v<version>`.
Requires the Rust toolchain, since the lockfile refresh runs `cargo metadata`.

Pushing that tag triggers `.github/workflows/desktop-release.yml`, which builds on
`windows-latest` and `macos-latest` and attaches an NSIS installer and a universal
`.dmg` to a **draft** GitHub Release. Review the draft and publish it by hand.

Builds are unsigned, so both platforms warn on first launch; the release body
carries the workarounds.

## Guardrails

- `bun run release` aborts on a dirty working tree (so unrelated in-flight changes never
  get bundled into the release commit) and on an empty `[Unreleased]` (so a release always
  has a changelog entry).
- The ZeroVer rule is enforced in code, not by discipline: the script rejects `major`.
