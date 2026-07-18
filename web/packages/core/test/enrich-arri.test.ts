import { describe, expect, test } from 'bun:test'
import { mapMediaInfoToClipMetadata } from '../src/metadata/mediainfo'

const arriMov = {
  media: {
    track: [
      {
        '@type': 'General',
        extra: {
          com_arri_camera_CameraModel: 'ALEXA Mini',
          com_arri_camera_ExposureIndexAsa: '800',
          com_arri_camera_WhiteBalanceKelvin: '5600',
          com_arri_camera_ShutterAngle: '1728',
          com_arri_camera_ColorGammaSxS: 'LOG-C',
          com_arri_camera_LensType: 'Cooke S4 32mm',
          com_arri_camera_ReelName: 'A001R2B',
        },
      },
      { '@type': 'Video', Width: '2880', Format: 'ProRes', colour_primaries: 'BT.709' },
    ],
  },
}

describe('ARRI .mov enrichment', () => {
  test('fills the camera block from com_arri_camera_* with normalization', () => {
    const m = mapMediaInfoToClipMetadata(arriMov)
    expect(m.camera).toBe('ALEXA Mini')
    expect(m.iso).toBe('800')
    expect(m.whiteBalance).toBe('5600 K')
    expect(m.shutter).toBe('172.8°')
    expect(m.gamma).toBe('LOG-C')
    expect(m.lens).toBe('Cooke S4 32mm')
    expect(m.reelName).toBe('A001R2B') // vendor reel WINS (base had none)
    expect(m.colorSpace).toBe('BT.709') // primaries untouched; gamma carries the truth
    expect(m.width).toBe(2880) // base fields untouched
  })

  test('non-ARRI payloads are untouched (enricher no-op)', () => {
    const m = mapMediaInfoToClipMetadata({
      media: { track: [{ '@type': 'General' }, { '@type': 'Video', Format: 'AVC' }] },
    })
    expect(m.camera).toBeUndefined()
    expect(m.gamma).toBeUndefined()
  })
})
