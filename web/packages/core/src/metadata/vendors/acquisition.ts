import type { MediaInfoObjectResult } from '../mediainfo'
import { fNumberDisplay, focalLengthDisplay, kelvinDisplay, tNumberDisplay } from '../normalize'
import type { VendorEnricher } from './types'
import { extraOf, generalTrack, vendorString } from './types'

// Sony/Canon MXF acquisition track (RDD 18 "Acquisition Metadata", @type Other) —
// identical shape across Venice/BURANO/FX6/C50 (FINDINGS). Every value is a
// string; the track's ISOSensitivity_FirstFrame key is the reliable detection
// signal (present on every corpus clip that carries this track, absent
// elsewhere — e.g. ARRI .mxf's Other tracks are plain time-code tracks).
function acquisitionTrack(result: MediaInfoObjectResult) {
  return result.media?.track?.find(
    (t) => t['@type'] === 'Other' && 'ISOSensitivity_FirstFrame' in extraOf(t),
  )
}

// Camera model: General.Encoded_Application_Name (single field per the original
// spec) doesn't hold a usable camera name for Sony — real corpus clips (BURANO,
// FX6) put their *recorder software* name there ("AXS", "Mem"), not the camera.
// Encoded_Library_CompanyName ('Sony') is the reliable generic label there.
// Canon's C50 .mxf has no Encoded_Library_CompanyName at all, and its
// Encoded_Application_Name *is* the full model ('EOS C50') — so try the
// company-name field first (favors the correct generic Sony label) and only
// fall back to Encoded_Application_Name when it's absent (Canon).
function acquisitionCamera(result: MediaInfoObjectResult): string | undefined {
  const general = generalTrack(result)
  return (
    vendorString(general?.Encoded_Library_CompanyName) ??
    vendorString(general?.Encoded_Application_Name)
  )
}

export const acquisitionEnricher: VendorEnricher = {
  id: 'acquisition',
  detect: (result) => acquisitionTrack(result) !== undefined,
  enrich: (result, base) => {
    const extra = extraOf(acquisitionTrack(result))
    return {
      ...base,
      iso: vendorString(extra.ISOSensitivity_FirstFrame) ?? base.iso,
      whiteBalance:
        vendorString(extra.WhiteBalance_FirstFrame_String) ??
        kelvinDisplay(extra.WhiteBalance_FirstFrame) ??
        base.whiteBalance,
      shutter:
        vendorString(extra.ShutterSpeed_Angle_FirstFrame_String) ??
        vendorString(extra.ShutterSpeed_Time_FirstFrame) ??
        base.shutter,
      aperture:
        tNumberDisplay(extra.IrisTNumber_FirstFrame) ??
        fNumberDisplay(extra.IrisFNumber_FirstFrame) ??
        base.aperture,
      focalLength:
        focalLengthDisplay(extra.LensZoomActualFocalLength_FirstFrame) ?? base.focalLength,
      lens: vendorString(extra.LensAttributes_FirstFrame) ?? base.lens,
      // Camera truth WINS, same precedence rule as arri-mov (FINDINGS):
      gamma: vendorString(extra.TransferCharacteristics_FirstFrame) ?? base.gamma,
      camera: acquisitionCamera(result) ?? base.camera,
    }
  },
}
