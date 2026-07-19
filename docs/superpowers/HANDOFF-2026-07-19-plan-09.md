# Luna Web — Handoff for Plan 09 (written 2026-07-19, end of Plan 08)

You are a fresh agent picking up an in-flight project. Read this whole file before doing
anything. The previous session ran Plans 01–08 to completion; your job starts at scoping
**Plan 09** with the maintainer (Shaked).

## What this project is

**Luna Web** — a fully client-side (Chromium-only, File System Access API) web port of the
Luna camera-report tool, in this repo under `web/` (Bun workspace). A DIT picks a footage
folder; the app scans, extracts metadata + thumbnails locally (nothing uploads), groups
clips into reels, and exports a PDF/CSV camera report. The .NET desktop app at the repo
root is legacy — **never touch anything outside `web/`, `docs/superpowers/`, and
`.github/workflows/web-ci.yml`**.

- Branch: `feature/luna-web` (~104 commits ahead of master). **NOT pushed — Shaked pushes
  personally. Never push.**
- All gates green at handoff: from `web/` → `bun run lint && bun run typecheck && bun test
  && bun run build` (121 core tests). Run these before AND after everything.
- Publish target (deploy plan, not yet built): `luna.ozer2.one`, docs at path `/docs`,
  two Cloudflare Workers (free plan), CDN-cached ffmpeg core (never bundle its 31MB).

## THE MAINTAINER'S HARD RULES (violations got prior work rejected — memorize)

1. **Never hand-write a dependency version.** Every dep enters via `bun add`/`bun add -d`
   (installs latest) or an official scaffolder CLI. Exception only when tooling forces a
   ceiling (workspace is unified on TypeScript 6 because `astro check` rejects TS 7 —
   don't "upgrade" it). Vet new deps for maintenance/recency; present vetted lists, don't
   speculatively install (exifr was rejected as stale; LibRaw-Wasm rejected as untrusted).
2. **Official CLIs scaffold; humans run interactive CLIs.** Shaked personally runs anything
   interactive (create-vite, shadcn init, create-astro): stop, hand over the exact command
   and prompt answers, wait for "done". Git commands and visual checks are NOT handoffs.
3. **No MVP framing.** Full-featured implementations; don't propose stripped spikes.
4. **Bun only.** Package manager, task runner, `bun test` (core only — no Vitest, no
   Playwright). Shaked tests UI flows manually; give them a QA checklist per plan.
5. **Plans need explicit approval.** Rhythm: write plan → commit → present summary +
   decisions → wait for the literal "go" → execute. Present library/stack choices with
   trade-offs and WAIT; never treat a proposal as approved.
6. **Shaked works the repo in parallel** (their "design track": app styling, logos, their
   research tooling). Their commits land mid-execution constantly. Consequences:
   - Stage by EXPLICIT file paths, never `git add web/`.
   - `git status` before staging; if a shared file has their uncommitted changes, STOP and
     coordinate (this happened; they appreciated the pause).
   - Review packages may span their commits — scope reviewers to our hunks/dirs.
   - Never modify `web/app/src/components/ui/` (theirs), `web/tools/` (their research
     tooling — read-only + running `bun tools/analyze-clips.mjs` is fine).
7. **Data-driven, never guess.** `web/tools/FINDINGS.md` is a LIVING requirements doc from
   Shaked's real-corpus research (corpus at `D:/LUNA_TEST/TEST_PROJECT_LUNA/CAMERA`; dumps
   in `web/tools/out/`). **Standing instruction: re-read FINDINGS before scoping any
   format-related work** — their separate research agent keeps updating it (.crm and other
   formats). When a key name/offset/codec string is unknown: read the dumps, run their
   tool, or probe the real file. Subagents return NEEDS_CONTEXT rather than invent.

## The process that has been working (keep it)

Superpowers skills drive everything: `superpowers:writing-plans` for plan docs,
`superpowers:subagent-driven-development` (SDD) for execution. Concretely per plan:

