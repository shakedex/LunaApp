import { describe, expect, test } from 'vite-plus/test'
import { mapMediaInfoToClipMetadata } from '../src/metadata/mediainfo'

// Panasonic P2-style ClipMetadata XML atom — corpus-confirmed shape
// (S_cam/S002/lumix_s5.mov, tools/analysis/out/S_cam_S002_lumix_s5.mov.json):
// General.extra.com_panasonic_SemiPro_metadata_xml carries the WHOLE clip
// metadata document as a single string (mediainfo.js joins the atom's
// original newlines with " / "). Camera-block fields genuinely present in
// this corpus dump: ClipContent/ClipMetadata/Device (Manufacturer,
// ModelName) and UserArea/AcquisitionMetadata/CameraUnitMetadata
// (ISOSensitivity, Gamma/CaptureGamma, Gamut/CaptureGamut). There is NO
// white-balance or shutter tag anywhere in the document — those stay blank
// (FINDINGS: never guess a value the corpus doesn't carry).
//
// Verbatim from the real dump (only the EssenceList's inner numeric detail
// is irrelevant to camera enrichment; every tag below is genuine — none
// invented):
const lumixXml =
  '<?xml version="1.0" encoding="UTF-8" standalone="no" ?> / ' +
  '<ClipMain xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance" xmlns="urn:schemas-Professional-Plug-in:Semi-Pro:ClipMetadata:v1.0"> / ' +
  '  <ClipContent> / ' +
  '    <GlobalClipID>060A2B340101010501010D2113000000972542627A23BD447BDD146104850023</GlobalClipID> / ' +
  '    <Duration>624</Duration> / ' +
  '    <EditUnit>1001/24000</EditUnit> / ' +
  '    <ClipMetadata> / ' +
  '      <Rating>0</Rating> / ' +
  '      <Access> / ' +
  '        <CreationDate>2022-04-19T22:47:20-05:00</CreationDate> / ' +
  '        <LastUpdateDate>2022-04-19T22:47:20-05:00</LastUpdateDate> / ' +
  '      </Access> / ' +
  '      <Device> / ' +
  '        <Manufacturer>Panasonic</Manufacturer> / ' +
  '        <ModelName>DC-S5</ModelName> / ' +
  '      </Device> / ' +
  '      <Shoot> / ' +
  '        <StartDate>2022-04-19T22:47:20-05:00</StartDate> / ' +
  '      </Shoot> / ' +
  '    </ClipMetadata> / ' +
  '  </ClipContent> / ' +
  '  <UserArea> / ' +
  '    <AcquisitionMetadata xmlns="urn:schemas-Professional-Plug-in:P2:CameraMetadata:v1.2"> / ' +
  '      <CameraUnitMetadata> / ' +
  '        <ISOSensitivity>640</ISOSensitivity> / ' +
  '        <Gamma> / ' +
  '          <CaptureGamma>V-Log</CaptureGamma> / ' +
  '        </Gamma> / ' +
  '        <Gamut> / ' +
  '          <CaptureGamut>V-Gamut</CaptureGamut> / ' +
  '        </Gamut> / ' +
  '      </CameraUnitMetadata> / ' +
  '    </AcquisitionMetadata> / ' +
  '  </UserArea> / ' +
  '</ClipMain>'

function lumixPayload(xml: string | undefined, generalExtra: Record<string, unknown> = {}) {
  return {
    media: {
      track: [
        {
          '@type': 'General',
          Format: 'MPEG-4',
          CodecID_Compatible: 'qt  /pana',
          extra:
            xml === undefined
              ? generalExtra
              : { com_panasonic_SemiPro_metadata_xml: xml, ...generalExtra },
        },
        {
          '@type': 'Video',
          Width: '3840',
          Format: 'AVC',
          colour_primaries: 'BT.709',
          transfer_characteristics: 'BT.709',
        },
      ],
    },
  }
}

describe('Panasonic P2-XML enrichment', () => {
  test('fills camera/iso/gamma from the real ClipMetadata XML atom; whiteBalance and shutter stay blank', () => {
    const m = mapMediaInfoToClipMetadata(lumixPayload(lumixXml))
    expect(m.camera).toBe('Panasonic DC-S5')
    expect(m.iso).toBe('640')
    // Camera truth WINS over container tags, same precedence rule as the other enrichers:
    expect(m.gamma).toBe('V-Log')
    // Not in the corpus dump — must stay blank, no fabrication:
    expect(m.whiteBalance).toBeUndefined()
    expect(m.shutter).toBeUndefined()
    expect(m.colorSpace).toBe('BT.709') // container primaries untouched; gamma carries the truth
    expect(m.width).toBe(3840) // base fields untouched
  })

  test('non-Panasonic payloads are untouched (enricher no-op)', () => {
    const m = mapMediaInfoToClipMetadata(lumixPayload(undefined, { manufacturer: 'Sony' }))
    expect(m.camera).toBeUndefined()
    expect(m.iso).toBeUndefined()
    expect(m.gamma).toBeUndefined()
  })

  test('tolerates absence of ISOSensitivity/Gamma (camera still fills from Device block)', () => {
    // Real tags, trimmed document: Device present, CameraUnitMetadata absent entirely
    // (simulates a firmware/model that doesn't emit the UserArea block).
    const trimmed =
      '<?xml version="1.0" encoding="UTF-8" standalone="no" ?> / ' +
      '<ClipMain xmlns="urn:schemas-Professional-Plug-in:Semi-Pro:ClipMetadata:v1.0"> / ' +
      '  <ClipContent> / ' +
      '    <ClipMetadata> / ' +
      '      <Device> / ' +
      '        <Manufacturer>Panasonic</Manufacturer> / ' +
      '        <ModelName>DC-S5</ModelName> / ' +
      '      </Device> / ' +
      '    </ClipMetadata> / ' +
      '  </ClipContent> / ' +
      '</ClipMain>'
    const m = mapMediaInfoToClipMetadata(lumixPayload(trimmed))
    expect(m.camera).toBe('Panasonic DC-S5')
    expect(m.iso).toBeUndefined()
    expect(m.gamma).toBeUndefined()
  })

  test('detects via the vendor extra key even with an otherwise-empty General track', () => {
    const m = mapMediaInfoToClipMetadata({
      media: {
        track: [
          { '@type': 'General', extra: { com_panasonic_SemiPro_metadata_xml: lumixXml } },
          { '@type': 'Video', Format: 'AVC' },
        ],
      },
    })
    expect(m.camera).toBe('Panasonic DC-S5')
  })
})
