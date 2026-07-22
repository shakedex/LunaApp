# Releasing Luna

Both the web app and the desktop shell follow **[ZeroVer](https://0ver.org/)**: the major
version stays `0` forever, releases are `0.MINOR.PATCH` (MINOR for features, PATCH for
fixes), and **there is never a 1.0**. They version independently, with separate tag
namespaces (`v<version>` for the web app, `desktop-v<version>` for desktop) — see
[Desktop releases](#desktop-releases) below.

The web app's version lives in [`apps/web/package.json`](apps/web/package.json) and
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

A `--no-push` desktop release leaves the tag committed but unpushed locally. A later
`bun run release` (web) pushes with `--follow-tags`, which pushes *every* pending
annotated tag — including that dangling desktop tag — and unexpectedly kicks off a
desktop build. If you used `--no-push` on purpose, push the desktop tag deliberately
(`git push --follow-tags`) before running the web release, or drop it.

### The `desktop-v0.1.0` trap

`desktop-v0.1.0` predates this release pipeline and was **never tagged**. Do not
retroactively create and push it. `.github/workflows/desktop-release.yml` triggers on
any `desktop-v*` push — creating and pushing that tag now would build a release from
whatever is on `master` at push time labeled as `0.1.0`, which is not what `0.1.0`
actually was and (before this fix pass) would have shipped a binary containing the
build machine's absolute filesystem path. If you want the tag purely for changelog-link
hygiene, create it **locally and never push it**:

```bash
git tag -a desktop-v0.1.0 <commit> -m "Luna Desktop v0.1.0"   # local only — do not push
```

## Guardrails

Both `bun run release` and `bun run release:desktop` abort on:

- a dirty working tree, so unrelated in-flight changes never get bundled into the
  release commit;
- an empty `[Unreleased]` section, so a release always has a changelog entry.

`bun run release:desktop` additionally refuses up front if its target tag already
exists, rather than committing and then failing on the tag step.

The ZeroVer rule is enforced in code, not by discipline: both scripts reject `major`.
