import type { VendorEnricher } from './types'
import { generalTrack, vendorString, videoTrack } from './types'

// SMPTE UL (Video.transfer_characteristics) -> gamma display name. ARRI .mxf
// (ALEXA 35 / Mini LF) does not embed ISO/WB/shutter (FINDINGS: "not embedded"),
// but the container's transfer-characteristics UL is a real color-pipeline
// signal we can map. Seed confirmed from corpus (ALEXA 35, tools/analysis/out/):
// "0E17010204020000" -> LogC4. Add more ULs here as new corpus clips confirm them.
export const arriTransferUlToGamma: Record<string, string> = {
  '0E17010204020000': 'LogC4',
}

// ARRI .mxf carries no com_arri_camera_* QuickTime atoms (those are .mov-only,
// see arri-mov.ts) — camera model instead lives in General.Encoded_Application_Name,
// alongside General.Encoded_Application_CompanyName === 'ARRI' (corpus-confirmed;
// NOT Encoded_Library_CompanyName, which ARRI .mxf does not set on the General track).
export const arriMxfEnricher: VendorEnricher = {
  id: 'arri-mxf',
  detect: (result) => {
    const general = generalTrack(result)
    return (
      vendorString(general?.Encoded_Application_CompanyName) === 'ARRI' &&
      vendorString(general?.Encoded_Application_Name) !== undefined
    )
  },
  enrich: (result, base) => {
    const general = generalTrack(result)
    const video = videoTrack(result)
    const ul = vendorString(video?.transfer_characteristics)
    return {
      ...base,
      camera: vendorString(general?.Encoded_Application_Name) ?? base.camera,
      gamma: (ul ? arriTransferUlToGamma[ul] : undefined) ?? base.gamma,
    }
  },
}
