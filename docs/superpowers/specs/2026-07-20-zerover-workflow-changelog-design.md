# Luna Web — ZeroVer Workflow + Changelog Design

**Date:** 2026-07-20
**Status:** Approved — implementing (revised from `0.11.0` to `0.12.0` after milestone 12 shipped)
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
| Starting version | **`0.12.0`** — one MINOR per shipped milestone (01–12) |
| Baseline tag | `v0.12.0` on the release commit (the commit that sets `version` to 0.12.0) |
| Changelog format | Keep a Changelog 1.1.0, hand-curated, plain user-facing language |
| Canonical changelog file | `apps/docs/src/content/docs/changelog.md` (a Starlight page) |
| Root `CHANGELOG.md` | Thin pointer to the canonical file + online URL |
| Release mechanism | `bun run release <minor\|patch>` (`tools/release.ts`) + `RELEASING.md` |
| Auto-generation | **None** — no git-cliff/conventional-changelog/changesets |

### Why `0.12.0`

Verified against the tree on 2026-07-20: milestone 12 (Report Library) is **complete**. All
five tasks landed on `master` — `db41cf2` (core summary), `69a785e` (idb-v4 persistence),
`9c7bc42` (shared `ReportView` + Save button), and `52d5d5b` (the `/reports` list route and
the read-only saved-report view, `reports.index.tsx` + `reports.$reportId.tsx`). Twelve
milestones are shipped, so the current state is `0.12.0`, and the Report Library is a
shipped feature in its changelog entry — not `[Unreleased]`.

The baseline tag is placed on the release commit itself (the commit that sets `version` to
`0.12.0`) — the same way `bun run release` tags every future release. That captures all of
milestone 12, including its refinements (e.g. the read-only saved-report view), and is
robust against the maintainer continuing to commit on `master` in parallel. `[Unreleased]`
starts empty.

(Had milestone 12 still been unfinished, the version would have been `0.11.0` with the
report-library work sitting under `[Unreleased]`. It finished, so `0.12.0` is correct.)

## 3. Component design

### 3.1 Version source of truth

`apps/web/package.json` `version` is the one product version. `packages/core`,
`apps/docs`, and the workspace root stay unversioned / `0.0.0` — they are internal and
never published to npm. No re-plumbing: `apps/web/vite.config.ts` already injects
`__APP_VERSION__` from this field into the app header, Credits page, and PDF footer stamp.

### 3.2 Baseline: adopt `0.12.0`

- Set `apps/web/package.json` `version` → `0.12.0`.
- Create annotated tag `v0.12.0` on the release commit (`git tag -a v0.12.0 -m "v0.12.0"`).
- We do **not** fabricate `0.1.0`–`0.11.0` dated releases; they never happened. `0.12.0`
  is the first *tracked* release, and its changelog entry consolidates milestones 01–12
  (including the Report Library).

### 3.3 Changelog — canonical file lives in docs

**File:** `apps/docs/src/content/docs/changelog.md` — a Starlight content page.

Structure (Keep a Changelog 1.1.0), with the H1 omitted (Starlight renders the title from
frontmatter): frontmatter (`title: Changelog`), an intro line, `## [Unreleased]`, then
`## [0.12.0] - 2026-07-20` with an `### Added` group summarizing the app as shipped through
milestone 12 in plain user-facing language, then the compare-link reference definitions.

- `[Unreleased]` starts empty. As later features become user-facing, the maintainer adds
  `Added/Changed/Fixed` bullets there. `bun run release` promotes them into a dated version.
- Added to the Starlight sidebar in `apps/docs/astro.config.mjs` as the final item:
  `{ label: 'Changelog', slug: 'changelog' }`.
- Compare links use `github.com/shakedex/LunaApp` (matches the docs GitHub social link).

**Root `CHANGELOG.md`** — thin pointer so developers browsing the repo root find it:
points at the online URL and the canonical source file, and notes ZeroVer + `RELEASING.md`.

