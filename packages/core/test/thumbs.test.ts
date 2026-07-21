import { describe, expect, test } from 'vite-plus/test'
import { THUMBNAIL_POSITIONS, thumbnailTimestamps } from '../src/thumbs/model'
import { decodePathFor, PRORES_RAW_CODEC_PATTERN, thumbnailRouteFor } from '../src/thumbs/router'

describe('thumbnailTimestamps', () => {
  test('maps 10/50/90% of duration', () => {
    expect(THUMBNAIL_POSITIONS).toEqual([0.1, 0.5, 0.9])
    expect(thumbnailTimestamps(100)).toEqual([10, 50, 90])
  })
  test('degenerate durations collapse to a single frame at 0', () => {
    expect(thumbnailTimestamps(0)).toEqual([0])
    expect(thumbnailTimestamps(-5)).toEqual([0])
    expect(thumbnailTimestamps(Number.NaN)).toEqual([0])
  })
  test('timestamps never reach the duration itself', () => {
    for (const t of thumbnailTimestamps(1)) expect(t).toBeLessThan(1)
  })
})

describe('thumbnailRouteFor', () => {
  test('fast-path containers go to mediabunny (no codec)', () => {
    for (const ext of ['.mp4', '.m4v', '.mov', '.mkv', '.webm', '.3gp']) {
      expect(thumbnailRouteFor(ext)).toBe('mediabunny')
    }
  })
  test('MXF and legacy go to ffmpeg', () => {
    for (const ext of ['.mxf', '.avi', '.mts', '.m2ts', '.wmv', '.flv']) {
      expect(thumbnailRouteFor(ext)).toBe('ffmpeg')
    }
  })
  test('.crm and .r3d always route to preview (no free frame decode, embedded/sidecar preview instead)', () => {
    expect(thumbnailRouteFor('.crm')).toBe('preview')
    expect(thumbnailRouteFor('.r3d')).toBe('preview')
    // codec is irrelevant for these — routed by extension unconditionally.
    expect(thumbnailRouteFor('.crm', 'CRAW')).toBe('preview')
  })
  test('.braw routes to none (clip with metadata, placeholder thumbnail)', () => {
    expect(thumbnailRouteFor('.braw')).toBe('none')
    expect(thumbnailRouteFor('.braw', 'Blackmagic RAW')).toBe('none')
  })
  test('unknown extensions are never decoded', () => {
    for (const ext of ['.ari', '.txt', '']) {
      expect(thumbnailRouteFor(ext)).toBe('none')
    }
  })

  describe('ProRes RAW detection on a mediabunny-set extension', () => {
    // tools/analysis/out/S_cam_S006_A003C0011_210922_0000_ronin4d_proresraw.mov.json
    // (DJI Ronin-4D ProRes RAW) — mediainfo Video track:
    //   Format = "ProRes", Format_Profile = "RAW", CodecID = "aprn"
    // tools/analysis/out/CAMERA_RONIN-4D_A001C0004_..._4K_ProRes4444_25FPS.mov.json
    // (plain ProRes 4444), same Video track shape:
    //   Format = "ProRes", Format_Profile = "4444", CodecID = "ap4h"
    // `mapMediaInfoToClipMetadata` (commit 80efa25) now threads Format_Profile
    // into ClipMetadata.codec: a "RAW" profile yields "ProRes RAW" for the
    // S006 clip above, while the 4444 dump still maps to the bare "ProRes".
    // So this pattern is written against the real, cited profile/codecID
    // strings, and deliberately does NOT match bare "ProRes" (else every
    // ordinary ProRes 4444/422 clip would be misrouted to 'preview' too).
    test('matches the real Format_Profile/CodecID strings from the S006 dump', () => {
      expect(PRORES_RAW_CODEC_PATTERN.test('RAW')).toBe(false) // bare profile alone is too ambiguous to match on its own
      expect(PRORES_RAW_CODEC_PATTERN.test('ProRes RAW')).toBe(true)
      expect(PRORES_RAW_CODEC_PATTERN.test('aprn')).toBe(true)
      expect(PRORES_RAW_CODEC_PATTERN.test('APRN')).toBe(true) // case-insensitive
    })

    test('a codec string matching the pattern routes a mediabunny extension to preview', () => {
      expect(thumbnailRouteFor('.mov', 'ProRes RAW')).toBe('preview')
      expect(thumbnailRouteFor('.mov', 'aprn')).toBe('preview')
    })

    test('plain "ProRes" (the mapped codec when Format_Profile isn\'t RAW) does not match', () => {
      // A bare "ProRes" string — e.g. the 4444/422 dumps above, or any caller
      // that doesn't carry Format_Profile through — deliberately doesn't
      // match: the mediainfo mapper only appends " RAW" when Format_Profile
      // genuinely is "RAW" (commit 80efa25), so this is the correct negative
      // case, not a known gap. The mediabunny->ffmpeg cascade remains the
      // safety net for any codec string this pattern still misses.
      expect(thumbnailRouteFor('.mov', 'ProRes')).toBe('mediabunny')
    })

    test('ordinary ProRes 4444/422 codec strings stay on mediabunny', () => {
      expect(thumbnailRouteFor('.mov', 'ProRes 4444')).toBe('mediabunny')
      expect(thumbnailRouteFor('.mov', 'ProRes 422')).toBe('mediabunny')
    })
  })
})

describe('decodePathFor (deprecated alias)', () => {
  test('delegates exactly to thumbnailRouteFor(extension) — a true alias, not a lossy narrowing', () => {
    for (const ext of ['.mp4', '.m4v', '.mov', '.mkv', '.webm', '.3gp']) {
      expect(decodePathFor(ext)).toBe('mediabunny')
    }
    for (const ext of ['.mxf', '.avi', '.mts', '.m2ts', '.wmv', '.flv']) {
      expect(decodePathFor(ext)).toBe('ffmpeg')
    }
    // Deliberate change: .crm/.r3d are reclassified clips now, routed to
    // 'preview' (not 'none' as before) — the deprecated alias reflects this
    // real routing rather than hiding it.
    expect(decodePathFor('.crm')).toBe('preview')
    expect(decodePathFor('.r3d')).toBe('preview')
    expect(decodePathFor('.braw')).toBe('none')
    for (const ext of ['.ari', '.txt', '']) {
      expect(decodePathFor(ext)).toBe('none')
    }
  })
})
