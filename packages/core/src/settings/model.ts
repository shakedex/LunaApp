import type { CoverFields } from '../report/model'

export const SETTINGS_SCHEMA_VERSION = 1

export const WORKER_POOL_CAP_MIN = 1
export const WORKER_POOL_CAP_MAX = 8
export const WORKER_POOL_CAP_DEFAULT = 4

// Settings v1 (spec §14, as amended at Plan 09 scoping): no `theme` (the app
// is dark-only by design) and no `defaultExport` (nothing for it to control).
// Adding either later is a schemaVersion bump + migration, not a breaking edit.
export interface Settings<TImage = unknown> {
  schemaVersion: typeof SETTINGS_SCHEMA_VERSION
  workerPoolCap: number
  coverDefaults: Partial<CoverFields<TImage>>
}

export function defaultSettings<TImage = unknown>(): Settings<TImage> {
  return {
    schemaVersion: SETTINGS_SCHEMA_VERSION,
    workerPoolCap: WORKER_POOL_CAP_DEFAULT,
    coverDefaults: {},
  }
}

export function clampWorkerPoolCap(value: unknown): number {
  if (typeof value !== 'number' || !Number.isFinite(value)) return WORKER_POOL_CAP_DEFAULT
  return Math.min(WORKER_POOL_CAP_MAX, Math.max(WORKER_POOL_CAP_MIN, Math.round(value)))
}

// Cover text fields that may be persisted as defaults. `date` is deliberately
// excluded — the cover date is seeded to "today" on every launch, and a stale
// persisted date is exactly the bug that seeding exists to prevent.
const COVER_DEFAULT_TEXT_KEYS = [
  'projectTitle',
  'productionCompany',
  'dit',
  'director',
  'dp',
] as const

// Defensive load (spec §14): any record we did not write in this exact schema
// version — including newer versions from a future deploy — collapses to
// defaults rather than crashing or half-loading.
export function normalizeSettings<TImage = unknown>(raw: unknown): Settings<TImage> {
  if (typeof raw !== 'object' || raw === null) return defaultSettings<TImage>()
  const record = raw as Record<string, unknown>
  if (record.schemaVersion !== SETTINGS_SCHEMA_VERSION) return defaultSettings<TImage>()
  return {
    schemaVersion: SETTINGS_SCHEMA_VERSION,
    workerPoolCap: clampWorkerPoolCap(record.workerPoolCap),
    coverDefaults: normalizeCoverDefaults<TImage>(record.coverDefaults),
  }
}

function normalizeCoverDefaults<TImage>(value: unknown): Partial<CoverFields<TImage>> {
  if (typeof value !== 'object' || value === null) return {}
  const source = value as Record<string, unknown>
  const out: Partial<CoverFields<TImage>> = {}
  for (const key of COVER_DEFAULT_TEXT_KEYS) {
    const field = source[key]
    if (typeof field === 'string' && field !== '') out[key] = field
  }
  // Boundary: core is DOM-free, so the logo image type is opaque here — the
  // caller (app) owns TImage (a Blob in practice) and idb round-trips it.
  if (source.logo !== undefined && source.logo !== null) out.logo = source.logo as TImage
  return out
}
