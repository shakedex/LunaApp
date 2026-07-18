import { describe, expect, test } from 'bun:test'
import { aggregateThumbnailOutcome, CSV_COLUMNS, generateReportCsv } from '../src/export/csv'
import type { ReportModel } from '../src/report/model'
import type { ThumbnailFrame } from '../src/thumbs/model'

const frame = (outcome: ThumbnailFrame['outcome']): ThumbnailFrame => ({
  positionRatio: 0.1,
  timestampSeconds: 0,
  outcome,
})

describe('aggregateThumbnailOutcome', () => {
  test('empty means never attempted', () => {
    expect(aggregateThumbnailOutcome([])).toBe('NotAttempted')
  })
  test('any success wins', () => {
    expect(aggregateThumbnailOutcome([frame('SeekFailed'), frame('Success')])).toBe('Success')
  })
  test('otherwise the first failure is reported', () => {
    expect(aggregateThumbnailOutcome([frame('SeekFailed'), frame('DecodeFailed')])).toBe(
      'SeekFailed',
    )
  })
})

const model: ReportModel = {
  cover: {},
  stats: { cardCount: 1, clipCount: 2, rawCount: 0, totalDurationSeconds: 15, totalSizeBytes: 150 },
  raw: [],
  reels: [
    {
      name: 'A001',
      stats: { clipCount: 2, totalSizeBytes: 150, totalDurationSeconds: 15 },
      clips: [
        {
          id: 'A001/one.mov',
          fileName: 'one.mov',
          relativePath: 'A001/one.mov',
          extension: '.mov',
          sizeBytes: 100,
          metadata: {
            width: 1920,
            height: 1080,
            codec: 'ProRes',
            frameRate: 25,
            durationSeconds: 10,
            startTimecode: '10:20:30:00',
            camera: 'Cam "A", unit 1',
          },
          thumbnails: [frame('Success')],
        },
        {
          id: 'A001/two.mov',
          fileName: 'two.mov',
          relativePath: 'A001/two.mov',
          extension: '.mov',
          sizeBytes: 50,
          metadata: { durationSeconds: 5 },
          thumbnails: [],
        },
      ],
    },
  ],
}

describe('generateReportCsv', () => {
  test('header matches the spec column order exactly', () => {
    const header = generateReportCsv(model).split('\r\n')[0]
    expect(header).toBe(CSV_COLUMNS.join(','))
    expect(CSV_COLUMNS).toEqual([
      'reel',
      'fileName',
      'relativePath',
      'startTimecode',
      'width',
      'height',
      'codec',
      'frameRate',
      'durationSeconds',
      'sizeBytes',
      'colorSpace',
      'camera',
      'iso',
      'whiteBalance',
      'lens',
      'focalLength',
      'aperture',
      'shutter',
      'gamma',
      'thumbnailOutcome',
    ])
  })

  test('rows: values, RFC 4180 quoting, blanks for missing, CRLF', () => {
    const lines = generateReportCsv(model).split('\r\n')
    expect(lines).toHaveLength(3) // header + 2 clips, no trailing newline
    expect(lines[1]).toBe(
      'A001,one.mov,A001/one.mov,10:20:30:00,1920,1080,ProRes,25,10,100,,"Cam ""A"", unit 1",,,,,,,,Success',
    )
    expect(lines[2]).toBe('A001,two.mov,A001/two.mov,,,,,,5,50,,,,,,,,,,NotAttempted')
  })

  test('leading formula characters are neutralized', () => {
    const injected = structuredClone(model)
    const clip0 = injected.reels[0]?.clips[0]
    if (clip0) clip0.metadata.camera = '=cmd|/c calc'
    const row = generateReportCsv(injected).split('\r\n')[1]
    expect(row).toContain("'=cmd|/c calc")
  })
})
