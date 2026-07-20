# Luna Web — Anonymous Community Stats Design

**Date:** 2026-07-20
**Status:** Approved — awaiting implementation plan
**Branch:** `feat/anonymous-stats`

## 1. Motivation

Luna processes real production media every day, but nobody — including us — can see the
aggregate: how many clips read, how much data crunched, how many reports shipped. A public
`/stats` page showing live community totals is fun, is honest marketing, and costs almost
nothing to run — **if** it is done without weakening Luna's privacy story. This design adds
Luna's first server component: an anonymous, opt-out, aggregate-counters-only telemetry
pipeline and the page that displays it.

## 2. Decisions (locked during brainstorming)

| Decision | Choice |
| --- | --- |
| Scope | Global community stats (aggregate across all users) |
| Consent model | Opt-out; on by default; toggle in Settings |
| Dedup | Client-side fingerprint set; server receives plain counter deltas only |
| Backend | New `apps/stats` Worker + D1, route `luna.ozer2.one/api/stats*` (manual, dashboard) |
| Abuse protection | Turnstile (invisible) + Cloudflare rate-limit rule + server-side clamps |
| Stats page | In-app route `/stats` |
| Metrics | Core five + format/camera breakdowns (see §3) |
| Privacy docs | `privacy.md` rewritten; README tagline softened to "Your footage never leaves your device" |

## 3. Metrics

All-time global counters. Nothing per-user, nothing per-file, no identifiers of any kind.

| Metric | Storage | Source event |
| --- | --- | --- |
| Clips read | `clips` counter | clip successfully processed |
| Data processed | `bytes` counter | sum of processed clip file sizes |
| Thumbnails generated | `thumbs` counter | thumbnail pipeline output |
| Reports created | `reports` counter | report generation completes |
| Avg report generation time | `report_ms_sum` + `report_count` | measured around generation; divided at display time |
| Format breakdown | `categories(kind='format', name, value)` | codec family per clip (ProRes, BRAW, R3D, ARRIRAW, H.264/5, DNxHD, …) |
| Camera breakdown | `categories(kind='camera', name, value)` | manufacturer per clip, from existing metadata |

Category names come from a normalization step over metadata Luna already extracts —
lowercased, mapped to a known family where possible, bucketed as `other` when unknown.
Unbounded free-text category names are never sent.

## 4. Client pipeline (`packages/core` + `apps/web`)

Pure logic (fingerprints, delta accumulation/merge, category normalization) lives in
`packages/core`; IndexedDB and network glue live in `apps/web`.

### 4.1 Fingerprints & dedup

- **Clip fingerprint:** SHA-256 over the clip identity Luna already derives (name + size +
  mtime + duration). Computed locally, stored locally, **never transmitted**.
- **Report fingerprint:** SHA-256 over the sorted set of its clip fingerprints.
- **Reported set:** a new IndexedDB object store of fingerprints already counted.
- On work completion, diff fresh fingerprints against the reported set; only never-seen
  work contributes to the delta. Consequences:
  - Re-scanning the same card → +0.
  - Regenerating an identical report → +0.
  - Adding a card to a report and regenerating → only the new clips count, and the report
    (new clip-set → new fingerprint) counts once more.
- Known limitation (accepted): the reported set is per browser profile. Clearing site data
  or using another machine counts the same media again. This is inflation-prevention, not
  global uniqueness — global dedup would require sending per-clip hashes, which is ruled
  out for privacy.

### 4.2 Outbox & sending

- Deltas accumulate into a **single merged outbox record** in IndexedDB
  (`{clips, bytes, thumbs, reports, reportMsSum, reportCount, formats: {…}, cameras: {…}}`).
- Flush triggers: app start, `online` event, after new activity lands. Flush = obtain
  invisible Turnstile token → `POST /api/stats/ingest` with the delta JSON → clear outbox
  only on 2xx. Failure leaves the outbox intact for the next trigger.
- Offline sessions simply keep accumulating; the merged record arrives whenever the app is
  next online. Telemetry never blocks or slows actual work.
- No cookies, no install ID, no session ID, no timestamps beyond the request itself.

### 4.3 Settings toggle

