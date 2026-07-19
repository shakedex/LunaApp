# Luna Web — ZeroVer Workflow + Changelog Design

**Date:** 2026-07-20
**Status:** Approved (brainstorming) — pending spec review
**Depends on:** existing ZeroVer policy in `DEPLOY.md` §"Versioning (ZeroVer)" and design spec §18–19.

## 1. Motivation

Luna Web already *declares* a ZeroVer policy (`DEPLOY.md`, spec §18) and anticipates a
"Docs changelog page in Starlight" (spec §19), but nothing implements either: every
package is still `0.0.0`, there is no changelog, and there is no repeatable way to cut a
release. This design turns the written policy into a working, low-ceremony workflow:

- a single product version that surfaces in the app and PDF reports,
- a hand-curated, user-facing changelog that publishes to the docs site,
- one command to cut a release (bump + stamp changelog + tag), guarded so ZeroVer can't
  be violated.

## 2. Decisions (locked during brainstorming)

| Decision | Choice |
| --- | --- |
| Version source of truth | `apps/web/package.json` `version` (already read by `__APP_VERSION__`) |
| Starting version | **`0.11.0`** — one MINOR per shipped milestone (01–11) |
| Baseline tag | Retroactive `v0.11.0` at commit `6d41b18` (last commit before any milestone-12 work) |
| Changelog format | Keep a Changelog 1.1.0, hand-curated, plain user-facing language |
| Canonical changelog file | `apps/docs/src/content/docs/changelog.md` (a Starlight page) |
| Root `CHANGELOG.md` | Thin pointer to the canonical file + online URL |
| Release mechanism | `bun run release <minor\|patch>` (`tools/release.ts`) + `RELEASING.md` |
| Auto-generation | **None** — no git-cliff/conventional-changelog/changesets |

### Why `0.11.0` and not `0.12.0`

Verified against the tree on 2026-07-20: milestone 12 (Report Library) is **not
complete**. Tasks 1–3 have landed (`db41cf2`, `69a785e`, `9c7bc42` — core summary,
idb-v4 persistence, shared `ReportView` + Save button) but Task 4 (`/reports` list route)
and Task 5 (saved-report read-only view) have not. The navigable Report Library does not
yet exist, so the current shipped state is `0.11.0`; the report-library work is
`[Unreleased]` and ships as `0.12.0` when Tasks 4–5 land and `bun run release minor` runs.

The baseline tag anchors to the fixed historical commit `6d41b18`, so it is unaffected by
the maintainer continuing to commit milestone-12 work on `master` in parallel.
`git log v0.11.0..HEAD` therefore resolves to exactly the unreleased commits.

## 3. Component design

### 3.1 Version source of truth

`apps/web/package.json` `version` is the one product version. `packages/core`,
`apps/docs`, and the workspace root stay unversioned / `0.0.0` — they are internal and
never published to npm. No re-plumbing: `apps/web/vite.config.ts` already injects
`__APP_VERSION__` from this field into the app header, Credits page, and PDF footer stamp.

### 3.2 Baseline: adopt `0.11.0`

- Set `apps/web/package.json` `version` → `0.11.0`.
- Create annotated tag `v0.11.0` at `6d41b18` (`git tag -a v0.11.0 6d41b18 -m "v0.11.0"`).
- We do **not** fabricate `0.1.0`–`0.10.0` dated releases; they never happened. `0.11.0`
  is the first *tracked* release, and its changelog entry consolidates milestones 01–11.

### 3.3 Changelog — canonical file lives in docs

**File:** `apps/docs/src/content/docs/changelog.md` — a Starlight content page.

Structure (Keep a Changelog 1.1.0), with the H1 omitted (Starlight renders the title from
frontmatter):

```markdown
---
title: Changelog
description: Notable changes to Luna Web, newest first.
---

All notable changes to Luna Web. This format is based on
[Keep a Changelog](https://keepachangelog.com); Luna Web follows
[ZeroVer](https://0ver.org) — versions are `0.MINOR.PATCH` and there will never be a 1.0.

## [Unreleased]

## [0.11.0] - 2026-07-20

### Added
- <consolidated, user-facing summary of the app as shipped through milestone 11:
  folder scanning, metadata extraction, thumbnails, reels & report workspace,
  PDF/CSV export, RAW clips & embedded previews, persisted settings, activity log,
  Cloudflare deploy readiness — written for a DIT, grouped Added/Changed as appropriate>

[unreleased]: https://github.com/shakedex/LunaApp/compare/v0.11.0...HEAD
[0.11.0]: https://github.com/shakedex/LunaApp/releases/tag/v0.11.0
```

- `[Unreleased]` starts empty. As report-library (and later) features become user-facing,
  the maintainer adds `Added/Changed/Fixed` bullets there. `bun run release` promotes them.
- Added to the Starlight sidebar in `apps/docs/astro.config.ts` as the final item:
  `{ label: 'Changelog', slug: 'changelog' }`.
- Compare links use `github.com/shakedex/LunaApp` (matches the docs GitHub social link).

**Root `CHANGELOG.md`** — thin pointer so developers browsing the repo root find it:

