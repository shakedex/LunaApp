import { describe, expect, test } from 'bun:test'
import { mapMediaInfoToClipMetadata } from '../src/metadata/mediainfo'

// Sony/Canon acquisition track (RDD 18 "Acquisition Metadata") — corpus-confirmed shape
// (CAMERA_BURANO_.../tools/analysis/out/CAMERA_BURANO_BURANO_XOCN_LT_Clip_2.mxf.json): every value
// is a string, several ship a display-ready `_String` sibling. Values below are taken verbatim
// from that dump (ISO 800, WB 3600, shutter 180.0°/1:48, IrisTNumber 1.922025, focal 50.000,
// TransferCharacteristics "Linear").
const acquisitionExtra = {
  ISOSensitivity_FirstFrame: '800',
  ISOSensitivity_Values: '800',
  WhiteBalance_FirstFrame: '3600',
  WhiteBalance_FirstFrame_String: '3600 K',
  ShutterSpeed_Angle_FirstFrame: '180.0',
  ShutterSpeed_Angle_FirstFrame_String: '180.0°',
  ShutterSpeed_Time_FirstFrame: '1/48',
  ShutterSpeed_Time_FirstFrame_String: '1/48 s',
  IrisFNumber_FirstFrame: '1.838358',
  IrisTNumber_FirstFrame: '1.922025',
  LensZoomActualFocalLength_FirstFrame: '50.000',
  LensZoomActualFocalLength_FirstFrame_String: '50.000 mm',
  LensAttributes_FirstFrame: '2050.0212',
  TransferCharacteristics_FirstFrame: 'Linear',
}

function withAcquisitionTrack(general: Record<string, unknown>) {
  return {
    media: {
      track: [
        { '@type': 'General', Format: 'MXF', ...general },
        { '@type': 'Video', Width: '4096', Format: 'AVC' },
        {
          '@type': 'Other',
          Format: 'Acquisition Metadata',
          extra: acquisitionExtra,
        },
      ],
    },
  }
}

describe('Sony/Canon acquisition-track enrichment', () => {
  test('Sony (Encoded_Library_CompanyName): fills the camera block, _String variants preferred, gamma wins', () => {
    const m = mapMediaInfoToClipMetadata(
      withAcquisitionTrack({ Encoded_Library_CompanyName: 'Sony' }),
    )
    expect(m.iso).toBe('800')
    expect(m.whiteBalance).toBe('3600 K') // _String variant, not kelvinDisplay(3600)
    expect(m.shutter).toBe('180.0°') // _String angle variant, not the Time fallback
    expect(m.aperture).toBe('T1.9') // tNumberDisplay(1.922025), Iris_T wins over Iris_F
    expect(m.focalLength).toBe('50.000 mm')
    expect(m.lens).toBe('2050.0212')
    expect(m.gamma).toBe('Linear') // TransferCharacteristics_FirstFrame wins, as-is string
    expect(m.camera).toBe('Sony')
    expect(m.width).toBe(4096) // base fields untouched
  })

  test('Canon C50 (Encoded_Application_Name, no Library_CompanyName): camera = full model', () => {
    const m = mapMediaInfoToClipMetadata(
      withAcquisitionTrack({
        Encoded_Application_CompanyName: 'Canon',
        Encoded_Application_Name: 'EOS C50',
      }),
    )
    expect(m.camera).toBe('EOS C50')
    expect(m.iso).toBe('800')
    expect(m.gamma).toBe('Linear')
  })

  test('missing angle/T-number falls back to Time string and F-number', () => {
    const m = mapMediaInfoToClipMetadata(
      withAcquisitionTrack({ Encoded_Library_CompanyName: 'Sony' }),
    )
    expect(m).toBeDefined() // sanity: base case above already covers the _String-preferred path

    const fallback = mapMediaInfoToClipMetadata({
      media: {
        track: [
          { '@type': 'General', Encoded_Library_CompanyName: 'Sony' },
          {
            '@type': 'Other',
            Format: 'Acquisition Metadata',
            extra: {
              ISOSensitivity_FirstFrame: '500',
              ShutterSpeed_Time_FirstFrame: '1/50',
              IrisFNumber_FirstFrame: '2.8',
            },
          },
        ],
      },
    })
    expect(fallback.shutter).toBe('1/50')
    expect(fallback.aperture).toBe('F2.8')
  })

  test('non-acquisition payloads are untouched (enricher no-op)', () => {
    const m = mapMediaInfoToClipMetadata({
      media: {
        track: [
          { '@type': 'General', Format: 'MOV' },
          { '@type': 'Video', Format: 'AVC' },
          { '@type': 'Other', Type: 'Time code', TimeCode_FirstFrame: '00:00:00:00' },
        ],
      },
    })
    expect(m.iso).toBeUndefined()
    expect(m.whiteBalance).toBeUndefined()
    expect(m.aperture).toBeUndefined()
    expect(m.camera).toBeUndefined()
  })
})
