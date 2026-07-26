import type { ReportStats, ScanSummary } from '@luna-web/core'
import { describe, expect, test } from 'vite-plus/test'
import { pdfShortfalls, reportShortfalls, scanShortfall } from './shortfall-lines'

const stats = (over: Partial<ReportStats> = {}): ReportStats => ({
  cardCount: 1,
  clipCount: 1,
  otherFileCount: 0,
  otherFileSizeBytes: 0,
  totalDurationSeconds: 0,
  totalSizeBytes: 0,
  unreadableFileCount: 0,
  unreadableDirectoryCount: 0,
  clipsMissingMetadataCount: 0,
  ...over,
})

const summary = (over: Partial<ScanSummary> = {}): ScanSummary => ({
  clipCount: 0,
  otherFileCount: 0,
  otherFileSizeBytes: 0,
  totalSizeBytes: 0,
  unreadableFileCount: 0,
  unreadableDirectoryCount: 0,
  byExtension: {},
  ...over,
})

describe('reportShortfalls', () => {
  test('a complete report says nothing', () => {
    expect(reportShortfalls(stats())).toEqual([])
  })
  test('missing metadata costs duration only', () => {
    expect(reportShortfalls(stats({ clipsMissingMetadataCount: 6 }))).toEqual([
      '6 clips missing metadata — duration not included in the totals.',
    ])
  })
  test('an unreadable file is not counted at all', () => {
    expect(reportShortfalls(stats({ unreadableFileCount: 2 }))).toEqual([
      '2 files could not be opened — not counted in the totals.',
    ])
  })
  test('an unreadable folder hides an unknown amount', () => {
    expect(reportShortfalls(stats({ unreadableDirectoryCount: 1 }))).toEqual([
      '1 folder could not be opened — nothing inside it is counted in the totals.',
    ])
  })
  test('singulars', () => {
    expect(
      reportShortfalls(
        stats({
          clipsMissingMetadataCount: 1,
          unreadableFileCount: 1,
          unreadableDirectoryCount: 2,
        }),
      ),
    ).toEqual([
      '1 clip missing metadata — duration not included in the totals.',
      '1 file could not be opened — not counted in the totals.',
      '2 folders could not be opened — nothing inside them is counted in the totals.',
    ])
  })
})

describe('pdfShortfalls', () => {
  test('a complete report says nothing', () => {
    expect(pdfShortfalls(stats())).toEqual([])
  })
  test('missing metadata costs duration only', () => {
    expect(pdfShortfalls(stats({ clipsMissingMetadataCount: 6 }))).toEqual([
      '6 clips missing metadata — duration not included above',
    ])
  })
  test('an unreadable file is not counted at all', () => {
    expect(pdfShortfalls(stats({ unreadableFileCount: 2 }))).toEqual([
      '2 files could not be opened — not counted above',
    ])
  })
  test('each kind gets its own sentence, worded for a printed page', () => {
    expect(
      pdfShortfalls(
        stats({
          clipsMissingMetadataCount: 2,
          unreadableFileCount: 1,
          unreadableDirectoryCount: 1,
        }),
      ),
    ).toEqual([
      '2 clips missing metadata — duration not included above',
      '1 file could not be opened — not counted above',
      '1 folder could not be opened — nothing inside it is counted above',
    ])
  })
})

describe('scanShortfall', () => {
  test('a clean scan says nothing', () => {
    expect(scanShortfall(summary())).toBeNull()
  })
  test('files only', () => {
    expect(scanShortfall(summary({ unreadableFileCount: 2 }))).toBe(
      "2 files couldn't be opened and won't be in the report",
    )
  })
  test('folders only', () => {
    expect(scanShortfall(summary({ unreadableDirectoryCount: 1 }))).toBe(
      "1 folder couldn't be opened and nothing inside it will be in the report",
    )
  })
  test('folders only, plural', () => {
    expect(scanShortfall(summary({ unreadableDirectoryCount: 2 }))).toBe(
      "2 folders couldn't be opened and nothing inside them will be in the report",
    )
  })
  test('both, joined', () => {
    expect(scanShortfall(summary({ unreadableFileCount: 1, unreadableDirectoryCount: 2 }))).toBe(
      "1 file and 2 folders couldn't be opened and won't be in the report",
    )
  })
})
