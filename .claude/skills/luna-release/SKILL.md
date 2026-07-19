---
name: luna-release
description: Use after landing user-facing work in Luna Web (a feature or a fix), and whenever deciding whether to cut a version release. Records the change in the changelog and judges whether a version bump is warranted, following ZeroVer. Do NOT invoke for pure chore/refactor/docs/test/CI work — those are invisible to users and get no changelog entry.
---

# Luna Release — keep the changelog and version current

You maintain Luna Web's changelog and version so the human maintainer never has to
remember to. Two responsibilities: **record user-facing changes as they land**, and
**judge when a release is worth cutting**. This skill is self-contained — follow it without
asking the maintainer to walk you through it.

## The version model — ZeroVer (non-negotiable)

- Versions are `0.MINOR.PATCH`. **The major stays `0` forever. There is never a 1.0.**
  Never propose, write, or pass a `1.x` or "stable" version.
- MINOR = a new user-facing feature or changed behavior. PATCH = a user-facing bug fix.
- Single source of truth: `apps/web/package.json` `version` (it feeds `__APP_VERSION__` →
  app header, Credits page, PDF footer stamp). `core`, `docs`, and the root stay `0.0.0`.

## Step 1 — Record the change in the changelog (do this when you finish user-facing work)

Canonical file: **`apps/docs/src/content/docs/changelog.md`** (publishes to
`/docs/changelog/`). Add a bullet under `## [Unreleased]`, in the right group:

- `### Added` — a new capability. `### Changed` — behavior/appearance of existing
  capability changed. `### Fixed` — a bug fix. `### Removed` — a capability taken away.

Create the group heading under `## [Unreleased]` if it isn't there yet.

### Only user-facing changes get an entry

If the change is invisible to someone using the app (refactor, test, CI, build, internal
docs, dependency bump with no behavior change), **write nothing** — skip this skill. The
changelog is for users, not the commit log.

### How to write the bullet

Write for a **DIT reading the report** — concrete, plain, present-tense. Say what the user
can now do or what got fixed. Not commit-speak, not internal justification, and **never
invent product/feature names** — use the plain terms the UI uses.

| Bad (why) | Good |
| --- | --- |
| `feat(core): summarizeReport derivation` (commit-speak, internal) | `- Save a finished report to your browser and reopen it later.` |
| `- Added first-class clip handling and honest totals` (invented framing) | `- RAW clips now count toward duration and size totals.` |
| `- Refactored the export pipeline` (invisible to users) | *(no entry — internal refactor)* |

Keep it to one line per change. If unsure whether it's user-facing, look at whether the
change alters something on screen, in an export (PDF/CSV), or in a saved report.

## Step 2 — Decide whether a release is worth cutting

Do this when the maintainer asks to release, or when you've just recorded a change and want
to judge if it's time. Look at `## [Unreleased]` and `git log $(git describe --tags --abbrev=0)..HEAD --oneline`, then:

1. **`[Unreleased]` is empty (or only trivial):** do **not** release. Nothing shipped for users.
2. **`[Unreleased]` has `Added`/`Changed` entries:** a **minor** release is warranted — a
   feature is ready.
3. **`[Unreleased]` has only `Fixed` entries:** a **patch** release is warranted.
4. **Timing judgment:** release when the accumulated entries form something worth shipping —
   a completed feature, or fixes users are waiting on. Don't cut a release after every
   commit (churny changelog); don't sit on shippable user value indefinitely.
5. **When genuinely unsure, don't release.** Keep `[Unreleased]` updated and tell the
   maintainer "a <minor|patch> release is available with N entries" — let them call it.

Never a major bump. If some input pushes toward `1.0`, refuse and restate ZeroVer.

## Step 3 — Cut the release

Preconditions: a clean working tree and a non-empty `[Unreleased]`. Run the gates first —
`bun run lint && bun run typecheck && bun test && bun run build` — then:

```bash
bun run release minor    # a feature set is ready
bun run release patch    # bug fixes only
```

That one command does everything: bumps `apps/web/package.json`, promotes `[Unreleased]` →
`## [x.y.z] - <today>` (leaving a fresh empty `[Unreleased]`), updates the compare links,
commits `chore(release): vx.y.z`, tags `vx.y.z`, and **pushes**. Cloudflare Workers Builds
then rebuilds `master` and the new version + changelog go live on their own.

- `bun run release --dry-run <minor|patch>` — preview the bump and the pending entries, no writes.
- `bun run release <minor|patch> --no-push` — cut it locally but hold the push.

The script is guarded: it refuses `major`, a dirty tree, and an empty `[Unreleased]`, so it
will stop you rather than cut a bad release.

## Checklist

- [ ] Change is user-facing? If not, stop — no changelog entry, no release.
- [ ] Added a one-line, user-facing bullet under the right `## [Unreleased]` group.
- [ ] Judged a release per Step 2 (or told the maintainer one is available).
- [ ] If releasing: gates green → `bun run release <minor|patch>`.

## Reference

- Human runbook: `RELEASING.md`. Release script: `tools/release.ts` (`tools/release.test.ts`).
- Root `CHANGELOG.md` is only a pointer to the docs changelog. Deploy details: `DEPLOY.md`.
