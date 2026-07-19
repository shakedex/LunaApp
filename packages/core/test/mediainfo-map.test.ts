import { describe, expect, test } from 'bun:test'
import { mapMediaInfoToClipMetadata } from '../src/metadata/mediainfo'

const mxfLike = {
  media: {
    track: [
      { '@type': 'General', Duration: '5.000', FileSize: '104857600', Reel_Name: 'A001R2B' },
      {
        '@type': 'Video',
        Width: '1920',
        Height: '1080',
        Format: 'ProRes',
        FrameRate: '25.000',
        colour_primaries: 'BT.709',
      },
      { '@type': 'Audio', Format: 'PCM' },
      { '@type': 'Other', Type: 'Time code', TimeCode_FirstFrame: '10:20:30:00' },
    ],
  },
}

describe('mapMediaInfoToClipMetadata', () => {
  test('maps a full result (string-typed values)', () => {
    const m = mapMediaInfoToClipMetadata(mxfLike)
    expect(m.width).toBe(1920)
    expect(m.height).toBe(1080)
    expect(m.codec).toBe('ProRes')
    expect(m.frameRate).toBe(25)
    expect(m.durationSeconds).toBe(5)
    expect(m.colorSpace).toBe('BT.709')
    expect(m.startTimecode).toBe('10:20:30:00')
    expect(m.reelName).toBe('A001R2B')
  })

  test('handles number-typed values and ColorSpace fallback', () => {
    const m = mapMediaInfoToClipMetadata({
      media: {
        track: [
          { '@type': 'General', Duration: 12.5 },
          { '@type': 'Video', Width: 3840, Height: 2160, Format: 'AVC', ColorSpace: 'YUV' },
        ],
      },
    })
    expect(m.width).toBe(3840)
    expect(m.durationSeconds).toBe(12.5)
    expect(m.colorSpace).toBe('YUV')
  })

  test('missing fields stay undefined — never fabricated', () => {
    const m = mapMediaInfoToClipMetadata({})
    expect(m.width).toBeUndefined()
    expect(m.codec).toBeUndefined()
    expect(m.startTimecode).toBeUndefined()
    expect(m.reelName).toBeUndefined()
    expect(m.iso).toBeUndefined()
  })

  test('empty strings and NaN are treated as missing', () => {
    const m = mapMediaInfoToClipMetadata({
      media: { track: [{ '@type': 'Video', Width: '', Format: '  ', FrameRate: 'abc' }] },
    })
    expect(m.width).toBeUndefined()
    expect(m.codec).toBeUndefined()
    expect(m.frameRate).toBeUndefined()
  })

  test('RAW profile is threaded into the codec string', () => {
    const m = mapMediaInfoToClipMetadata({
      media: { track: [{ '@type': 'Video', Format: 'ProRes', Format_Profile: 'RAW' }] },
    })
    expect(m.codec).toBe('ProRes RAW')
  })

  test('non-RAW profiles do not pollute the codec string', () => {
    const m = mapMediaInfoToClipMetadata({
      media: { track: [{ '@type': 'Video', Format: 'AVC', Format_Profile: 'High@L5.1' }] },
    })
    expect(m.codec).toBe('AVC')
  })
})