```markdown
# Changelog

The Luna Web changelog lives with the docs so it publishes to the site.

- Online: https://luna.ozer2.one/docs/changelog/
- Source: apps/docs/src/content/docs/changelog.md

Versioning follows ZeroVer (0.MINOR.PATCH). See RELEASING.md for how releases are cut.
```

### 3.4 Release mechanism — `tools/release.ts`

Wired as a root script: `package.json` → `"release": "bun tools/release.ts"`.
Invoked `bun run release <minor|patch>` (with `--dry-run` to preview without writing).

**Pure, unit-tested helpers** (keep git/fs side effects thin):

- `parseVersion(s: string): { major: 0; minor: number; patch: number }` — asserts `major === 0`.
- `nextVersion(current, bump: 'minor' | 'patch'): string` — `minor` → `0.(minor+1).0`,
  `patch` → `0.minor.(patch+1)`. Major is hard-locked to `0`.
- `rewriteChangelog(text, version, date): string` — renames `## [Unreleased]` → adds a
  fresh empty `## [Unreleased]` above `## [version] - date`, moving existing unreleased
  entries under the new version heading, and rewrites the `[unreleased]` / adds `[version]`
  compare-link reference definitions.

**`main()` flow:**

1. Parse arg. Anything other than `minor` / `patch` → print usage and exit non-zero.
   `major` (or any request that would reach `1.0.0`) → refuse with a ZeroVer message.
2. Require a clean working tree (`git status --porcelain` empty) — else abort, so unrelated
   in-flight changes never get bundled into the release commit.
3. Read `apps/web/package.json` version; `parseVersion` (asserts `0.x.y`).
4. Require `## [Unreleased]` in the changelog to have at least one entry — else abort
   ("nothing to release; add entries under [Unreleased] first"). This is the enforcement
   point that keeps the changelog actually maintained.
5. Compute `next = nextVersion(...)`; write it back to `apps/web/package.json` (2-space
   indent, trailing newline).
6. `rewriteChangelog` the docs changelog file with `next` and today's date (`new Date()` —
   a normal bun runtime, not a Workflow script, so the clock is available).
7. Stage exactly `apps/web/package.json` and `apps/docs/src/content/docs/changelog.md`,
   commit `chore(release): v<next>`, create annotated tag `v<next>`.
8. Print the finish line: `Released v<next>. Push with: git push --follow-tags`.
   The script does **not** push and does **not** deploy — deploys stay on the Cloudflare
   dashboard git integration (per `DEPLOY.md`).

Implemented with Bun APIs (`Bun.file`, `Bun.$` for git). DOM-free, standalone.

**Tests:** `tools/release.test.ts` (bun) covers `nextVersion` (minor + patch), the
`major`/`1.0.0` rejection, and `rewriteChangelog` happy path (Unreleased → version + fresh
Unreleased + link definitions).

### 3.5 Docs cross-references

- `DEPLOY.md` "Versioning (ZeroVer)" section gains one line: "See `RELEASING.md` for the
  release workflow (`bun run release`)."
- `RELEASING.md` (new, repo root) documents: the one-command flow, the ZeroVer rule (major
  locked at 0, never 1.0), how to write good `[Unreleased]` entries (curate as you land
  user-facing work), and the fact that pushing the tag is a separate manual step.

## 4. Out of scope (YAGNI)

No git-cliff / conventional-changelog / changesets, no auto-generation from commit
messages, no per-package versioning, no CI release automation, no docs-sync script
(canonical-in-docs removes the need). CI stays quality-gates-only; cutting a release stays
a deliberate local `bun run release`.

## 5. File-by-file change list

**New**
- `apps/docs/src/content/docs/changelog.md` — canonical changelog (Starlight page).
- `CHANGELOG.md` — root pointer.
- `tools/release.ts` — release script.
- `tools/release.test.ts` — bun tests for the pure helpers.
- `RELEASING.md` — release runbook.

**Modified**
- `apps/web/package.json` — `version` `0.0.0` → `0.11.0`.
- `package.json` (root) — add `"release": "bun tools/release.ts"` script.
- `apps/docs/astro.config.ts` — add `{ label: 'Changelog', slug: 'changelog' }` to sidebar.
- `DEPLOY.md` — one-line pointer to `RELEASING.md`.

**Git (not a file change)**
- Annotated tag `v0.11.0` at `6d41b18`.

## 6. Verification

- `bun run lint && bun run typecheck && bun test && bun run build` green from repo root
  (the release script/tests must not break the gates; the docs build must include the new
  Changelog page).
- `bun run release --dry-run patch` previews a bump against a seeded `[Unreleased]` entry
  without writing (manual smoke check).
- `apps/web` build shows `v0.11.0` in the header/Credits; docs site serves `/docs/changelog/`.

## 7. Constraints honored

- Windows, Git Bash, bun 1.3.14, TypeScript 6.0.3, `.gitattributes` LF pinning.
- The maintainer works `master` in parallel: this work only touches the files listed in §5
  (all clean at design time) and stages by explicit path — never `git add -A`.
- `packages/core` untouched (stays DOM-free/clock-free). `apps/web/src/components/ui/`
  untouched.
