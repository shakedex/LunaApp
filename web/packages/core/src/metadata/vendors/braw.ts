import type { VendorEnricher } from './types'
import { extraOf, generalTrack, vendorString } from './types'

// Blackmagic RAW (.braw) — General.extra shape (FINDINGS, corpus-confirmed via
// D_CAM_D001_D001_09150935_C013.braw): manufacturer/camera_type/lens_type/reel_name/
// viewing_gamma live alongside a `shutter_type` mode string ("Angle"/"Speed"/"None")
// and an `analog_gain` exposure field.
//
// Deliberately NOT mapped:
// - `shutter`: the corpus dump carries only `shutter_type` (a mode label), never a
//   `shutter_angle`/`shutter_speed` value alongside it — mapping the mode string would
//   fabricate a reading, so `shutter` stays blank until a real value key is confirmed.
// - `iso`: only `analog_gain` exists (camera gain, not exposure index) — fidelity rule,
//   do not repurpose gain as ISO.
export const brawEnricher: VendorEnricher = {
  id: 'braw',
  detect: (result) =>
    vendorString(extraOf(generalTrack(result)).manufacturer) === 'Blackmagic Design',
  enrich: (result, base) => {
    const extra = extraOf(generalTrack(result))
    return {
      ...base,
      camera: vendorString(extra.camera_type) ?? base.camera,
      lens: vendorString(extra.lens_type) ?? base.lens,
      // Camera truth WINS over container tags, same precedence rule as the other
      // vendor enrichers (FINDINGS):
      gamma: vendorString(extra.viewing_gamma) ?? base.gamma,
      reelName: vendorString(extra.reel_name) ?? base.reelName,
    }
  },
}
