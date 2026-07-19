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
  sourceRoot: '',
  stats: {
    cardCount: 1,
    clipCount: 2,
    otherFileCount: 0,
    otherFileSizeBytes: 0,
    totalDurationSeconds: 15,
    totalSizeBytes: 150,
  },
  reels: [
    {
      name: 'A001',
      otherFiles: [],
      stats: {
        clipCount: 2,
        otherFileCount: 0,
        otherFileSizeBytes: 0,
        totalSizeBytes: 150,
        totalDurationSeconds: 15,
      },
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
      'path',
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
      'A001,one.mov,A001/one.mov,A001/one.mov,10:20:30:00,1920,1080,ProRes,25,10,100,,"Cam ""A"", unit 1",,,,,,,,Success',
    )
    expect(lines[2]).toBe('A001,two.mov,A001/two.mov,A001/two.mov,,,,,,5,50,,,,,,,,,,NotAttempted')
  })

  test('other files appear as inventory rows in their reel — name, path, size', () => {
    const withOthers = structuredClone(model)
    const reel = withOthers.reels[0]
    if (reel) {
      reel.otherFiles = [
        {
          fileName: 'sound.wav',
          relativePath: 'A001/sound.wav',
          extension: '.wav',
          sizeBytes: 200,
        },
      ]
    }
    const lines = generateReportCsv(withOthers).split('\r\n')
    expect(lines).toHaveLength(4) // header + 2 clips + 1 other file
    // Metadata/thumbnail columns stay blank — this row exists so the CSV is a
    // complete 1:1 inventory of the card, not because a WAV has a codec.
    expect(lines[3]).toBe('A001,sound.wav,A001/sound.wav,A001/sound.wav,,,,,,,200,,,,,,,,,,')
  })

  test('leading formula characters are neutralized', () => {
    const injected = structuredClone(model)
    const clip0 = injected.reels[0]?.clips[0]
    if (clip0) clip0.metadata.camera = '=cmd|/c calc'
    const row = generateReportCsv(injected).split('\r\n')[1]
    expect(row).toContain("'=cmd|/c calc")
  })

  test('path column renders sourceRoot-prefixed captured path', () => {
    const report: ReportModel = {
      cover: {},
      sourceRoot: 'CARD_A',
      stats: {
        cardCount: 1,
        clipCount: 1,
        otherFileCount: 0,
        otherFileSizeBytes: 0,
        totalDurationSeconds: 10,
        totalSizeBytes: 100,
      },
      reels: [
        {
          name: 'A001',
          otherFiles: [],
          stats: {
            clipCount: 1,
            otherFileCount: 0,
            otherFileSizeBytes: 0,
            totalSizeBytes: 100,
            totalDurationSeconds: 10,
          },
          clips: [
            {
              id: 'A001/clip.mov',
              fileName: 'clip.mov',
              relativePath: 'A001/clip.mov',
              extension: '.mov',
              sizeBytes: 100,
              metadata: { durationSeconds: 10 },
              thumbnails: [],
            },
          ],
        },
      ],
    }
    const csv = generateReportCsv(report)
    const header = csv.split('\r\n')[0]
    expect(header).toContain('relativePath,path,')
    expect(csv).toContain('A001/clip.mov,CARD_A/A001/clip.mov')
  })

  test('path column falls back to relativePath when sourceRoot is empty', () => {
    const report: ReportModel = {
      cover: {},
      sourceRoot: '',
      stats: {
        cardCount: 1,
        clipCount: 1,
        otherFileCount: 0,
        otherFileSizeBytes: 0,
        totalDurationSeconds: 10,
        totalSizeBytes: 100,
      },
      reels: [
        {
          name: 'A001',
          otherFiles: [],
          stats: {
            clipCount: 1,
            otherFileCount: 0,
            otherFileSizeBytes: 0,
            totalSizeBytes: 100,
            totalDurationSeconds: 10,
          },
          clips: [
            {
              id: 'A001/clip.mov',
              fileName: 'clip.mov',
              relativePath: 'A001/clip.mov',
              extension: '.mov',
              sizeBytes: 100,
              metadata: { durationSeconds: 10 },
              thumbnails: [],
            },
          ],
        },
      ],
    }
    const line = generateReportCsv(report).split('\r\n')[1] ?? ''
    expect(line).toContain('A001/clip.mov,A001/clip.mov')
  })
})
