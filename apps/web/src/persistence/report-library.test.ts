import { expect, test } from 'vite-plus/test'
import { normalizeReportRecord } from './report-library'

const validish = {
  cover: {},
  sourceRoot: 'CARD_A',
  reels: [],
  stats: { cardCount: 1, clipCount: 2, totalSizeBytes: 100 },
}

test('a record written before the incomplete-report fields gets defaults', () => {
  const model = normalizeReportRecord(validish)
  expect(model?.unreadableEntries).toEqual([])
  expect(model?.stats.unreadableFileCount).toBe(0)
  expect(model?.stats.unreadableDirectoryCount).toBe(0)
  expect(model?.stats.clipsMissingMetadataCount).toBe(0)
  // Untouched fields survive.
  expect(model?.stats.clipCount).toBe(2)
  expect(model?.sourceRoot).toBe('CARD_A')
})

test('a current record passes through unchanged', () => {
  const entry = {
    kind: 'file' as const,
    fileName: 'b.mov',
    relativePath: 'A001/b.mov',
    extension: '.mov',
    reason: 'x',
  }
  const model = normalizeReportRecord({
    ...validish,
    unreadableEntries: [entry],
    stats: {
      ...validish.stats,
      unreadableFileCount: 1,
      unreadableDirectoryCount: 0,
      clipsMissingMetadataCount: 3,
    },
  })
  expect(model?.unreadableEntries).toEqual([entry])
  expect(model?.stats.unreadableFileCount).toBe(1)
  expect(model?.stats.clipsMissingMetadataCount).toBe(3)
})

test('junk is rejected rather than repaired', () => {
  expect(normalizeReportRecord(null)).toBeNull()
  expect(normalizeReportRecord('nope')).toBeNull()
  expect(normalizeReportRecord({ reels: [] })).toBeNull() // no stats
  expect(normalizeReportRecord({ ...validish, reels: 'not an array' })).toBeNull()
  // typeof null === 'object', so this one needs its own guard.
  expect(normalizeReportRecord({ ...validish, stats: null })).toBeNull()
})

test('a non-finite counter falls back to zero rather than poisoning a total', () => {
  const model = normalizeReportRecord({
    ...validish,
    stats: { ...validish.stats, unreadableFileCount: Number.NaN },
  })
  expect(model?.stats.unreadableFileCount).toBe(0)
})