`Share anonymous usage stats` in Settings (settings-store + settings-screen), **default
on**. When off: no fingerprint tracking, no accumulation, outbox deleted immediately, the
Turnstile script is never loaded. One-line plain-language description linking to the
privacy page.

## 5. Backend (`apps/stats`)

A new workspace member, deliberately tiny — one Worker plus a D1 database.

- **Deploy:** dashboard Git integration like the other two Workers (root directory
  `apps/stats`); route `luna.ozer2.one/api/stats*` added manually in the dashboard per the
  repo's existing no-routes-in-config convention. D1 database created in the dashboard,
  bound in `apps/stats/wrangler.jsonc`.
- **Schema:** `counters(key TEXT PRIMARY KEY, value INTEGER)` and
  `categories(kind TEXT, name TEXT, value INTEGER, PRIMARY KEY (kind, name))`.
- **`POST /api/stats/ingest`:**
  1. Verify the Turnstile token server-side (secret lives only in the Worker). No token /
     invalid token → 403.
  2. Validate shape; clamp every field to per-request sanity caps (counts, bytes, max
     category entries, max name length); re-normalize category names server-side.
  3. Atomic `UPDATE … SET value = value + ?` (upsert for categories). Return 204.
  4. Store nothing else — no IPs, no logs of payloads, no request metadata.
- **`GET /api/stats`:** all counters + categories as JSON (avg derived client-side from
  sum/count), edge-cached ~60 s, CORS limited to the app origin.
- **Rate limiting:** Cloudflare WAF rate-limit rule on the ingest route (dashboard-managed,
  like routes/domains), 10 requests per IP per hour (legit clients flush far less).

### 5.1 Abuse posture (honest limits)

There are no accounts and the client is open source, so embedded secrets are impossible —
the endpoint cannot be *authenticated*, only *defended*. The layers: Turnstile kills
scripted/curl abuse and replay (single-use browser-issued tokens verified server-side);
rate limiting kills volume; clamps cap the damage of any single forged request. Residual
risk — someone running the real app against synthetic media — is accepted; no consent
model can prevent it.

## 6. Stats page (`/stats`)

- New TanStack route + `features/stats` screen, built with the existing shadcn/Cinema Dark
  kit.
- Big-number tiles for the core five (count-up animation on load), a format breakdown and
  a camera breakdown visualization, auto-refresh every 60 s.
- Data unreachable/offline → graceful placeholder state, no errors thrown.
- Footer note: "Anonymous aggregate counters — see how this works" → privacy page. Link
  the page from the app nav where /activity and /credits already live.

## 7. Privacy & docs changes (part of the feature, not follow-up)

- **`apps/docs/src/content/docs/privacy.md`:** the current "has no analytics or telemetry"
  claim is removed. Replacement states exactly what is sent (show the real payload JSON),
  what is never sent (file names, paths, hashes, footage, metadata values, IPs, IDs,
  cookies), when it is sent, that Turnstile is used for abuse protection at send time, and
  where the opt-out lives.
- **README:** tagline becomes "Your footage never leaves your device" (still true, no
  longer overclaiming). Stats page linked as a feature.
- Settings toggle copy links to the privacy page.

## 8. Error handling

- Ingest failure (network, 4xx/5xx, Turnstile failure) → outbox persists, retried on next
  flush trigger; silent to the user.
- Malformed/oversized ingest payloads → 4xx, no partial writes.
- GET failure on the stats page → placeholder state.
- Telemetry code paths are wrapped so a bug there can never break scanning/reporting.

## 9. Testing

- `bun test` coverage for the pure logic: fingerprint derivation (stability + sensitivity),
  delta accumulation/merge, category normalization, outbox merge semantics.
- Worker: validation/clamping/upsert logic extracted as pure functions with `bun test`
  coverage; the fetch handler stays thin. Turnstile verification mocked at the boundary.
- Manual verification checklist: offline accumulation → later flush; opt-out wipes outbox;
  re-scan counts +0.

## 10. Out of scope

- Historical time-series / trend charts (counters are all-time totals only).
- Global cross-user dedup.
- Per-user cloud profiles or any identifier, even hashed.
- Docs-site embedding of live counters.
