import { degreesDisplay, kelvinDisplay } from '../normalize'
import type { VendorEnricher } from './types'
import { extraOf, generalTrack } from './types'

function str(v: unknown): string | undefined {
  return typeof v === 'string' && v.trim() !== '' ? v : undefined
}

// ARRI records QuickTime atoms only in .mov (FINDINGS: "mxf vs mov splits ARRI in two").
export const arriMovEnricher: VendorEnricher = {
  id: 'arri-mov',
  detect: (result) =>
    Object.keys(extraOf(generalTrack(result))).some((k) => k.startsWith('com_arri_camera_')),
  enrich: (result, base) => {
    const extra = extraOf(generalTrack(result))
    return {
      ...base,
      camera: str(extra.com_arri_camera_CameraModel) ?? base.camera,
      iso: str(extra.com_arri_camera_ExposureIndexAsa) ?? base.iso,
      whiteBalance: kelvinDisplay(extra.com_arri_camera_WhiteBalanceKelvin) ?? base.whiteBalance,
      shutter: degreesDisplay(extra.com_arri_camera_ShutterAngle, 10) ?? base.shutter,
      lens: str(extra.com_arri_camera_LensType) ?? base.lens,
      // Camera truth WINS over container tags (confirmed mapper bugs, FINDINGS):
      gamma: str(extra.com_arri_camera_ColorGammaSxS) ?? base.gamma,
      reelName: str(extra.com_arri_camera_ReelName) ?? base.reelName,
    }
  },
}
