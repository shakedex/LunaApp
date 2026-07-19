import { describe, expect, test } from 'bun:test'
import { normalizeReportSummaries, type ReportSummary, summarizeReport } from './library'
import type { ReportModel } from './model'

type Img = { bytes: number }

function model(overrides: Partial<ReportModel<Img>> = {}): ReportModel<Img> {
  return {
    cover: { projectTitle: 'Luna Feature' },
    sourceRoot: 'CARD_A',
    stats: {
      cardCount: 1,
      clipCount: 2,
      otherFileCount: 1,
      otherFileSizeBytes: 10,
      totalDurationSeconds: 120,
      totalSizeBytes: 5_000,
    },
    reels: [
      {
        name: 'A001',
        clips: [
          {
            id: 'A001/a.mov',
            fileName: 'a.mov',
            relativePath: 'A001/a.mov',
            extension: '.mov',
            sizeBytes: 2_000,
            metadata: {},
            thumbnails: [
              {
                positionRatio: 0.5,
                timestampSeconds: 1,
                outcome: 'Success',
                image: { bytes: 300 },
              },
              { positionRatio: 0.9, timestampSeconds: 2, outcome: 'SeekFailed' },
            ],
          },
          {
            id: 'A001/b.mov',
            fileName: 'b.mov',
            relativePath: 'A001/b.mov',
            extension: '.mov',
            sizeBytes: 2_990,
            metadata: {},
            thumbnails: [],
          },
        ],
        otherFiles: [],
        stats: {
          clipCount: 2,
          otherFileCount: 0,
          otherFileSizeBytes: 0,
          totalSizeBytes: 4_990,
          totalDurationSeconds: 120,
        },
      },
    ],
    ...overrides,
  }
}

const META = { id: 'r-1', savedAt: 1_700_000_000_000 }
const bytesOf = (image: Img) => image.bytes

describe('summarizeReport', () => {
  test('derives title, counts, sizes, thumbnail bytes and flag', () => {
    expect(summarizeReport(model(), META, bytesOf)).toEqual({
      id: 'r-1',
      savedAt: 1_700_000_000_000,
      title: 'Luna Feature',
      sourceRoot: 'CARD_A',
      clipCount: 2,
      otherFileCount: 1,
      totalSizeBytes: 5_000,
      storedFrameBytes: 300,
      hasThumbnails: true,
    })
  })

  test('title falls back to sourceRoot, then to "Camera report"', () => {
    expect(summarizeReport(model({ cover: {} }), META, bytesOf).title).toBe('CARD_A')
    expect(summarizeReport(model({ cover: {}, sourceRoot: '' }), META, bytesOf).title).toBe(
      'Camera report',
    )
    expect(summarizeReport(model({ cover: { projectTitle: '  ' } }), META, bytesOf).title).toBe(
      'CARD_A',
    )
  })

  test('metadata-only report: no thumbnails, zero stored bytes', () => {
    const m = model()
    for (const reel of m.reels) for (const clip of reel.clips) clip.thumbnails = []
    const summary = summarizeReport(m, META, bytesOf)
    expect(summary.hasThumbnails).toBe(false)
    expect(summary.storedFrameBytes).toBe(0)
  })

  test('non-Success frames with images still count toward stored bytes', () => {
    const m = model()
    m.reels[0]?.clips[0]?.thumbnails.push({
      positionRatio: 0.1,
      timestampSeconds: 0,
      outcome: 'DecodeFailed',
      image: { bytes: 50 },
    })
    expect(summarizeReport(m, META, bytesOf).storedFrameBytes).toBe(350)
  })
})

describe('normalizeReportSummaries', () => {
  const valid: ReportSummary = {
    id: 'r-1',
    savedAt: 100,
    title: 't',
    sourceRoot: 's',
    clipCount: 1,
    otherFileCount: 0,
    totalSizeBytes: 10,
    storedFrameBytes: 0,
    hasThumbnails: false,
  }

  test('non-array yields empty', () => {
    expect(normalizeReportSummaries(undefined)).toEqual([])
    expect(normalizeReportSummaries('junk')).toEqual([])
  })

  test('invalid members dropped, valid kept, sorted newest first', () => {
    const newer = { ...valid, id: 'r-2', savedAt: 200 }
    const result = normalizeReportSummaries([
      valid,
      { id: 7 },
      null,
      newer,
      { ...valid, savedAt: 'x' },
    ])
    expect(result.map((s) => s.id)).toEqual(['r-2', 'r-1'])
  })
})
