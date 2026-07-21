import { describe, expect, test } from 'vite-plus/test'
import { detectReels } from '../src'
import { mapMediaInfoToClipMetadata } from '../src/metadata/mediainfo'

// Blackmagic RAW (.braw) — corpus-confirmed shape (D_CAM_D001_D001_09150935_C013.braw,
// tools/analysis/out/): General.extra carries manufacturer/camera_type/lens_type/reel_name/
// viewing_gamma. The dump's only shutter-adjacent key is `shutter_type: "Angle"` — a mode
// string, not a value — so `shutter` stays blank (FINDINGS: never guess). `analog_gain` is
// the only exposure-adjacent field; it is gain, not ISO, so `iso` stays blank too.
const brawExtra = {
  manufacturer: 'Blackmagic Design',
  camera_id: '1b4dda73-ef34-4cfc-909f-93eecfd8d0f4',
  camera_type: 'Blackmagic Cinema Camera 6K',
  reel_name: '1',
  scene: '1',
  take: '13',
  shutter_type: 'Angle',
  lens_type: 'Sigma 24-70mm F2.8 DG DN | Art 019',
  viewing_gamma: 'Blackmagic Design Film',
  viewing_gamut: 'Blackmagic Design',
  analog_gain: '1.000',
}

function brawPayload(generalExtra: Record<string, unknown>, general: Record<string, unknown> = {}) {
  return {
    media: {
      track: [
        { '@type': 'General', Format: 'QuickTime', ...general, extra: generalExtra },
        { '@type': 'Video', Width: '6064', Format: 'Blackmagic RAW', colour_primaries: 'BT.709' },
      ],
    },
  }
}

describe('BRAW enrichment', () => {
  test('fills camera/lens/gamma/reelName from General.extra; iso and shutter stay blank', () => {
    const m = mapMediaInfoToClipMetadata(brawPayload(brawExtra))
    expect(m.camera).toBe('Blackmagic Cinema Camera 6K')
    expect(m.lens).toBe('Sigma 24-70mm F2.8 DG DN | Art 019')
    expect(m.gamma).toBe('Blackmagic Design Film')
    expect(m.reelName).toBe('1')
    // Fidelity: only analog_gain exists (gain, not ISO) — never map it to iso.
    expect(m.iso).toBeUndefined()
    // Only shutter_type (mode string "Angle") exists in the corpus — no angle/speed value.
    expect(m.shutter).toBeUndefined()
    expect(m.width).toBe(6064) // base fields untouched
  })

  test('non-BRAW payloads are untouched (enricher no-op)', () => {
    const m = mapMediaInfoToClipMetadata({
      media: {
        track: [
          { '@type': 'General', Format: 'QuickTime', extra: { manufacturer: 'Sony' } },
          { '@type': 'Video', Format: 'AVC' },
        ],
      },
    })
    expect(m.camera).toBeUndefined()
    expect(m.gamma).toBeUndefined()
    expect(m.reelName).toBeUndefined()
  })
})

describe('vendor precedence', () => {
  test('vendor reel_name WINS over General.Reel_Name', () => {
    const m = mapMediaInfoToClipMetadata(
      brawPayload(
        {
          manufacturer: 'Blackmagic Design',
          camera_type: 'Blackmagic Cinema Camera 6K',
          reel_name: 'RIGHT',
        },
        { Reel_Name: 'WRONG' },
      ),
    )
    expect(m.reelName).toBe('RIGHT')
  })

  test('camera gamma WINS over container colour_primaries, which still carries colorSpace', () => {
    const m = mapMediaInfoToClipMetadata(
      brawPayload({
        manufacturer: 'Blackmagic Design',
        camera_type: 'Blackmagic Pocket Cinema Camera 6K',
        viewing_gamma: 'Blackmagic Design Film',
      }),
    )
    expect(m.gamma).toBe('Blackmagic Design Film')
    expect(m.colorSpace).toBe('BT.709') // container primaries tag untouched
  })

  test('detectReels groups clips under the vendor-sourced reel name', () => {
    const m = mapMediaInfoToClipMetadata(brawPayload(brawExtra, { Reel_Name: 'WRONG' }))
    const reels = detectReels([{ relativePath: 'X/a.braw', reelName: m.reelName }])
    expect(reels).toHaveLength(1)
    expect(reels[0]?.name).toBe('1')
    expect(reels[0]?.clips.map((c) => c.relativePath)).toEqual(['X/a.braw'])
  })
})
