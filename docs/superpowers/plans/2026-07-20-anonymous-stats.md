# Anonymous Community Stats Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Anonymous, opt-out, aggregate-counters-only telemetry feeding a public in-app `/stats` page, per `docs/superpowers/specs/2026-07-20-anonymous-stats-design.md`.

**Architecture:** Pure logic (delta model, clamps, category normalizers, fingerprints) lives in `@luna-web/core` and is shared by the web client and a new tiny `apps/stats` Cloudflare Worker (D1-backed). The client dedups locally (fingerprint set in IndexedDB), accumulates deltas in an offline outbox, and flushes them with an invisible Turnstile token. The Worker verifies the token, clamps, and upserts counters; `GET /api/stats` serves the totals the `/stats` page renders.

**Tech Stack:** Bun workspace, TypeScript strict, React 19 + TanStack Router/Store, idb, Cloudflare Workers + D1 + Turnstile, `bun test`.

## Global Constraints

- Work directly on `master` — no branches, no worktrees (user directive 2026-07-20).
- Bun only: `bun test`, `bun add` (installs latest), `bun run …`. Never npm/npx except `npx wrangler` inside Cloudflare's own deploy pipeline.
- Biome style: single quotes, no semicolons, 2-space indent. Run `bun run lint` before every commit; fix with `bun run format` if needed.
- `useSelector` from `@tanstack/react-store` — never the deprecated `useStore`.
- UI copy uses plain developer terminology; no coined feature names ("anonymous usage counters", not a branded name).
- ZeroVer: no version bumps in this plan; changelog/release happens via the `luna-release` skill in Task 12.
- Core stays DOM-free: `crypto.subtle` and plain data only — no `window`, no `import.meta.env`.
- Commit scopes observed in history: `feat(core)`, `feat(web)`, `docs(web)`, plus new `feat(stats)` for `apps/stats`.
- Every task ends with: `bun run lint` clean, `bun run typecheck` clean, tests passing, one commit.

## File Structure

```
packages/core/src/stats/
  delta.ts + delta.test.ts            # StatsDelta model, merge, defensive normalize + clamps
  categories.ts + categories.test.ts  # formatFamilyOf, cameraMakerOf
  fingerprint.ts + fingerprint.test.ts# sha256Hex, clipFingerprint, reportFingerprint
packages/core/src/settings/model.ts   # v3: shareUsageStats
apps/stats/
  package.json, tsconfig.json, wrangler.jsonc, schema.sql
  src/ingest.ts + src/ingest.test.ts  # pure statement builder + GET response shaper
  src/index.ts                        # thin fetch handler (Turnstile verify, D1 batch)
apps/web/src/persistence/db.ts        # v5: statsReported + statsOutbox stores
apps/web/src/persistence/stats.ts     # outbox + reported-set persistence
apps/web/src/features/stats/
  config.ts                           # TURNSTILE_SITE_KEY, STATS_API_BASE
  telemetry.ts                        # recordProcessedRun, recordReportExport, outbox lock
  flush.ts                            # Turnstile token + POST, initStatsFlush
  stats-screen.tsx                    # /stats page
apps/web/src/routes/stats.tsx         # route
apps/web/src/components/app-shell.tsx # nav link
apps/web/src/features/settings/settings-screen.tsx  # Privacy card
apps/docs/src/content/docs/privacy.md # rewrite
README.md, apps/docs/astro.config.mjs, apps/web/index.html, DEPLOY.md
```

---

### Task 1: Core stats delta model

**Files:**
- Create: `packages/core/src/stats/delta.ts`
- Test: `packages/core/src/stats/delta.test.ts`
- Modify: `packages/core/src/index.ts` (exports)

**Interfaces:**
- Consumes: nothing.
- Produces (used by Tasks 5–8, 10):
  - `interface StatsDelta { clips: number; bytes: number; thumbs: number; reports: number; reportMsSum: number; formats: Record<string, number>; cameras: Record<string, number> }`
  - `emptyStatsDelta(): StatsDelta`
  - `isEmptyStatsDelta(d: StatsDelta): boolean`
  - `mergeStatsDeltas(a: StatsDelta, b: StatsDelta): StatsDelta`
  - `normalizeStatsDelta(raw: unknown): StatsDelta`
  - `STATS_CLAMPS` constant object
- Note: `normalizeStatsDelta` depends on `FORMAT_FAMILIES`/`CAMERA_MAKERS` from Task 2's `categories.ts`. Task 1 creates `categories.ts` with ONLY the two const arrays (no functions yet); Task 2 adds the functions.