1. Plan file at `docs/superpowers/plans/YYYY-MM-DD-luna-web-NN-<topic>.md` (see 01–08 for
   the exact format: header, Global Constraints, per-task Files/Interfaces/checkbox steps
   with complete code, Definition of done incl. maintainer QA checklist, self-review).
   Commit it, present, await "go".
2. Per task: record BASE commit → `bash "<skills-dir>/scripts/task-brief" <plan> <N>` →
   dispatch ONE implementer subagent (model by complexity: haiku = transcription of
   complete plan code; sonnet = integration/data-driven; the dispatch carries environment
   notes, VERIFY-DON'T-ASSUME items, report path `.superpowers/sdd/task-pNN-N-report.md`,
   and the DONE/DONE_WITH_CONCERNS/NEEDS_CONTEXT/BLOCKED contract) → verify the commit →
   `scripts/review-package BASE HEAD` → dispatch a reviewer subagent (sonnet) with brief +
   report + diff paths → adjudicate; Critical/Important → fix subagent → re-review (reuse
   the same reviewer via SendMessage). Skill dir:
   `C:\Users\shake\.claude\plugins\cache\claude-plugins-official\superpowers\6.1.1\skills\subagent-driven-development\`
3. After all tasks: FINAL whole-range review on **opus**, fixes + re-review, then close.
4. **Ledger** (`.superpowers/sdd/progress.md`, git-ignored): append per-task entries,
   decisions, minors, carry-forwards. It is the durable memory — read it fully at start;
   it holds the complete Plans 01–08 history and all open items.
5. Reviews have caught real bugs every plan (empirically reproduced parser defects, an
   offset error settled by probing the real .crm on disk, race conditions, my own plan
   bugs). Never skip them; never tell a reviewer what not to flag. Environment for
   subagents: Windows, Git Bash tool, `cd /e/Coding/LunaApp` first (cwd drifts), bun
   1.3.14, CRLF warnings benign.

## Key documents

- **Spec**: `docs/superpowers/specs/2026-07-17-luna-web-design.md` — amended over time:
  shadcn on **Base UI** (base-nova, NOT Radix); **mediabunny + @mediabunny/prores** primary
  decode (mp4box.js dropped); ffmpeg.wasm fallback (CDN, lazy, Cache API). §20 milestones.
- **Plans 01–08**: same dir. 01 foundation, 02 scan, 03 metadata, 04 thumbnails,
  05 reels/report workspace, 06 exports, 07 vendor metadata enrichment, 08 RAW clips +
  embedded previews. Read the plan + ledger entries for any subsystem before touching it.
- **FINDINGS.md** (`web/tools/`): the corpus truth. Also Shaked's own design docs:
  `docs/superpowers/specs/2026-07-19-luna-web-visual-design.md` + their implementation
  plan — their track, not yours, but read before UI work.

## Architecture snapshot (what exists, where)

- `web/packages/core` — pure TS, **DOM-free** (structural `BlobLike`/`FileHandleLike`),
  `bun test`ed (121): extensions/allowlist (`.ari` is the only raw-notice left; braw/r3d/
  crm are clips), scan walker (+`.rtn` sidecar association → `ClipRef.previewSidecar`),
  mediainfo mapper + **vendor enricher registry** (arri-mov, arri-mxf, acquisition
  Sony/Canon, braw, panasonic — first-match, content-detected, corpus-verified),
  `runPool` (generic lane pool: cancellation, retry-on-fresh-lane, settle-all-before-
  reject), thumbs router `thumbnailRouteFor(ext, codec)` (mediabunny|ffmpeg|preview|none),
  ISO-BMFF box walker + SOF-sanity JPEG utils + crm/tail-moov/rtn extractors, reels
  (`detectReels`: reelName → top folder → Ungrouped), `ReportModel`/`buildReportModel`
  (FROZEN exporter contract + `ReelStats`), CSV generator (RFC-4180, formula-guarded).
- `web/app` — Vite+React19+TanStack Router/Store/Form+Tailwind4+shadcn(Base UI):
  scan feature (File System Access, idb recent sources), two-pass processing under a
  **run-token + guardedUpdate discipline** (metadata pass → thumbnail pass with THREE
  queues: mediabunny workers ≤4, ffmpeg single-lane lazy CDN engine, preview queue), phase
  machine idle→scanning→summary→processing→thumbnailing→processed→error, report workspace
  (Shaked heavily restyled it — "camera report" look, Cinema Dark, useSelector), exporter
  registry (PDF via react-pdf with mime-branched image prep; CSV), `coverStore` (separate,
  survives Start over, date-seeded).
- Boundary casts are few and documented (scan handle, metadata File, worker OffscreenCanvas,
  pdf() weak-type) — audit greps exist in prior plans; don't add undocumented casts.
- `web/docs` — Astro Starlight, base `/docs`, near-empty content (a milestone remains).
  NOTE: astro `base` prefixes URLs only — build emits `web/docs/dist/index.html` (matters
  for deploy Worker routing).
- CI: `.github/workflows/web-ci.yml` (gates on push to feature/luna-web + master).

## Plan 09 — what remains (scope WITH Shaked, present options, await "go")

Remaining spec milestones + accumulated carry-forwards, roughly in value order:

**A. Results grid + virtualization polish** (spec §8.10 deferred from 05/06): TanStack
Table sortable/filterable view + TanStack Virtual for big cards. Contracts to preserve:
`thumbStatus`/`thumbsById` keyed by `clip.id` (= relativePath); `clips` array identity
stability (a WeakMap cache in `clip-row.tsx` depends on it). Coordinate heavily with
Shaked's restyle — they own the look. Consider moving preview normalization into the
thumbs worker if scroll work lands (main-thread queue note).

**B. Settings + activity** (spec §8.13/§8.14/§14, milestone 8): settings route (worker cap,
theme, cover defaults persistence into idb with schemaVersion migrations), activity/log
view (ring buffer logger), stale-recent-source UX (§15 — mark stale, re-pick prompt
instead of raw DOMException), credits route. Carry-forward: surface per-clip failure
reasons honestly (ProRes RAW metadata-failure edge currently reads 'failed' not
'no browser decoder' — harden here).

**C. Docs content** (milestone 9): Starlight pages — what it is, privacy ("footage never
leaves your device"), supported formats (BRAW/R3D/CRM are first-class clips now — do NOT
call them "undecodable RAW"; .ari-only notice), limitations, FAQ.

**D. Deploy** (milestone 10): two Workers, `luna.ozer2.one` routes (`/docs*` → docs Worker
— remember the outDir note), Wrangler configs, CI deploy. Needs Shaked's Cloudflare zone —
human handoffs likely.

Small logged debts (fold into whichever plan touches them — full list in the ledger):
uncapped `.rtn` read; `_mime` param rename in pdf-prepare; `jpegDimensions` printable-type
nit; BRAW reel_name is a bare integer ("1") as a reel display name; ARRI whole-number
shutter renders `180°` vs Sony `180.0°`; per-clip checksums (MHL) would need hashes added
to ReportModel (data gap, registry is ready); docs TS6 ceiling — revisit when Astro
supports TS7; consider a `priority` field on exporters instead of push/unshift ordering.

## First moves for the new session

1. Read `.superpowers/sdd/progress.md` end-to-end (the full history + open items).
2. `git log --oneline master..HEAD | head -30` and `git status` — Shaked's commits land
   continuously; sync your mental model.
3. Re-read `web/tools/FINDINGS.md` (their research agent may have added formats).
4. Run the four gates from `web/` to confirm green baseline.
5. Present the Plan 09 scoping options (A–D above, adjusted for anything new in FINDINGS
   or their design track) with a recommendation — and wait for Shaked's choice + "go".
