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

// ARRI .mxf (ALEXA 35 / Mini LF) — corpus-confirmed shape (A_CAM_A001..., web/tools/out/):
// General track carries Encoded_Application_CompanyName/_Name (no com_arri_camera_* atoms —
// those are QuickTime-only), and Video.transfer_characteristics carries a SMPTE UL for gamma.
const arriMxf = {
  media: {
    track: [
      {
        '@type': 'General',
        Format: 'MXF',
        Encoded_Application_CompanyName: 'ARRI',
        Encoded_Application_Name: 'ALEXA 35',
        Encoded_Application_Version: 'SUP 5.01.00',
      },
      {
        '@type': 'Video',
        Width: '3840',
        Format: 'ProRes',
        colour_primaries: 'BT.601 PAL',
        transfer_characteristics: '0E17010204020000',
      },
    ],
  },
}

describe('ARRI .mxf enrichment', () => {
  test('fills camera + gamma from Encoded_Application_Name and the transfer-UL map', () => {
    const m = mapMediaInfoToClipMetadata(arriMxf)
    expect(m.camera).toBe('ALEXA 35')
    expect(m.gamma).toBe('LogC4')
    // Not embedded in .mxf (FINDINGS) — must stay blank, no fabrication:
    expect(m.iso).toBeUndefined()
    expect(m.whiteBalance).toBeUndefined()
    expect(m.shutter).toBeUndefined()
    expect(m.colorSpace).toBe('BT.601 PAL') // untouched container tag
    expect(m.width).toBe(3840)
  })

  test('unrecognized transfer UL leaves gamma blank (map structured for additions)', () => {
    const m = mapMediaInfoToClipMetadata({
      media: {
        track: [
          {
            '@type': 'General',
            Encoded_Application_CompanyName: 'ARRI',
            Encoded_Application_Name: 'ALEXA Mini LF',
          },
          { '@type': 'Video', transfer_characteristics: '0E17040103010201' },
        ],
      },
    })
    expect(m.camera).toBe('ALEXA Mini LF')
    expect(m.gamma).toBeUndefined()
  })

  test('non-ARRI mxf payloads are untouched (enricher no-op)', () => {
    const m = mapMediaInfoToClipMetadata({
      media: {
        track: [
          { '@type': 'General', Encoded_Application_CompanyName: 'Sony' },
          { '@type': 'Video', Format: 'AVC' },
        ],
      },
    })
    expect(m.camera).toBeUndefined()
    expect(m.gamma).toBeUndefined()
  })
})

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