- [ ] **Step 1: Create the category name lists** (needed by normalize's key allowlist)

`packages/core/src/stats/categories.ts`:

```ts
// Closed category vocabularies for the community stats (spec §3): every
// breakdown key sent or stored is one of these — unknown values collapse to
// 'other' at the normalization boundary, so free text never leaves the app.
export const FORMAT_FAMILIES = [
  'prores',
  'braw',
  'redcode',
  'arriraw',
  'canonraw',
  'h264',
  'hevc',
  'dnx',
  'other',
] as const
export type FormatFamily = (typeof FORMAT_FAMILIES)[number]

export const CAMERA_MAKERS = [
  'arri',
  'red',
  'blackmagic',
  'canon',
  'sony',
  'panasonic',
  'fujifilm',
  'nikon',
  'dji',
  'gopro',
  'other',
] as const
export type CameraMaker = (typeof CAMERA_MAKERS)[number]
```

- [ ] **Step 2: Write the failing tests**

`packages/core/src/stats/delta.test.ts`:

```ts
import { describe, expect, test } from 'bun:test'
import {
  emptyStatsDelta,
  isEmptyStatsDelta,
  mergeStatsDeltas,
  normalizeStatsDelta,
  STATS_CLAMPS,
} from './delta'

describe('emptyStatsDelta', () => {
  test('all zeros, fresh objects each call', () => {
    const a = emptyStatsDelta()
    expect(a).toEqual({
      clips: 0,
      bytes: 0,
      thumbs: 0,
      reports: 0,
      reportMsSum: 0,
      formats: {},
      cameras: {},
    })
    expect(emptyStatsDelta().formats).not.toBe(a.formats)
  })
})

describe('isEmptyStatsDelta', () => {
  test('empty delta is empty', () => {
    expect(isEmptyStatsDelta(emptyStatsDelta())).toBe(true)
  })
  test('any counter or category makes it non-empty', () => {
    expect(isEmptyStatsDelta({ ...emptyStatsDelta(), bytes: 1 })).toBe(false)
    expect(isEmptyStatsDelta({ ...emptyStatsDelta(), formats: { prores: 1 } })).toBe(false)
  })
})

describe('mergeStatsDeltas', () => {
  test('sums counters and unions category maps', () => {
    const merged = mergeStatsDeltas(
      { clips: 2, bytes: 100, thumbs: 6, reports: 1, reportMsSum: 500, formats: { prores: 2 }, cameras: { arri: 2 } },
      { clips: 3, bytes: 50, thumbs: 9, reports: 0, reportMsSum: 0, formats: { prores: 1, braw: 2 }, cameras: {} },
    )
    expect(merged).toEqual({
      clips: 5,
      bytes: 150,
      thumbs: 15,
      reports: 1,
      reportMsSum: 500,
      formats: { prores: 3, braw: 2 },
      cameras: { arri: 2 },
    })
  })
  test('does not mutate its inputs', () => {
    const a = emptyStatsDelta()
    mergeStatsDeltas(a, { ...emptyStatsDelta(), clips: 1, formats: { braw: 1 } })
    expect(a.clips).toBe(0)
    expect(a.formats).toEqual({})
  })
})

describe('normalizeStatsDelta', () => {
  test('non-object input yields the empty delta', () => {
    expect(normalizeStatsDelta(undefined)).toEqual(emptyStatsDelta())
    expect(normalizeStatsDelta('junk')).toEqual(emptyStatsDelta())
    expect(normalizeStatsDelta(null)).toEqual(emptyStatsDelta())
  })
  test('valid delta passes through with integers floored', () => {
    const normalized = normalizeStatsDelta({
      clips: 3.7,
      bytes: 100.9,
      thumbs: 9,
      reports: 1,
      reportMsSum: 1234,
      formats: { prores: 3 },
      cameras: { arri: 3 },
    })
    expect(normalized.clips).toBe(3)
    expect(normalized.bytes).toBe(100)
    expect(normalized.formats).toEqual({ prores: 3 })
  })
  test('negative, NaN, Infinity, and non-number counters collapse to 0', () => {
    const normalized = normalizeStatsDelta({
      clips: -5,
      bytes: Number.NaN,
      thumbs: Number.POSITIVE_INFINITY,
      reports: 'many',
      reportMsSum: {},
      formats: {},
      cameras: {},
    })
    expect(normalized).toEqual(emptyStatsDelta())
  })
  test('counters clamp to their caps', () => {
    const normalized = normalizeStatsDelta({
      clips: 10 ** 9,
      bytes: 10 ** 18,
      thumbs: 10 ** 9,
      reports: 10 ** 9,
      reportMsSum: 10 ** 12,
      formats: {},
      cameras: {},
    })
    expect(normalized.clips).toBe(STATS_CLAMPS.clips)
    expect(normalized.bytes).toBe(STATS_CLAMPS.bytes)
    expect(normalized.thumbs).toBe(STATS_CLAMPS.thumbs)
    expect(normalized.reports).toBe(STATS_CLAMPS.reports)
    expect(normalized.reportMsSum).toBe(STATS_CLAMPS.reportMsSum)
  })
  test('unknown category keys are dropped; values clamp; junk values drop the entry', () => {
    const normalized = normalizeStatsDelta({
      ...emptyStatsDelta(),
      formats: { prores: 2, evil: 99, braw: 10 ** 9 },
      cameras: { arri: 'lots', sony: 4 },
    })
    expect(normalized.formats).toEqual({ prores: 2, braw: STATS_CLAMPS.categoryValue })
    expect(normalized.cameras).toEqual({ sony: 4 })
  })
})
```

- [ ] **Step 3: Run tests to verify they fail**

Run: `cd packages/core && bun test src/stats/delta.test.ts`
Expected: FAIL — `Cannot find module './delta'`

- [ ] **Step 4: Implement `delta.ts`**

```ts
import { CAMERA_MAKERS, FORMAT_FAMILIES } from './categories'

// One anonymous increment batch (spec §3/§4.2): the ONLY thing the client
// ever sends, and the exact shape the server stores. Also reused as the
// GET /api/stats response and the outbox record — one shape everywhere.
export interface StatsDelta {
  clips: number
  bytes: number
  thumbs: number
  reports: number
  reportMsSum: number
  formats: Record<string, number>
  cameras: Record<string, number>
}

// Per-request sanity caps (spec §5.1): a forged request can nudge counters,
// never explode them. Generous for real use — a 20 TB single flush is beyond
// any one card session.
export const STATS_CLAMPS = {
  clips: 20_000,
  bytes: 20 * 2 ** 40, // 20 TB
  thumbs: 200_000,
  reports: 100,
  reportMsSum: 21_600_000, // 6 hours
  categoryValue: 20_000,
} as const

export function emptyStatsDelta(): StatsDelta {
  return { clips: 0, bytes: 0, thumbs: 0, reports: 0, reportMsSum: 0, formats: {}, cameras: {} }
}

export function isEmptyStatsDelta(d: StatsDelta): boolean {
  return (
    d.clips === 0 &&
    d.bytes === 0 &&
    d.thumbs === 0 &&
    d.reports === 0 &&
    d.reportMsSum === 0 &&
    Object.keys(d.formats).length === 0 &&
    Object.keys(d.cameras).length === 0
  )
}

function mergeCategoryMaps(
  a: Record<string, number>,
  b: Record<string, number>,
): Record<string, number> {
  const out: Record<string, number> = { ...a }
  for (const [key, value] of Object.entries(b)) out[key] = (out[key] ?? 0) + value
  return out
}

export function mergeStatsDeltas(a: StatsDelta, b: StatsDelta): StatsDelta {
  return {
    clips: a.clips + b.clips,
    bytes: a.bytes + b.bytes,
    thumbs: a.thumbs + b.thumbs,
    reports: a.reports + b.reports,
    reportMsSum: a.reportMsSum + b.reportMsSum,
    formats: mergeCategoryMaps(a.formats, b.formats),
    cameras: mergeCategoryMaps(a.cameras, b.cameras),
  }
}

function clampCount(value: unknown, max: number): number {
  if (typeof value !== 'number' || !Number.isFinite(value)) return 0
  return Math.min(max, Math.max(0, Math.floor(value)))
}

function normalizeCategoryMap(
  value: unknown,
  allowed: readonly string[],
): Record<string, number> {
  if (typeof value !== 'object' || value === null) return {}
  const out: Record<string, number> = {}
  for (const key of allowed) {
    const count = clampCount((value as Record<string, unknown>)[key], STATS_CLAMPS.categoryValue)
    if (count > 0) out[key] = count
  }
  return out
}

// Defensive read + clamp (spec §5, §8): the server runs this on every ingest
// body, and the client runs it on every outbox load — any past app version or
// hostile payload collapses to a safe, bounded delta.
export function normalizeStatsDelta(raw: unknown): StatsDelta {
  if (typeof raw !== 'object' || raw === null) return emptyStatsDelta()
  const record = raw as Record<string, unknown>
  return {
    clips: clampCount(record.clips, STATS_CLAMPS.clips),
    bytes: clampCount(record.bytes, STATS_CLAMPS.bytes),
    thumbs: clampCount(record.thumbs, STATS_CLAMPS.thumbs),
    reports: clampCount(record.reports, STATS_CLAMPS.reports),
    reportMsSum: clampCount(record.reportMsSum, STATS_CLAMPS.reportMsSum),
    formats: normalizeCategoryMap(record.formats, FORMAT_FAMILIES),
    cameras: normalizeCategoryMap(record.cameras, CAMERA_MAKERS),
  }
}
```

- [ ] **Step 5: Export from `packages/core/src/index.ts`** — add alongside the existing grouped exports (alphabetical file order, matching the file's style):

```ts
export type { CameraMaker, FormatFamily } from './stats/categories'
export { CAMERA_MAKERS, FORMAT_FAMILIES } from './stats/categories'
export type { StatsDelta } from './stats/delta'
export {
  emptyStatsDelta,
  isEmptyStatsDelta,
  mergeStatsDeltas,
  normalizeStatsDelta,
  STATS_CLAMPS,
} from './stats/delta'
```

- [ ] **Step 6: Run tests to verify they pass**

Run: `cd packages/core && bun test src/stats/delta.test.ts`
Expected: PASS (all)

- [ ] **Step 7: Gates + commit**

```bash
bun run lint && bun run typecheck && bun test
git add packages/core/src/stats packages/core/src/index.ts
git commit -m "feat(core): stats delta model with merge and clamped normalization"
```

---

### Task 2: Core category normalizers

**Files:**
- Modify: `packages/core/src/stats/categories.ts`
- Test: `packages/core/src/stats/categories.test.ts`
- Modify: `packages/core/src/index.ts`

**Interfaces:**
- Consumes: `FORMAT_FAMILIES`, `CAMERA_MAKERS` (Task 1).
- Produces (used by Task 7):
  - `formatFamilyOf(extension: string, codec: string | undefined): FormatFamily` — `extension` is a ClipRef extension (leading dot, e.g. `'.braw'`).
  - `cameraMakerOf(camera: string | undefined): CameraMaker | null` — `null` means "no camera metadata, don't count".

- [ ] **Step 1: Write the failing tests**

`packages/core/src/stats/categories.test.ts`:

```ts
import { describe, expect, test } from 'bun:test'
import { cameraMakerOf, formatFamilyOf } from './categories'

describe('formatFamilyOf', () => {
  test('raw formats resolve by extension regardless of codec', () => {
    expect(formatFamilyOf('.braw', undefined)).toBe('braw')
    expect(formatFamilyOf('.r3d', 'REDCODE RAW')).toBe('redcode')
    expect(formatFamilyOf('.ari', undefined)).toBe('arriraw')
    expect(formatFamilyOf('.arx', undefined)).toBe('arriraw')
    expect(formatFamilyOf('.crm', 'Canon RAW')).toBe('canonraw')
  })
  test('generic containers resolve by codec string', () => {
    expect(formatFamilyOf('.mov', 'ProRes 422 HQ')).toBe('prores')
    expect(formatFamilyOf('.mov', 'Apple ProRes RAW')).toBe('prores')
    expect(formatFamilyOf('.mxf', 'DNxHD')).toBe('dnx')
    expect(formatFamilyOf('.mp4', 'AVC')).toBe('h264')
    expect(formatFamilyOf('.mp4', 'H.264')).toBe('h264')
    expect(formatFamilyOf('.mov', 'HEVC')).toBe('hevc')
    expect(formatFamilyOf('.mov', 'H.265')).toBe('hevc')
  })
  test('extension casing and unknowns', () => {
    expect(formatFamilyOf('.BRAW', undefined)).toBe('braw')
    expect(formatFamilyOf('.mov', undefined)).toBe('other')
    expect(formatFamilyOf('.mov', 'MJPEG')).toBe('other')
  })
})

describe('cameraMakerOf', () => {
  test('known makers match case-insensitively as substrings', () => {
    expect(cameraMakerOf('ARRI')).toBe('arri')
    expect(cameraMakerOf('ALEXA Mini (ARRI)')).toBe('arri')
    expect(cameraMakerOf('Blackmagic design')).toBe('blackmagic')
    expect(cameraMakerOf('RED DIGITAL CINEMA')).toBe('red')
    expect(cameraMakerOf('Canon EOS C300')).toBe('canon')
    expect(cameraMakerOf('Sony VENICE')).toBe('sony')
    expect(cameraMakerOf('Panasonic VariCam')).toBe('panasonic')
    expect(cameraMakerOf('FUJIFILM X-H2')).toBe('fujifilm')
    expect(cameraMakerOf('NIKON Z 9')).toBe('nikon')
    expect(cameraMakerOf('DJI Ronin 4D')).toBe('dji')
    expect(cameraMakerOf('GoPro HERO12')).toBe('gopro')
  })
  test('unrecognized camera strings bucket as other', () => {
    expect(cameraMakerOf('Kinefinity MAVO')).toBe('other')
  })
  test('missing or empty camera metadata is not counted at all', () => {
    expect(cameraMakerOf(undefined)).toBeNull()
    expect(cameraMakerOf('')).toBeNull()
    expect(cameraMakerOf('   ')).toBeNull()
  })
})
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cd packages/core && bun test src/stats/categories.test.ts`
Expected: FAIL — `cameraMakerOf is not a function` (or export missing)

- [ ] **Step 3: Implement the functions** (append to `categories.ts`)

```ts
// Extensions whose family is intrinsic — codec strings for these vary by
// metadata source, the container itself is the identity.
const FAMILY_BY_EXTENSION: Record<string, FormatFamily> = {
  '.braw': 'braw',
  '.r3d': 'redcode',
  '.ari': 'arriraw',
  '.arx': 'arriraw',
  '.crm': 'canonraw',
}

export function formatFamilyOf(extension: string, codec: string | undefined): FormatFamily {
  const byExtension = FAMILY_BY_EXTENSION[extension.toLowerCase()]
  if (byExtension) return byExtension
  const c = (codec ?? '').toLowerCase()
  if (c.includes('prores')) return 'prores'
  if (c.includes('dnx')) return 'dnx'
  if (c.includes('hevc') || c.includes('h.265') || c.includes('h265')) return 'hevc'
  if (c.includes('avc') || c.includes('h.264') || c.includes('h264')) return 'h264'
  return 'other'
}

// Ordered so more specific needles never lose to broader ones. 'other' is the
// recognized-but-unlisted bucket; null means "no camera metadata at all" and
// the clip simply doesn't join the camera breakdown.
const MAKER_NEEDLES: readonly [CameraMaker, string][] = [
  ['blackmagic', 'blackmagic'],
  ['arri', 'arri'],
  ['red', 'red'],
  ['canon', 'canon'],
  ['sony', 'sony'],
  ['panasonic', 'panasonic'],
  ['fujifilm', 'fujifilm'],
  ['nikon', 'nikon'],
  ['dji', 'dji'],
  ['gopro', 'gopro'],
]

export function cameraMakerOf(camera: string | undefined): CameraMaker | null {
  const value = (camera ?? '').trim().toLowerCase()
  if (value === '') return null
  for (const [maker, needle] of MAKER_NEEDLES) if (value.includes(needle)) return maker
  return 'other'
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cd packages/core && bun test src/stats/categories.test.ts`
Expected: PASS

- [ ] **Step 5: Export from `packages/core/src/index.ts`** — extend the Task 1 export line:

```ts
export { CAMERA_MAKERS, cameraMakerOf, FORMAT_FAMILIES, formatFamilyOf } from './stats/categories'
```

- [ ] **Step 6: Gates + commit**

```bash
bun run lint && bun run typecheck && bun test
git add packages/core/src/stats packages/core/src/index.ts
git commit -m "feat(core): format-family and camera-maker normalizers for stats"
```

---

### Task 3: Core fingerprints

**Files:**
- Create: `packages/core/src/stats/fingerprint.ts`
- Test: `packages/core/src/stats/fingerprint.test.ts`
- Modify: `packages/core/src/index.ts`

**Interfaces:**
- Consumes: nothing (uses `globalThis.crypto.subtle` — available in Bun, browsers, and Workers; core stays DOM-free).
- Produces (used by Task 7):
  - `sha256Hex(text: string): Promise<string>`
  - `clipFingerprint(fileName: string, sizeBytes: number, durationSeconds: number | undefined): Promise<string>`
  - `reportFingerprint(clipFingerprints: readonly string[]): Promise<string>` — order-insensitive.

- [ ] **Step 1: Write the failing tests**

`packages/core/src/stats/fingerprint.test.ts`:

```ts
import { describe, expect, test } from 'bun:test'
import { clipFingerprint, reportFingerprint, sha256Hex } from './fingerprint'

describe('sha256Hex', () => {
  test('known vector', async () => {
    // SHA-256 of the empty string — the canonical test vector.
    expect(await sha256Hex('')).toBe(
      'e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855',
    )
  })
})

describe('clipFingerprint', () => {
  test('stable for identical identity', async () => {
    expect(await clipFingerprint('A001C001.mov', 1024, 47.5)).toBe(
      await clipFingerprint('A001C001.mov', 1024, 47.5),
    )
  })
  test('any identity component changes the fingerprint', async () => {
    const base = await clipFingerprint('A001C001.mov', 1024, 47.5)
    expect(await clipFingerprint('A001C002.mov', 1024, 47.5)).not.toBe(base)
    expect(await clipFingerprint('A001C001.mov', 2048, 47.5)).not.toBe(base)
    expect(await clipFingerprint('A001C001.mov', 1024, 48)).not.toBe(base)
  })
  test('missing duration is a stable identity of its own', async () => {
    expect(await clipFingerprint('X.braw', 10, undefined)).toBe(
      await clipFingerprint('X.braw', 10, undefined),
    )
    expect(await clipFingerprint('X.braw', 10, undefined)).not.toBe(
      await clipFingerprint('X.braw', 10, 0),
    )
  })
})

describe('reportFingerprint', () => {
  test('order-insensitive over the clip set', async () => {
    expect(await reportFingerprint(['bbb', 'aaa'])).toBe(await reportFingerprint(['aaa', 'bbb']))
  })
  test('adding a clip changes the fingerprint', async () => {
    expect(await reportFingerprint(['aaa', 'bbb'])).not.toBe(
      await reportFingerprint(['aaa', 'bbb', 'ccc']),
    )
  })
})
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cd packages/core && bun test src/stats/fingerprint.test.ts`
Expected: FAIL — `Cannot find module './fingerprint'`

- [ ] **Step 3: Implement `fingerprint.ts`**

```ts
// Anonymous dedup identities (spec §4.1). Computed locally, stored locally in
// the app's reported-set — NEVER transmitted. A fingerprint exists so the
// same clip/report is not counted twice on this machine, nothing more.

export async function sha256Hex(text: string): Promise<string> {
  const digest = await globalThis.crypto.subtle.digest('SHA-256', new TextEncoder().encode(text))
  return Array.from(new Uint8Array(digest), (b) => b.toString(16).padStart(2, '0')).join('')
}

// Identity = name + size + duration ('-' when the format exposes none). Not
// content-hashed on purpose: reading whole clips to fingerprint them would
// cost more than the stat is worth, and this triple is unique in practice.
export function clipFingerprint(
  fileName: string,
  sizeBytes: number,
  durationSeconds: number | undefined,
): Promise<string> {
  return sha256Hex(`clip|${fileName}|${sizeBytes}|${durationSeconds ?? '-'}`)
}

// A report is its clip set: same set → same fingerprint (re-export counts
// once), changed set → new fingerprint (the amended report counts again).
export function reportFingerprint(clipFingerprints: readonly string[]): Promise<string> {
  return sha256Hex(`report|${[...clipFingerprints].sort().join('|')}`)
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cd packages/core && bun test src/stats/fingerprint.test.ts`
Expected: PASS

- [ ] **Step 5: Export from `packages/core/src/index.ts`**

```ts
export { clipFingerprint, reportFingerprint, sha256Hex } from './stats/fingerprint'
```

- [ ] **Step 6: Gates + commit**

```bash
bun run lint && bun run typecheck && bun test
git add packages/core/src/stats packages/core/src/index.ts
git commit -m "feat(core): local dedup fingerprints for clips and reports"
```

---

### Task 4: Settings v3 — `shareUsageStats`

**Files:**
- Modify: `packages/core/src/settings/model.ts`
- Modify: `packages/core/src/settings/model.test.ts`

**Interfaces:**
- Consumes: existing Settings v2 model.
- Produces (used by Tasks 7–9): `Settings.shareUsageStats: boolean`, `SETTINGS_SCHEMA_VERSION = 3`, default `true`, v2→v3 migration preserving all fields.

- [ ] **Step 1: Update the tests to the v3 world**

In `packages/core/src/settings/model.test.ts`:
- In the `defaultSettings` equality test, add `shareUsageStats: true` to the expected object (and expect `schemaVersion: SETTINGS_SCHEMA_VERSION` as it already does).
- Update the v1-migration test's expected object to `schemaVersion: 3` with `generateThumbnails: true, shareUsageStats: true`.
- Everywhere a literal `schemaVersion: 2` record is normalized and expected to pass through, the input stays v2 but expectations gain `shareUsageStats: true` and `schemaVersion: 3` (v2 records migrate).
- Add these new tests:

```ts
test('v2 records migrate: fields preserved, shareUsageStats defaults on', () => {
  const migrated = normalizeSettings({
    schemaVersion: 2,
    workerPoolCap: 6,
    generateThumbnails: false,
    coverDefaults: { dit: 'Shaked' },
  })
  expect(migrated).toEqual({
    schemaVersion: 3,
    workerPoolCap: 6,
    generateThumbnails: false,
    shareUsageStats: true,
    coverDefaults: { dit: 'Shaked' },
  })
})

test('shareUsageStats must be a real boolean; junk falls back to true', () => {
  const base = { schemaVersion: 3, workerPoolCap: 4, generateThumbnails: true, coverDefaults: {} }
  expect(normalizeSettings({ ...base, shareUsageStats: false }).shareUsageStats).toBe(false)
  expect(normalizeSettings({ ...base, shareUsageStats: 'no' }).shareUsageStats).toBe(true)
  expect(normalizeSettings({ ...base }).shareUsageStats).toBe(true)
})
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cd packages/core && bun test src/settings/model.test.ts`
Expected: FAIL — defaults lack `shareUsageStats`, version mismatch

- [ ] **Step 3: Implement v3 in `model.ts`**

- `export const SETTINGS_SCHEMA_VERSION = 3`
- Add to the interface: `shareUsageStats: boolean` (with a comment: `// Anonymous community counters (stats spec §4.3): opt-out, on by default.`)
- Add to `defaultSettings()`: `shareUsageStats: true`
- Add the migration and chain it after v1→v2:

```ts
function migrateV2toV3(record: Record<string, unknown>): Record<string, unknown> {
  return { ...record, schemaVersion: 3, shareUsageStats: true }
}
```

In `normalizeSettings`, after the v1 line: `if (record.schemaVersion === 2) record = migrateV2toV3(record)`, and in the returned object: `shareUsageStats: record.shareUsageStats !== false,`

- [ ] **Step 4: Run tests to verify they pass**

Run: `cd packages/core && bun test src/settings/model.test.ts`
Expected: PASS

- [ ] **Step 5: Gates + commit** (root `typecheck` also proves the app compiles against v3)

```bash
bun run lint && bun run typecheck && bun test
git add packages/core/src/settings
git commit -m "feat(core): settings v3 adds opt-out shareUsageStats flag"
```

---

### Task 5: `apps/stats` Worker — scaffold + pure ingest logic

**Files:**
- Create: `apps/stats/package.json`, `apps/stats/tsconfig.json`, `apps/stats/wrangler.jsonc`, `apps/stats/schema.sql`
- Create: `apps/stats/src/ingest.ts`
- Test: `apps/stats/src/ingest.test.ts`

**Interfaces:**
- Consumes: `StatsDelta` from `@luna-web/core` (Task 1).
- Produces (used by Task 6):
  - `interface StatementSpec { sql: string; params: (string | number)[] }`
  - `ingestStatements(delta: StatsDelta): StatementSpec[]`
  - `statsResponseFrom(counters: readonly { key: string; value: number }[], categories: readonly { kind: string; name: string; value: number }[]): StatsDelta`
- D1 keys: `clips`, `bytes`, `thumbs`, `reports`, `report_ms_sum`; category kinds `format` | `camera`.

- [ ] **Step 1: Scaffold the workspace member**

`apps/stats/package.json` (then run `bun add` for real latest versions — never hand-pin):

```json
{
  "name": "@luna-web/stats",
  "private": true,
  "type": "module",
  "version": "0.0.0",
  "scripts": {
    "dev": "wrangler dev",
    "typecheck": "tsc --noEmit",
    "build": "tsc --noEmit",
    "test": "bun test"
  },
  "dependencies": {
    "@luna-web/core": "workspace:*"
  },
  "devDependencies": {}
}
```

The workspace dep is written by hand (workspace protocol); dev deps install at latest via `bun add`:

```bash
cd apps/stats
bun install
bun add -d wrangler @cloudflare/workers-types @types/bun
```

`apps/stats/tsconfig.json` (mirrors `packages/core/tsconfig.json`):

```json
{
  "extends": "../../tsconfig.base.json",
  "compilerOptions": { "lib": ["ES2022"], "types": ["@cloudflare/workers-types", "bun"] },
  "include": ["src"]
}
```

`apps/stats/wrangler.jsonc`:

```jsonc
{
  "$schema": "./node_modules/wrangler/config-schema.json",
  "name": "luna-stats",
  "main": "src/index.ts",
  "compatibility_date": "2026-07-19",
  // Community-stats API (stats spec §5): two tables of aggregate counters,
  // nothing else. TURNSTILE_SECRET_KEY is a dashboard secret, never config.
  "d1_databases": [
    {
      "binding": "DB",
      "database_name": "luna-stats",
      // Filled from the dashboard after `wrangler d1 create` equivalent —
      // see DEPLOY.md "luna-stats" section.
      "database_id": "TO-BE-FILLED-FROM-DASHBOARD"
    }
  ],
  // Routing is managed manually in the dashboard (see DEPLOY.md) — same
  // convention as the other two Workers.
  "workers_dev": false,
  "preview_urls": false
}
```

`apps/stats/schema.sql`:

```sql
CREATE TABLE IF NOT EXISTS counters (
  key TEXT PRIMARY KEY,
  value INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE IF NOT EXISTS categories (
  kind TEXT NOT NULL,
  name TEXT NOT NULL,
  value INTEGER NOT NULL DEFAULT 0,
  PRIMARY KEY (kind, name)
);
```

- [ ] **Step 2: Write the failing tests**

`apps/stats/src/ingest.test.ts`:

```ts
import { emptyStatsDelta } from '@luna-web/core'
import { describe, expect, test } from 'bun:test'
import { ingestStatements, statsResponseFrom } from './ingest'

describe('ingestStatements', () => {
  test('emits one upsert per non-zero counter and category', () => {
    const statements = ingestStatements({
      clips: 12,
      bytes: 340_000_000,
      thumbs: 96,
      reports: 1,
      reportMsSum: 5400,
      formats: { prores: 10, braw: 2 },
      cameras: { arri: 8 },
    })
    expect(statements).toHaveLength(8)
    expect(statements[0]).toEqual({
      sql: 'INSERT INTO counters (key, value) VALUES (?1, ?2) ON CONFLICT (key) DO UPDATE SET value = value + ?2',
      params: ['clips', 12],
    })
    expect(statements.at(-1)).toEqual({
      sql: 'INSERT INTO categories (kind, name, value) VALUES (?1, ?2, ?3) ON CONFLICT (kind, name) DO UPDATE SET value = value + ?3',
      params: ['camera', 'arri', 8],
    })
  })
  test('zero fields emit nothing; the empty delta emits no statements', () => {
    expect(ingestStatements(emptyStatsDelta())).toEqual([])
    expect(ingestStatements({ ...emptyStatsDelta(), reports: 1 })).toHaveLength(1)
  })
})

describe('statsResponseFrom', () => {
  test('reshapes rows into a StatsDelta, missing counters default to 0', () => {
    const response = statsResponseFrom(
      [
        { key: 'clips', value: 100 },
        { key: 'report_ms_sum', value: 9000 },
      ],
      [
        { kind: 'format', name: 'prores', value: 60 },
        { kind: 'camera', name: 'arri', value: 40 },
        { kind: 'junk', name: 'x', value: 1 },
      ],
    )
    expect(response).toEqual({
      clips: 100,
      bytes: 0,
      thumbs: 0,
      reports: 0,
      reportMsSum: 9000,
      formats: { prores: 60 },
      cameras: { arri: 40 },
    })
  })
})
```

- [ ] **Step 3: Run tests to verify they fail**

Run: `cd apps/stats && bun test`
Expected: FAIL — `Cannot find module './ingest'`

- [ ] **Step 4: Implement `src/ingest.ts`**

```ts
import type { StatsDelta } from '@luna-web/core'

export interface StatementSpec {
  sql: string
  params: (string | number)[]
}

const COUNTER_UPSERT =
  'INSERT INTO counters (key, value) VALUES (?1, ?2) ON CONFLICT (key) DO UPDATE SET value = value + ?2'
const CATEGORY_UPSERT =
  'INSERT INTO categories (kind, name, value) VALUES (?1, ?2, ?3) ON CONFLICT (kind, name) DO UPDATE SET value = value + ?3'

// Pure statement builder — the handler stays thin (stats spec §9) and this
// stays testable without a D1 instance.
export function ingestStatements(delta: StatsDelta): StatementSpec[] {
  const out: StatementSpec[] = []
  const counters: [string, number][] = [
    ['clips', delta.clips],
    ['bytes', delta.bytes],
    ['thumbs', delta.thumbs],
    ['reports', delta.reports],
    ['report_ms_sum', delta.reportMsSum],
  ]
  for (const [key, value] of counters) {
    if (value > 0) out.push({ sql: COUNTER_UPSERT, params: [key, value] })
  }
  for (const [name, value] of Object.entries(delta.formats)) {
    if (value > 0) out.push({ sql: CATEGORY_UPSERT, params: ['format', name, value] })
  }
  for (const [name, value] of Object.entries(delta.cameras)) {
    if (value > 0) out.push({ sql: CATEGORY_UPSERT, params: ['camera', name, value] })
  }
  return out
}

export function statsResponseFrom(
  counters: readonly { key: string; value: number }[],
  categories: readonly { kind: string; name: string; value: number }[],
): StatsDelta {
  const byKey = new Map(counters.map((row) => [row.key, row.value]))
  const formats: Record<string, number> = {}
  const cameras: Record<string, number> = {}
  for (const row of categories) {
    if (row.kind === 'format') formats[row.name] = row.value
    else if (row.kind === 'camera') cameras[row.name] = row.value
  }
  return {
    clips: byKey.get('clips') ?? 0,
    bytes: byKey.get('bytes') ?? 0,
    thumbs: byKey.get('thumbs') ?? 0,
    reports: byKey.get('reports') ?? 0,
    reportMsSum: byKey.get('report_ms_sum') ?? 0,
    formats,
    cameras,
  }
}
```

- [ ] **Step 5: Run tests to verify they pass**

Run: `cd apps/stats && bun test`
Expected: PASS

- [ ] **Step 6: Gates + commit** (root `bun run typecheck` / `bun run build` now include `@luna-web/stats` via the `apps/*` workspace glob)

```bash
bun run lint && bun run typecheck && bun test
git add apps/stats bun.lock
git commit -m "feat(stats): worker scaffold, D1 schema, and pure ingest logic"
```

---

### Task 6: `apps/stats` Worker — fetch handler

**Files:**
- Create: `apps/stats/src/index.ts`

**Interfaces:**
- Consumes: `ingestStatements`, `statsResponseFrom` (Task 5); `normalizeStatsDelta`, `isEmptyStatsDelta` (Task 1).
- Produces (used by Tasks 8, 10):
  - `POST /api/stats/ingest` — body `{ token: string, delta: StatsDelta }` → `204` on success, `403` bad/missing Turnstile token, `400` malformed JSON, `404` other paths/methods.
  - `GET /api/stats` — `StatsDelta`-shaped JSON, `cache-control: public, max-age=60`, `access-control-allow-origin: *`.
- Env: `{ DB: D1Database, TURNSTILE_SECRET_KEY: string }` (secret set in the dashboard, Task 11).

- [ ] **Step 1: Implement `src/index.ts`** (thin handler — all logic already tested in Tasks 1 and 5; Turnstile siteverify per Cloudflare docs)

```ts
import { isEmptyStatsDelta, normalizeStatsDelta } from '@luna-web/core'
import { ingestStatements, statsResponseFrom } from './ingest'

export interface Env {
  DB: D1Database
  TURNSTILE_SECRET_KEY: string
}

const SITEVERIFY_URL = 'https://challenges.cloudflare.com/turnstile/v0/siteverify'

// The endpoint cannot be authenticated (open-source client, no accounts) —
// only defended: Turnstile kills scripted submission, normalizeStatsDelta
// clamps whatever gets through, the dashboard rate-limit rule caps volume
// (stats spec §5.1). Nothing about the request is stored — not even logs.
async function verifyTurnstile(token: unknown, secret: string): Promise<boolean> {
  if (typeof token !== 'string' || token === '') return false
  const response = await fetch(SITEVERIFY_URL, {
    method: 'POST',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify({ secret, response: token }),
  })
  if (!response.ok) return false
  const outcome = (await response.json()) as { success?: boolean }
  return outcome.success === true
}

async function ingest(request: Request, env: Env): Promise<Response> {
  let body: { token?: unknown; delta?: unknown }
  try {
    body = (await request.json()) as { token?: unknown; delta?: unknown }
  } catch {
    return new Response('Bad request', { status: 400 })
  }
  if (!(await verifyTurnstile(body.token, env.TURNSTILE_SECRET_KEY))) {
    return new Response('Forbidden', { status: 403 })
  }
  const delta = normalizeStatsDelta(body.delta)
  if (!isEmptyStatsDelta(delta)) {
    const statements = ingestStatements(delta)
    await env.DB.batch(statements.map((s) => env.DB.prepare(s.sql).bind(...s.params)))
  }
  return new Response(null, { status: 204 })
}

async function getStats(env: Env): Promise<Response> {
  const counters = await env.DB.prepare('SELECT key, value FROM counters').all<{
    key: string
    value: number
  }>()
  const categories = await env.DB.prepare('SELECT kind, name, value FROM categories').all<{
    kind: string
    name: string
    value: number
  }>()
  return Response.json(statsResponseFrom(counters.results, categories.results), {
    headers: {
      'cache-control': 'public, max-age=60',
      'access-control-allow-origin': '*',
    },
  })
}

export default {
  async fetch(request: Request, env: Env): Promise<Response> {
    const url = new URL(request.url)
    if (url.pathname === '/api/stats' && request.method === 'GET') return getStats(env)
    if (url.pathname === '/api/stats/ingest' && request.method === 'POST')
      return ingest(request, env)
    return new Response('Not found', { status: 404 })
  },
}
```

- [ ] **Step 2: Typecheck it compiles against workers-types**

Run: `cd apps/stats && bun run typecheck`
Expected: clean

- [ ] **Step 3: Gates + commit**

```bash
bun run lint && bun run typecheck && bun test
git add apps/stats/src/index.ts
git commit -m "feat(stats): fetch handler with Turnstile verify and D1 upserts"
```

---

### Task 7: Web — DB v5, outbox/reported persistence, telemetry recording

**Files:**
- Modify: `apps/web/src/persistence/db.ts`
- Create: `apps/web/src/persistence/stats.ts`
- Create: `apps/web/src/features/stats/outbox-lock.ts`
- Create: `apps/web/src/features/stats/telemetry.ts`
- Modify: `apps/web/src/features/process/run-processing.ts`
- Modify: `apps/web/src/features/export/exporter.ts`

**Interfaces:**
- Consumes: core Tasks 1–4 exports; `scanStore` state; `settingsStore.state.shareUsageStats`; `logger.debug`; `errorMessage`.
- Produces (used by Tasks 8–9):
  - `persistence/stats.ts`: `loadOutbox(): Promise<StatsDelta>`, `saveOutbox(delta: StatsDelta): Promise<void>`, `clearOutbox(): Promise<void>`, `filterUnreported(fingerprints: readonly string[]): Promise<string[]>`, `markReported(fingerprints: readonly string[]): Promise<void>`
  - `outbox-lock.ts`: `withOutboxLock<T>(fn: () => Promise<T>): Promise<T>` — its own module so telemetry (Task 7) and flush (Task 8) can both use it without a telemetry⇄flush import cycle.
  - `telemetry.ts`: `recordProcessedRun(): Promise<void>`, `recordReportExport(report: ReportModel<Blob>, generateMs: number): Promise<void>`
  - `telemetry.ts` calls `flushOutbox()` from `./flush` (Task 8) — Task 7 creates `flush.ts` as a stub `export async function flushOutbox(): Promise<void> {}` that Task 8 replaces. Flush never imports telemetry, so there is no cycle.
- No unit tests: this layer is IndexedDB/store glue in the pattern of `persistence/activity.ts` (untested glue over tested core logic). All counting/merging/clamping logic already has core tests.

- [ ] **Step 1: DB v5** — in `apps/web/src/persistence/db.ts`:

Add to the `LunaDb` interface:

```ts
  // Stats dedup (stats spec §4.1): fingerprints already counted on this
  // machine. Key = fingerprint hex, value = 1 (presence is the datum).
  statsReported: { key: string; value: number }
  // One record ('pending') holding the merged StatsDelta outbox; `unknown`
  // because normalizeStatsDelta is the only trusted reader.
  statsOutbox: { key: string; value: unknown }
```

Bump `openDB<LunaDb>(DB_NAME, 4, …)` to `5` and add to `upgrade`:

```ts
      if (oldVersion < 5) {
        db.createObjectStore('statsReported')
        db.createObjectStore('statsOutbox')
      }
```

- [ ] **Step 2: Create `apps/web/src/persistence/stats.ts`**

```ts
import { normalizeStatsDelta, type StatsDelta } from '@luna-web/core'
import { getDb } from './db'

const OUTBOX_KEY = 'pending'

export async function loadOutbox(): Promise<StatsDelta> {
  const db = await getDb()
  return normalizeStatsDelta(await db.get('statsOutbox', OUTBOX_KEY))
}

export async function saveOutbox(delta: StatsDelta): Promise<void> {
  const db = await getDb()
  await db.put('statsOutbox', delta, OUTBOX_KEY)
}

export async function clearOutbox(): Promise<void> {
  const db = await getDb()
  await db.delete('statsOutbox', OUTBOX_KEY)
}

export async function filterUnreported(fingerprints: readonly string[]): Promise<string[]> {
  const db = await getDb()
  const fresh: string[] = []
  for (const fp of fingerprints) {
    if ((await db.get('statsReported', fp)) === undefined) fresh.push(fp)
  }
  return fresh
}

export async function markReported(fingerprints: readonly string[]): Promise<void> {
  const db = await getDb()
  const tx = db.transaction('statsReported', 'readwrite')
  for (const fp of fingerprints) void tx.store.put(1, fp)
  await tx.done
}
```

- [ ] **Step 3: Create the flush stub** `apps/web/src/features/stats/flush.ts` (replaced wholesale in Task 8):

```ts
// Replaced in the flush task — recording must not dead-import.
export async function flushOutbox(): Promise<void> {}
```

- [ ] **Step 4: Create `apps/web/src/features/stats/outbox-lock.ts`**

```ts
// Serializes every outbox read-modify-write (recording AND flushing) so a
// flush can never clear increments that landed between its load and its
// clear. Single-tab in practice; this guards the async interleavings.
// Standalone module: telemetry and flush both use it, and flush must not
// import telemetry (or vice versa beyond the one-way flush trigger).
let chain: Promise<unknown> = Promise.resolve()

export function withOutboxLock<T>(fn: () => Promise<T>): Promise<T> {
  const next = chain.then(fn, fn)
  chain = next.catch(() => undefined)
  return next
}
```

- [ ] **Step 5: Create `apps/web/src/features/stats/telemetry.ts`**

```ts
import {
  cameraMakerOf,
  clipFingerprint,
  emptyStatsDelta,
  formatFamilyOf,
  mergeStatsDeltas,
  type ReportModel,
  reportFingerprint,
  type StatsDelta,
} from '@luna-web/core'
import { errorMessage } from '@/lib/errors'
import { logger } from '@/lib/logger'
import { filterUnreported, loadOutbox, markReported, saveOutbox } from '@/persistence/stats'
import { scanStore } from '../scan/store'
import { settingsStore } from '../settings/settings-store'
import { flushOutbox } from './flush'
import { withOutboxLock } from './outbox-lock'

async function accumulate(delta: StatsDelta, fingerprints: readonly string[]): Promise<void> {
  await withOutboxLock(async () => {
    const pending = await loadOutbox()
    await saveOutbox(mergeStatsDeltas(pending, delta))
    await markReported(fingerprints)
  })
  void flushOutbox()
}

// Called once per completed processing run (phase 'processed'). Counts only
// never-before-seen clips (stats spec §4.1) — a re-scan contributes nothing.
// A failure here must never disturb the actual work: catch-all by design.
export async function recordProcessedRun(): Promise<void> {
  if (!settingsStore.state.shareUsageStats) return
  try {
    const s = scanStore.state
    if (s.phase !== 'processed') return
    const done = s.clips.filter((c) => s.clipStatus[c.id] === 'done')
    if (done.length === 0) return
    const withFps = await Promise.all(
      done.map(async (clip) => ({
        clip,
        fp: await clipFingerprint(
          clip.fileName,
          clip.sizeBytes,
          s.metadataById[clip.id]?.durationSeconds,
        ),
      })),
    )
    const fresh = new Set(await filterUnreported(withFps.map((entry) => entry.fp)))
    if (fresh.size === 0) return
    const delta = emptyStatsDelta()
    const counted = new Set<string>()
    for (const { clip, fp } of withFps) {
      if (!fresh.has(fp) || counted.has(fp)) continue
      counted.add(fp)
      delta.clips += 1
      delta.bytes += clip.sizeBytes
      delta.thumbs += (s.thumbsById[clip.id] ?? []).filter((f) => f.outcome === 'Success').length
      const meta = s.metadataById[clip.id]
      const family = formatFamilyOf(clip.extension, meta?.codec)
      delta.formats[family] = (delta.formats[family] ?? 0) + 1
      const maker = cameraMakerOf(meta?.camera)
      if (maker) delta.cameras[maker] = (delta.cameras[maker] ?? 0) + 1
    }
    await accumulate(delta, [...counted])
    logger.debug('Stats recorded', `${delta.clips} new clips`)
  } catch (err) {
    logger.debug('Stats recording skipped', errorMessage(err))
  }
}

// Called after an export blob is generated. Same report (same clip set) twice
// → one count; an amended report is a new clip set and counts again.
export async function recordReportExport(
  report: ReportModel<Blob>,
  generateMs: number,
): Promise<void> {
  if (!settingsStore.state.shareUsageStats) return
  try {
    const clips = report.reels.flatMap((reel) => reel.clips)
    if (clips.length === 0) return
    const clipFps = await Promise.all(
      clips.map((c) => clipFingerprint(c.fileName, c.sizeBytes, c.metadata.durationSeconds)),
    )
    const fp = await reportFingerprint(clipFps)
    const fresh = await filterUnreported([fp])
    if (fresh.length === 0) return
    const delta = emptyStatsDelta()
    delta.reports = 1
    delta.reportMsSum = Math.max(0, Math.round(generateMs))
    await accumulate(delta, fresh)
    logger.debug('Stats recorded', 'new report')
  } catch (err) {
    logger.debug('Stats recording skipped', errorMessage(err))
  }
}
```

- [ ] **Step 6: Hook the processing run** — in `apps/web/src/features/process/run-processing.ts`, add the import `import { recordProcessedRun } from '../stats/telemetry'` and append as the LAST statement of `startProcessing()` (after the thumbnail try/catch):

```ts
  // Fire-and-forget by design: stats must never block or fail the pipeline.
  if (isRunCurrent(run) && scanStore.state.phase === 'processed') void recordProcessedRun()
```

(`scanStore` is already imported there.)

- [ ] **Step 7: Hook the export** — in `apps/web/src/features/export/exporter.ts`, import `import { recordReportExport } from '../stats/telemetry'` and change `runExport`'s try block to time generation:

```ts
  try {
    const started = performance.now()
    const blob = await exporter.generate(report)
    // Fire-and-forget by design: stats must never block or fail the export.
    void recordReportExport(report, performance.now() - started)
    await saveBlob(
      blob,
      reportFileName(report.cover.projectTitle, report.cover.date, exporter.extension),
      exporter.mime,
    )
    logger.info(`Export finished: ${exporter.label}`)
  } catch (err) {
```

- [ ] **Step 8: Gates + commit**

```bash
bun run lint && bun run typecheck && bun test
git add apps/web/src/persistence apps/web/src/features/stats apps/web/src/features/process/run-processing.ts apps/web/src/features/export/exporter.ts
git commit -m "feat(web): local stats recording with fingerprint dedup and offline outbox"
```

---

### Task 8: Web — flush pipeline (Turnstile + POST) and boot wiring

**Files:**
- Create: `apps/web/src/features/stats/config.ts`
- Modify (replace stub): `apps/web/src/features/stats/flush.ts`
- Modify: `apps/web/src/vite-env.d.ts`
- Modify: `apps/web/src/main.tsx`

**Interfaces:**
- Consumes: `withOutboxLock` (Task 7), `loadOutbox`/`clearOutbox` (Task 7), `isEmptyStatsDelta` (Task 1), Task 6's `POST /api/stats/ingest` contract.
- Produces (used by Tasks 9–10):
  - `config.ts`: `TURNSTILE_SITE_KEY: string`, `STATS_API_BASE: string | null` (`''` in prod = same-origin; `null` in dev unless `VITE_LUNA_STATS_API` is set — dev sessions never pollute production counters by default).
  - `flush.ts`: `flushOutbox(): Promise<void>`, `initStatsFlush(): void`.

- [ ] **Step 1: Create `config.ts`**

```ts
// Turnstile sitekey is public by design (it ships in every page that renders
// a widget). Placeholder until the widget exists — see DEPLOY.md "luna-stats".
export const TURNSTILE_SITE_KEY = 'TO-BE-FILLED-FROM-DASHBOARD'

// Same-origin in production (the /api/stats* route lives on luna.ozer2.one).
// Dev sends nothing unless explicitly pointed at a stats worker — local runs
// must not pollute the community counters.
export const STATS_API_BASE: string | null = import.meta.env.DEV
  ? (import.meta.env.VITE_LUNA_STATS_API ?? null)
  : ''
```

- [ ] **Step 2: Type the env var** — in `apps/web/src/vite-env.d.ts` append:

```ts
interface ImportMetaEnv {
  readonly VITE_LUNA_STATS_API?: string
}
```

- [ ] **Step 3: Replace `flush.ts`** (complete file — replaces the Task 7 stub wholesale)

```ts
import { isEmptyStatsDelta, type StatsDelta } from '@luna-web/core'
import { errorMessage } from '@/lib/errors'
import { logger } from '@/lib/logger'
import { clearOutbox, loadOutbox, saveOutbox } from '@/persistence/stats'
import { settingsStore } from '../settings/settings-store'
import { STATS_API_BASE, TURNSTILE_SITE_KEY } from './config'
import { withOutboxLock } from './outbox-lock'

const TURNSTILE_SCRIPT = 'https://challenges.cloudflare.com/turnstile/v0/api.js?render=explicit'
const TOKEN_TIMEOUT_MS = 30_000

declare global {
  interface Window {
    turnstile?: {
      render: (
        container: HTMLElement,
        params: {
          sitekey: string
          callback: (token: string) => void
          'error-callback': () => void
        },
      ) => string
      remove: (widgetId: string) => void
    }
  }
}

// The Turnstile script is the feature's ONLY third-party script, loaded
// lazily at first flush — never at app boot, never when opted out
// (stats spec §4.3, privacy page).
let scriptPromise: Promise<void> | null = null
function loadTurnstileScript(): Promise<void> {
  scriptPromise ??= new Promise((resolve, reject) => {
    const script = document.createElement('script')
    script.src = TURNSTILE_SCRIPT
    script.async = true
    script.onload = () => resolve()
    script.onerror = () => {
      scriptPromise = null // allow a retry on the next flush
      reject(new Error('Turnstile script failed to load'))
    }
    document.head.appendChild(script)
  })
  return scriptPromise
}

// Invisible-mode widget (dashboard-configured): render auto-executes the
// challenge headlessly and hands back a single-use token via callback.
async function getTurnstileToken(): Promise<string> {
  await loadTurnstileScript()
  return new Promise((resolve, reject) => {
    const turnstile = window.turnstile
    if (!turnstile) {
      reject(new Error('Turnstile API unavailable after script load'))
      return
    }
    const container = document.createElement('div')
    document.body.appendChild(container)
    let widgetId: string | undefined
    const timer = setTimeout(() => {
      cleanup()
      reject(new Error('Turnstile token timed out'))
    }, TOKEN_TIMEOUT_MS)
    const cleanup = () => {
      clearTimeout(timer)
      if (widgetId !== undefined) turnstile.remove(widgetId)
      container.remove()
    }
    widgetId = turnstile.render(container, {
      sitekey: TURNSTILE_SITE_KEY,
      callback: (token) => {
        cleanup()
        resolve(token)
      },
      'error-callback': () => {
        cleanup()
        reject(new Error('Turnstile challenge failed'))
      },
    })
  })
}

// Returns latest − flushed, or null when nothing is left. Negative results
// clamp to 0 — the outbox only ever grows between load and clear.
function subtractFlushed(latest: StatsDelta, flushed: StatsDelta): StatsDelta | null {
  const subtractMap = (a: Record<string, number>, b: Record<string, number>) => {
    const out: Record<string, number> = {}
    for (const [key, value] of Object.entries(a)) {
      const remaining = value - (b[key] ?? 0)
      if (remaining > 0) out[key] = remaining
    }
    return out
  }
  const leftover: StatsDelta = {
    clips: Math.max(0, latest.clips - flushed.clips),
    bytes: Math.max(0, latest.bytes - flushed.bytes),
    thumbs: Math.max(0, latest.thumbs - flushed.thumbs),
    reports: Math.max(0, latest.reports - flushed.reports),
    reportMsSum: Math.max(0, latest.reportMsSum - flushed.reportMsSum),
    formats: subtractMap(latest.formats, flushed.formats),
    cameras: subtractMap(latest.cameras, flushed.cameras),
  }
  return isEmptyStatsDelta(leftover) ? null : leftover
}

// Fire-and-forget flush (stats spec §4.2): any failure leaves the outbox
// intact for the next trigger (boot, 'online', new activity). Clearing
// happens under the outbox lock so concurrent recording is never lost.
export async function flushOutbox(): Promise<void> {
  if (STATS_API_BASE === null) return
  if (!settingsStore.state.shareUsageStats) return
  if (!navigator.onLine) return
  try {
    const pending = await withOutboxLock(loadOutbox)
    if (isEmptyStatsDelta(pending)) return
    const token = await getTurnstileToken()
    const response = await fetch(`${STATS_API_BASE}/api/stats/ingest`, {
      method: 'POST',
      headers: { 'content-type': 'application/json' },
      body: JSON.stringify({ token, delta: pending }),
    })
    if (!response.ok) {
      logger.debug('Stats flush deferred', `HTTP ${response.status}`)
      return
    }
    await withOutboxLock(async () => {
      // Only the flushed increments are gone; anything recorded since this
      // flush started was merged into the outbox under the lock and survives.
      const latest = await loadOutbox()
      await clearOutbox()
      const leftover = subtractFlushed(latest, pending)
      if (leftover) await saveOutbox(leftover)
    })
  } catch (err) {
    logger.debug('Stats flush deferred', errorMessage(err))
  }
}

export function initStatsFlush(): void {
  void flushOutbox()
  window.addEventListener('online', () => void flushOutbox())
}
```

- [ ] **Step 4: Boot wiring** — in `apps/web/src/main.tsx`, import `initStatsFlush` from `./features/stats/flush` and inside the existing `.then(() => { … })` callback, before `createRoot(...)`:

```ts
  initStatsFlush() // after settings hydrate: flush respects the opt-out
```

- [ ] **Step 5: Gates + commit**

```bash
bun run lint && bun run typecheck && bun test
git add apps/web/src/features/stats apps/web/src/vite-env.d.ts apps/web/src/main.tsx
git commit -m "feat(web): outbox flush with invisible Turnstile token"
```

---

### Task 9: Settings — Privacy card with the opt-out toggle

**Files:**
- Modify: `apps/web/src/features/settings/settings-screen.tsx`

**Interfaces:**
- Consumes: `settingsStore`, `updateSettings`, `shareUsageStats` (Task 4), `clearOutbox` (Task 7), existing `Card`/`Label` primitives.
- Produces: user-visible opt-out. Turning it off also empties the outbox immediately (spec §4.3).

- [ ] **Step 1: Add the card** — after the "Report defaults" card and before the existing danger/clear-data card, following the exact checkbox pattern already used for `generateThumbnails`:

```tsx
      <Card>
        <CardHeader>
          <CardTitle>Anonymous usage counters</CardTitle>
          <CardDescription>
            Adds your processing to the community totals on the Stats page — plain counts only
            (clips, data size, thumbnails, reports). Never file names, paths, metadata values, or
            anything that identifies you.{' '}
            <a href="/docs/privacy/" className="text-foreground underline underline-offset-4">
              What exactly is sent
            </a>
          </CardDescription>
        </CardHeader>
        <CardContent>
          <label className="text-muted-foreground flex w-fit cursor-pointer items-center gap-2 text-sm">
            <input
              type="checkbox"
              className="accent-primary size-4"
              checked={shareUsageStats}
              onChange={(e) => {
                const next = e.currentTarget.checked
                void updateSettings({ shareUsageStats: next })
                if (!next) void clearOutbox()
              }}
            />
            Share anonymous usage counters
          </label>
        </CardContent>
      </Card>
```

With the selector near the others: `const shareUsageStats = useSelector(settingsStore, (s) => s.shareUsageStats)` and import `clearOutbox` from `@/persistence/stats`.

- [ ] **Step 2: Verify in the browser** — start the dev server (preview tools, launch config `@luna-web/app` dev), open `/settings/`, confirm: card renders, toggle flips and persists across reload (IndexedDB `settings` record shows `shareUsageStats`), no console errors.

- [ ] **Step 3: Gates + commit**

```bash
bun run lint && bun run typecheck && bun test
git add apps/web/src/features/settings/settings-screen.tsx
git commit -m "feat(web): anonymous usage counters opt-out in settings"
```

---

### Task 10: `/stats` page + nav link

**Files:**
- Create: `apps/web/src/features/stats/stats-screen.tsx`
- Create: `apps/web/src/routes/stats.tsx`
- Modify: `apps/web/src/components/app-shell.tsx`

**Interfaces:**
- Consumes: `GET /api/stats` (Task 6 shape), `STATS_API_BASE` (Task 8), `StatsDelta` type (Task 1), `formatBytes`/`formatDuration` from `@/lib/format`, `Card` primitives. Deliberately does NOT use `normalizeStatsDelta` for the response — its ingest clamps would cap the all-time totals.
- Produces: routed `/stats/` page, nav entry.

- [ ] **Step 1: Create `stats-screen.tsx`**

```tsx
import type { StatsDelta } from '@luna-web/core'
import { useEffect, useState } from 'react'
import { Card, CardContent } from '@/components/ui/card'
import { formatBytes, formatDuration } from '@/lib/format'
import { STATS_API_BASE } from './config'

const REFRESH_MS = 60_000

// Defensive read of the GET /api/stats body. Deliberately NOT
// normalizeStatsDelta: that function applies per-request ingest clamps, and
// community all-time totals are exactly the numbers meant to exceed them.
function toNonNegative(value: unknown): number {
  return typeof value === 'number' && Number.isFinite(value) && value >= 0 ? value : 0
}

function toCategoryMap(value: unknown): Record<string, number> {
  if (typeof value !== 'object' || value === null) return {}
  const out: Record<string, number> = {}
  for (const [key, count] of Object.entries(value as Record<string, unknown>)) {
    const n = toNonNegative(count)
    if (n > 0) out[key] = n
  }
  return out
}

function toDisplayStats(raw: unknown): StatsDelta {
  const record = (typeof raw === 'object' && raw !== null ? raw : {}) as Record<string, unknown>
  return {
    clips: toNonNegative(record.clips),
    bytes: toNonNegative(record.bytes),
    thumbs: toNonNegative(record.thumbs),
    reports: toNonNegative(record.reports),
    reportMsSum: toNonNegative(record.reportMsSum),
    formats: toCategoryMap(record.formats),
    cameras: toCategoryMap(record.cameras),
  }
}

// Display labels for the closed category vocabularies (core categories.ts).
// Plain format/manufacturer names — nothing coined.
const FORMAT_LABELS: Record<string, string> = {
  prores: 'ProRes',
  braw: 'Blackmagic RAW',
  redcode: 'REDCODE',
  arriraw: 'ARRIRAW',
  canonraw: 'Canon RAW',
  h264: 'H.264',
  hevc: 'H.265/HEVC',
  dnx: 'DNxHD/HR',
  other: 'Other',
}
const CAMERA_LABELS: Record<string, string> = {
  arri: 'ARRI',
  red: 'RED',
  blackmagic: 'Blackmagic',
  canon: 'Canon',
  sony: 'Sony',
  panasonic: 'Panasonic',
  fujifilm: 'Fujifilm',
  nikon: 'Nikon',
  dji: 'DJI',
  gopro: 'GoPro',
  other: 'Other',
}

function useCountUp(target: number): number {
  const [value, setValue] = useState(0)
  useEffect(() => {
    if (target <= 0) {
      setValue(0)
      return
    }
    let raf = 0
    const started = performance.now()
    const duration = 900
    const tick = (now: number) => {
      const t = Math.min(1, (now - started) / duration)
      setValue(Math.round(target * (1 - (1 - t) ** 3)))
      if (t < 1) raf = requestAnimationFrame(tick)
    }
    raf = requestAnimationFrame(tick)
    return () => cancelAnimationFrame(raf)
  }, [target])
  return value
}

function StatTile({ label, value, display }: { label: string; value: number; display?: string }) {
  const animated = useCountUp(value)
  return (
    <Card>
      <CardContent className="flex flex-col gap-1 pt-6">
        <span className="text-3xl font-semibold tracking-tight tabular-nums">
          {display ?? animated.toLocaleString()}
        </span>
        <span className="text-muted-foreground text-sm">{label}</span>
      </CardContent>
    </Card>
  )
}

function Breakdown({
  title,
  entries,
  labels,
}: {
  title: string
  entries: Record<string, number>
  labels: Record<string, string>
}) {
  const sorted = Object.entries(entries).sort((a, b) => b[1] - a[1])
  if (sorted.length === 0) return null
  const max = sorted[0][1]
  return (
    <Card>
      <CardContent className="flex flex-col gap-3 pt-6">
        <h2 className="text-sm font-medium">{title}</h2>
        {sorted.map(([key, count]) => (
          <div key={key} className="flex items-center gap-3">
            <span className="text-muted-foreground w-32 shrink-0 truncate text-sm">
              {labels[key] ?? key}
            </span>
            <div className="bg-muted h-2 flex-1 overflow-hidden rounded-full">
              <div
                className="bg-primary h-full rounded-full"
                style={{ width: `${Math.max(2, Math.round((count / max) * 100))}%` }}
              />
            </div>
            <span className="w-16 shrink-0 text-right text-sm tabular-nums">
              {count.toLocaleString()}
            </span>
          </div>
        ))}
      </CardContent>
    </Card>
  )
}

type LoadState = 'loading' | 'ready' | 'unreachable'

export function StatsScreen() {
  const [stats, setStats] = useState<StatsDelta | null>(null)
  const [state, setState] = useState<LoadState>('loading')

  useEffect(() => {
    if (STATS_API_BASE === null) {
      setState('unreachable')
      return
    }
    let cancelled = false
    const load = async () => {
      try {
        const response = await fetch(`${STATS_API_BASE}/api/stats`)
        if (!response.ok) throw new Error(`HTTP ${response.status}`)
        const body = toDisplayStats(await response.json())
        if (cancelled) return
        setStats(body)
        setState('ready')
      } catch {
        if (!cancelled) setState((prev) => (prev === 'ready' ? prev : 'unreachable'))
      }
    }
    void load()
    const timer = setInterval(() => void load(), REFRESH_MS)
    return () => {
      cancelled = true
      clearInterval(timer)
    }
  }, [])

  const avgSeconds =
    stats && stats.reports > 0 ? Math.round(stats.reportMsSum / stats.reports / 1000) : null

  return (
    <div className="mx-auto flex max-w-3xl flex-col gap-6">
      <div className="space-y-1">
        <h1 className="text-2xl font-semibold tracking-tight">Community stats</h1>
        <p className="text-muted-foreground text-sm">
          What Luna has processed across everyone using it — anonymous counters, updated every
          minute.{' '}
          <a href="/docs/privacy/" className="text-foreground underline underline-offset-4">
            How this works
          </a>
        </p>
      </div>

      {state === 'loading' && <p className="text-muted-foreground text-sm">Loading…</p>}
      {state === 'unreachable' && (
        <p className="text-muted-foreground text-sm">
          Community stats are unreachable right now. They'll be back — your work is unaffected.
        </p>
      )}

      {state === 'ready' && stats && (
        <>
          <div className="grid grid-cols-2 gap-4 sm:grid-cols-3">
            <StatTile label="Clips read" value={stats.clips} />
            <StatTile label="Data processed" value={stats.bytes} display={formatBytes(stats.bytes)} />
            <StatTile label="Thumbnails generated" value={stats.thumbs} />
            <StatTile label="Reports created" value={stats.reports} />
            <StatTile
              label="Avg report generation"
              value={avgSeconds ?? 0}
              display={avgSeconds === null ? '—' : formatDuration(avgSeconds)}
            />
          </div>
          <Breakdown title="By format" entries={stats.formats} labels={FORMAT_LABELS} />
          <Breakdown title="By camera" entries={stats.cameras} labels={CAMERA_LABELS} />
        </>
      )}
    </div>
  )
}
```

- [ ] **Step 2: Create the route** `apps/web/src/routes/stats.tsx`:

```tsx
import { createFileRoute } from '@tanstack/react-router'
import { StatsScreen } from '@/features/stats/stats-screen'

export const Route = createFileRoute('/stats')({
  component: StatsScreen,
})
```

- [ ] **Step 3: Nav link** — in `apps/web/src/components/app-shell.tsx`, after the Reports link:

```tsx
            <Link to="/stats/" className={cn(buttonVariants({ variant: 'ghost', size: 'sm' }))}>
              Stats
            </Link>
```

- [ ] **Step 4: Verify in the browser** — dev server, open `/stats/`. Without `VITE_LUNA_STATS_API` expect the unreachable placeholder (correct dev behavior); nav link present; no console errors. Optionally run `bun run dev` in `apps/stats` (needs a local D1 — `wrangler dev` provisions one automatically; apply `schema.sql` with `bunx wrangler d1 execute luna-stats --local --file schema.sql`) and set `VITE_LUNA_STATS_API=http://localhost:8787` in `apps/web/.env.local` to see live tiles. Screenshot for the record.

- [ ] **Step 5: Gates + commit** (`typecheck` regenerates `routeTree.gen.ts` via `tsr generate` — commit it too)

```bash
bun run lint && bun run typecheck && bun test
git add apps/web/src/features/stats apps/web/src/routes/stats.tsx apps/web/src/components/app-shell.tsx apps/web/src/routeTree.gen.ts
git commit -m "feat(web): community stats page"
```

---

### Task 11: Privacy docs, taglines, DEPLOY.md

**Files:**
- Modify: `apps/docs/src/content/docs/privacy.md`
- Modify: `README.md` (line 7 tagline)
- Modify: `apps/docs/astro.config.mjs` (description)
- Modify: `apps/web/index.html` (three meta descriptions)
- Modify: `DEPLOY.md` (luna-stats section)

**Interfaces:** none — copy only. Follow the "write for the user" rule: concrete facts a DIT needs, no internal-debate framing.

- [ ] **Step 1: Rewrite `privacy.md`** — replace the whole body under the frontmatter (keep frontmatter, update description):

```markdown
---
title: Privacy
description: How local processing works and what the anonymous usage counters are.
---

Luna reads your footage directly from disk with the File System Access API and processes it in
your browser. It never uploads footage, metadata, or reports.

The only things it downloads are the app itself and its decoding engines — the FFmpeg core
(~31 MB, from a CDN) and MediaInfo (~2.5 MB) — each fetched once and then cached. After that
first load, Luna works offline.

Reports are saved to a location you choose. Luna keeps nothing.

## Anonymous usage counters

Luna adds its work to the community totals on the [Stats page](https://luna.ozer2.one/stats/).
One request carries a handful of numbers and nothing else. A real payload looks like this:

​```json
{
  "clips": 24,
  "bytes": 412316860416,
  "thumbs": 192,
  "reports": 1,
  "reportMsSum": 5400,
  "formats": { "prores": 18, "braw": 6 },
  "cameras": { "arri": 18, "blackmagic": 6 }
}
​```

Format and camera names come from a fixed list (ProRes, BRAW, REDCODE, ARRI, Sony, …);
anything else is bucketed as "other".

**Never sent:** file names, file paths, folder names, footage, metadata values, report
contents, or timecodes. There are no cookies, no accounts, and no device or session IDs, and
the server stores no IP addresses. The counters cannot be traced to a person, a project, or a
card.

**When it's sent:** whenever Luna is online — work done offline is kept as a pending sum on
your device and sent on the next launch with connectivity. Each send passes a Cloudflare
Turnstile bot check; that script loads only at send time and is the feature's only
third-party script.

**Counted once:** Luna keeps private checksums on your device of what it already counted.
Re-scanning the same card or re-exporting the same report adds nothing; only new clips and
changed reports count.

**Opt out:** Settings → "Share anonymous usage counters". When off, nothing is counted,
stored, or sent, and any pending sum is deleted.
```

(The `​```json` fences above are written with a zero-width guard in this plan only — in the real file they are plain triple-backtick fences.)

- [ ] **Step 2: Tagline sweep** — change `Camera reports in your browser. Nothing leaves your device.` to `Camera reports in your browser. Your footage never leaves your device.` in:
  - `README.md` line 7
  - `apps/docs/astro.config.mjs` `description`
  - `apps/web/index.html` — all three (`name="description"`, `property="og:description"`, `name="twitter:description"`)

- [ ] **Step 3: README feature line** — in README's "What Luna Does" list, append:

```markdown
- Community stats: anonymous aggregate counters ([opt-out](https://luna.ozer2.one/docs/privacy/)) power a public stats page
```

- [ ] **Step 4: DEPLOY.md** — add a `luna-stats` column/section mirroring the existing two-worker table plus its own setup list:

```markdown
## Worker: `luna-stats` (community stats API)

Third Worker, same pattern: dashboard Git integration, manual routing.

| Setting | `luna-stats` |
|---|---|
| Root directory | `apps/stats` |
| Build command | `bun run build` |
| Deploy command | `npx wrangler deploy` (default) |
| Non-production branch deploy | `npx wrangler versions upload` (default) |
| Build watch paths | `apps/stats/**`, `packages/core/**`, `bun.lock` |
| Production branch | `master` |

One-time setup (dashboard + one local command):

1. **D1**: Storage & Databases → D1 → create `luna-stats`. Copy its ID into
   `apps/stats/wrangler.jsonc` `database_id` and commit.
2. **Schema**: `bunx wrangler d1 execute luna-stats --remote --file apps/stats/schema.sql`
   (requires `wrangler login`).
3. **Turnstile**: Turnstile → Add widget — domain `luna.ozer2.one`, mode **Invisible**.
   Sitekey goes into `apps/web/src/features/stats/config.ts` (`TURNSTILE_SITE_KEY`, commit);
   the secret key becomes the Worker secret `TURNSTILE_SECRET_KEY`
   (Worker → Settings → Variables and Secrets) after the first deploy.
4. **Route**: `luna.ozer2.one/api/stats*` → `luna-stats` (manual, like the other Workers).
5. **Rate limiting**: zone → Security → WAF → Rate limiting rules — path starts with
   `/api/stats/ingest`, 10 requests per minute per IP, block.
```

- [ ] **Step 5: Gates + commit**

```bash
bun run lint && bun run typecheck
git add apps/docs/src/content/docs/privacy.md README.md apps/docs/astro.config.mjs apps/web/index.html DEPLOY.md
git commit -m "docs: privacy page covers anonymous usage counters; taglines say footage"
```

---

### Task 12: Dashboard provisioning (Shaked) + end-to-end verification + release

**Files:**
- Modify: `apps/stats/wrangler.jsonc` (real `database_id`)
- Modify: `apps/web/src/features/stats/config.ts` (real `TURNSTILE_SITE_KEY`)

This task interleaves Shaked's dashboard work with code finalization. Nothing here is CI-automatable by design (DEPLOY.md convention: routing/resources are manual).

- [ ] **Step 1 (Shaked, dashboard):** run DEPLOY.md "luna-stats" one-time setup items 1–3 created in Task 11: create D1 `luna-stats`, apply `schema.sql` remotely, create the Invisible Turnstile widget. Hand back: `database_id`, sitekey.

- [ ] **Step 2 (code):** paste `database_id` into `apps/stats/wrangler.jsonc` and the sitekey into `apps/web/src/features/stats/config.ts`. Commit:

```bash
bun run lint && bun run typecheck
git add apps/stats/wrangler.jsonc apps/web/src/features/stats/config.ts
git commit -m "chore(stats): wire dashboard-provisioned D1 id and Turnstile sitekey"
git push
```

- [ ] **Step 3 (Shaked, dashboard):** after the push deploys — connect the `luna-stats` Workers Builds project (Task 11 table), set the `TURNSTILE_SECRET_KEY` secret, add the `/api/stats*` route, add the rate-limiting rule (setup items 4–5).

- [ ] **Step 4: Verify the API from here:**

```bash
curl -s https://luna.ozer2.one/api/stats
# Expected: {"clips":0,"bytes":0,...} with zeros (fresh DB)
curl -s -o /dev/null -w '%{http_code}' -X POST https://luna.ozer2.one/api/stats/ingest -H 'content-type: application/json' -d '{"token":"forged","delta":{"clips":999}}'
# Expected: 403 — Turnstile rejects; counters unchanged (re-run the GET)
```

- [ ] **Step 5 (Shaked, browser, production):** process a small card on luna.ozer2.one, export a PDF, open `/stats/` — tiles show the run. Re-process the same card, re-export the same report — numbers unchanged (dedup). Toggle Settings → opt-out, process again — numbers unchanged. Toggle back on.

- [ ] **Step 6: Release** — invoke the `luna-release` skill: user-facing feature (stats page + opt-out setting) → changelog entry + ZeroVer minor bump decision.

---

## Self-Review Notes

- Spec §4.1 said fingerprints use "name + size + mtime + duration"; `ClipRef` carries no mtime and threading `File.lastModified` through the pool adds plumbing for negligible entropy gain — the spec is amended to "name + size + duration" alongside this plan.
- Spec §3 listed a separate `report_count`; it is always equal to the `reports` counter, so the plan stores only `reports` + `report_ms_sum` (avg = sum/reports). Spec amended to match.
- Placeholder thumbnail frames (`NoDecoder` etc.) are excluded from the thumbs counter — only `outcome === 'Success'` frames count as "thumbnails generated".
- Rate limiting uses a 1-minute window (free-plan WAF periods); the spec's "10/hour" intent is served by 10/minute × Turnstile single-use tokens.
