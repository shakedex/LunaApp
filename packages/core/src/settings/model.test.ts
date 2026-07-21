import { describe, expect, test } from 'vite-plus/test'
import {
  clampWorkerPoolCap,
  defaultSettings,
  normalizeSettings,
  SETTINGS_SCHEMA_VERSION,
  WORKER_POOL_CAP_DEFAULT,
} from './model'

describe('defaultSettings', () => {
  test('returns schema v2 with default cap, thumbnails on, empty cover defaults', () => {
    expect(defaultSettings()).toEqual({
      schemaVersion: SETTINGS_SCHEMA_VERSION,
      workerPoolCap: WORKER_POOL_CAP_DEFAULT,
      generateThumbnails: true,
      coverDefaults: {},
    })
  })

  test('returns a fresh object each call (no shared mutable state)', () => {
    const a = defaultSettings()
    const b = defaultSettings()
    expect(a).not.toBe(b)
    expect(a.coverDefaults).not.toBe(b.coverDefaults)
  })
})

describe('clampWorkerPoolCap', () => {
  test('clamps below minimum to 1', () => {
    expect(clampWorkerPoolCap(0)).toBe(1)
    expect(clampWorkerPoolCap(-3)).toBe(1)
  })

  test('clamps above maximum to 8', () => {
    expect(clampWorkerPoolCap(99)).toBe(8)
  })

  test('rounds fractional values', () => {
    expect(clampWorkerPoolCap(3.6)).toBe(4)
    expect(clampWorkerPoolCap(2.2)).toBe(2)
  })

  test('non-numbers and non-finite values fall back to the default', () => {
    expect(clampWorkerPoolCap(Number.NaN)).toBe(WORKER_POOL_CAP_DEFAULT)
    expect(clampWorkerPoolCap(Number.POSITIVE_INFINITY)).toBe(WORKER_POOL_CAP_DEFAULT)
    expect(clampWorkerPoolCap('5')).toBe(WORKER_POOL_CAP_DEFAULT)
    expect(clampWorkerPoolCap(undefined)).toBe(WORKER_POOL_CAP_DEFAULT)
  })
})

describe('normalizeSettings', () => {
  test('non-object input yields defaults', () => {
    expect(normalizeSettings(undefined)).toEqual(defaultSettings())
    expect(normalizeSettings(null)).toEqual(defaultSettings())
    expect(normalizeSettings('junk')).toEqual(defaultSettings())
    expect(normalizeSettings(42)).toEqual(defaultSettings())
  })

  test('newer/unknown schemaVersion yields defaults (spec §14 defensive load)', () => {
    expect(normalizeSettings({ schemaVersion: 99, workerPoolCap: 2 })).toEqual(defaultSettings())
    expect(normalizeSettings({ workerPoolCap: 2 })).toEqual(defaultSettings())
  })

  test('v1 records migrate: fields preserved, generateThumbnails defaults on', () => {
    const migrated = normalizeSettings({
      schemaVersion: 1,
      workerPoolCap: 6,
      coverDefaults: { dit: 'Shaked' },
    })
    expect(migrated).toEqual({
      schemaVersion: 2,
      workerPoolCap: 6,
      generateThumbnails: true,
      coverDefaults: { dit: 'Shaked' },
    })
  })

  test('generateThumbnails must be a real boolean; junk falls back to true', () => {
    expect(
      normalizeSettings({ schemaVersion: 2, workerPoolCap: 4, generateThumbnails: false })
        .generateThumbnails,
    ).toBe(false)
    expect(
      normalizeSettings({ schemaVersion: 2, workerPoolCap: 4, generateThumbnails: 'no' })
        .generateThumbnails,
    ).toBe(true)
    expect(normalizeSettings({ schemaVersion: 2, workerPoolCap: 4 }).generateThumbnails).toBe(true)
  })

  test('valid record passes through with cap clamped', () => {
    const logo = { marker: 'image' }
    const result = normalizeSettings<{ marker: string }>({
      schemaVersion: 2,
      workerPoolCap: 99,
      generateThumbnails: true,
      coverDefaults: { dit: 'Shaked', projectTitle: 'Luna', logo },
    })
    expect(result).toEqual({
      schemaVersion: 2,
      workerPoolCap: 8,
      generateThumbnails: true,
      coverDefaults: { dit: 'Shaked', projectTitle: 'Luna', logo },
    })
  })

  test('cover defaults drop empty strings, non-strings, and unknown keys', () => {
    const result = normalizeSettings({
      schemaVersion: 2,
      workerPoolCap: 4,
      generateThumbnails: true,
      coverDefaults: { dit: '', director: 7, bogus: 'x', dp: 'Dana' },
    })
    expect(result.coverDefaults).toEqual({ dp: 'Dana' })
  })

  test('cover defaults never carry a persisted date', () => {
    const result = normalizeSettings({
      schemaVersion: 2,
      workerPoolCap: 4,
      generateThumbnails: true,
      coverDefaults: { date: '2020-01-01', dit: 'Shaked' },
    })
    expect(result.coverDefaults).toEqual({ dit: 'Shaked' })
  })

  test('garbage coverDefaults collapses to empty object', () => {
    const result = normalizeSettings({
      schemaVersion: 2,
      workerPoolCap: 4,
      generateThumbnails: true,
      coverDefaults: 'nope',
    })
    expect(result.coverDefaults).toEqual({})
  })
})