### 3.4 Release mechanism — `tools/release.ts`

Wired as a root script: `package.json` → `"release": "bun tools/release.ts"`.
Invoked `bun run release <minor|patch>` (with `--dry-run` to preview without writing).

**Pure, unit-tested helpers** (keep git/fs side effects thin):

- `parseVersion(s): { major: 0; minor; patch }` — asserts `major === 0` (ZeroVer).
- `nextVersion(current, bump: 'minor' | 'patch'): string` — `minor` → `0.(minor+1).0`,
  `patch` → `0.minor.(patch+1)`. Major is hard-locked to `0`.
- `unreleasedBody(text): string` — the entries between `## [Unreleased]` and the next
  `## [` heading (link defs stripped); empty string means nothing to release.
- `rewriteChangelog(text, {version, date, prevVersion}): string` — renames `## [Unreleased]`
  → adds a fresh empty `## [Unreleased]` above `## [version] - date`, moving existing
  unreleased entries under the new version heading, and rewrites the `[unreleased]` / adds
  `[version]` compare-link reference definitions.

**`main()` flow:**

1. Parse arg. Anything other than `minor` / `patch` → usage + exit non-zero. `major` (or
   anything that would reach `1.0.0`) → refuse with a ZeroVer message.
2. `--dry-run`: print `current -> next` and the current `[Unreleased]` body, then stop
   (no clean-tree requirement, no writes).
3. Real run preconditions: clean working tree (`git status --porcelain` empty) and a
   non-empty `[Unreleased]` — else abort. This is the enforcement point that keeps the
   changelog actually maintained.
4. Write the bumped version into `apps/web/package.json` (regex-preserving formatting).
5. `rewriteChangelog` the docs changelog with `next` and today's date (`new Date()` — a
   normal bun runtime, not a Workflow script, so the clock is available).
6. Stage exactly `apps/web/package.json` and the docs changelog, commit
   `chore(release): v<next>`, create annotated tag `v<next>`.
7. Print `Released v<next>. Push with: git push --follow-tags`. No push, no deploy —
   deploys stay on the Cloudflare dashboard git integration (per `DEPLOY.md`).

**Tests:** `tools/release.test.ts` (bun) covers `parseVersion` (0.x + the `1.0.0`
rejection), `nextVersion` (minor + patch), `unreleasedBody` (present + empty), and
`rewriteChangelog` (promotion + fresh Unreleased + link definitions).

### 3.5 Docs cross-references

- `DEPLOY.md` "Versioning (ZeroVer)" section gains one line pointing to `RELEASING.md`.
- `RELEASING.md` (new, repo root) documents the one-command flow, the ZeroVer rule (major
  locked at 0, never 1.0), how to write `[Unreleased]` entries, and that pushing the tag
  is a separate manual step.

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
- `apps/web/package.json` — `version` `0.0.0` → `0.12.0`.
- `package.json` (root) — add `"release": "bun tools/release.ts"` script.
- `apps/docs/astro.config.mjs` — add `{ label: 'Changelog', slug: 'changelog' }` to sidebar.
- `DEPLOY.md` — one-line pointer to `RELEASING.md`.

**Git (not a file change)**
- Annotated tag `v0.12.0` on the release commit.

## 6. Verification

- `bun run lint && bun run typecheck && bun test && bun run build` green from repo root.
- `bun run release --dry-run patch` previews a bump without writing (manual smoke check).
- `apps/web` build shows `v0.12.0` in the header/Credits; docs site serves `/docs/changelog/`.

## 7. Constraints honored

- Windows, Git Bash, bun 1.3.14, TypeScript 6.0.3, `.gitattributes` LF pinning.
- The maintainer works `master` in parallel: this work only touches the files listed in §5
  and stages by explicit path — never `git add -A`.
- `packages/core` untouched (stays DOM-free/clock-free). `apps/web/src/components/ui/`
  untouched.
