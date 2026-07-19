import type { VendorEnricher } from './types'
import { extraOf, generalTrack, vendorString } from './types'

// Panasonic P2-style ClipMetadata XML atom (FINDINGS: "new shape" —
// General.extra.com_panasonic_SemiPro_metadata_xml, a QuickTime-atom XML
// string, corpus-confirmed via S_cam/S002/lumix_s5.mov). Unlike the ARRI/BRAW
// enrichers, mediainfo does NOT split this atom into flat keys — it's a single
// XML document string we regex-extract targeted tags from (no DOMParser; core
// stays DOM-free). Camera-block tags genuinely present in the corpus dump:
// ClipContent/ClipMetadata/Device (Manufacturer, ModelName) and
// UserArea/AcquisitionMetadata/CameraUnitMetadata (ISOSensitivity,
// Gamma/CaptureGamma). There is no white-balance or shutter tag anywhere in
// the document — those fields are deliberately NOT mapped (fidelity rule:
// never fabricate a value the corpus doesn't carry).
const XML_EXTRA_KEY = 'com_panasonic_SemiPro_metadata_xml'

function panasonicXml(track: ReturnType<typeof generalTrack>): string | undefined {
  return vendorString(extraOf(track)[XML_EXTRA_KEY])
}

// Targeted regex per real tag name — mediainfo.js joins the atom's original
// newlines with " / " between tags, but never inside a tag's text content, so
// a simple non-greedy "no nested `<`" match is safe here.
function tag(xml: string, name: string): string | undefined {
  const match = xml.match(new RegExp(`<${name}>([^<]*)</${name}>`))
  const value = match?.[1]?.trim()
  return value ? value : undefined
}

function panasonicCamera(xml: string): string | undefined {
  const manufacturer = tag(xml, 'Manufacturer')
  const model = tag(xml, 'ModelName')
  if (manufacturer && model) return `${manufacturer} ${model}`
  return model ?? manufacturer
}

export const panasonicEnricher: VendorEnricher = {
  id: 'panasonic-p2-xml',
  detect: (result) => panasonicXml(generalTrack(result)) !== undefined,
  enrich: (result, base) => {
    const xml = panasonicXml(generalTrack(result))
    if (!xml) return base
    return {
      ...base,
      camera: panasonicCamera(xml) ?? base.camera,
      iso: tag(xml, 'ISOSensitivity') ?? base.iso,
      // Camera truth WINS over container tags, same precedence rule as the
      // other vendor enrichers (FINDINGS):
      gamma: tag(xml, 'CaptureGamma') ?? base.gamma,
    }
  },
}
