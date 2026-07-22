export interface Capabilities {
  fileSystemAccess: boolean
  webCodecs: boolean
  wasm: boolean
  mobile: boolean
}

/** The device signals mobile detection reads. Passed in so the rules are testable. */
export interface DeviceSignals {
  userAgent: string
  maxTouchPoints: number
  /** navigator.userAgentData.mobile — Chromium only, absent elsewhere. */
  uaMobileHint?: boolean
}

export function detectCapabilities(): Capabilities {
  return {
    fileSystemAccess: typeof window !== 'undefined' && 'showDirectoryPicker' in window,
    webCodecs: typeof window !== 'undefined' && 'VideoDecoder' in window,
    wasm: typeof WebAssembly !== 'undefined',
    mobile: typeof navigator !== 'undefined' && isMobileDevice(readDeviceSignals()),
  }
}

function readDeviceSignals(): DeviceSignals {
  const nav = navigator as Navigator & { userAgentData?: { mobile?: boolean } }
  return {
    userAgent: nav.userAgent,
    maxTouchPoints: nav.maxTouchPoints,
    uaMobileHint: nav.userAgentData?.mobile,
  }
}

export function isMobileDevice(s: DeviceSignals): boolean {
  // UA string first: UA-CH reports `mobile: false` for Android tablets, and Luna
  // rules out tablets too, so the hint alone would let them through.
  if (/Android|iPhone|iPad|iPod|Mobi|Tablet|Silk|Kindle|CrOS Tablet/i.test(s.userAgent)) return true
  // iPadOS 13+ claims a desktop Macintosh UA; touch points are the only tell.
  if (/Macintosh/.test(s.userAgent) && s.maxTouchPoints > 1) return true
  return s.uaMobileHint === true
}

/** Why Luna can't run here, or null when it can. */
export type BlockReason = 'mobile' | 'browser'

// File System Access + WASM are required. WebCodecs is optional: when absent,
// every clip decodes through the ffmpeg.wasm path (spec §8.1, §10.2).
// Mobile is ruled out regardless of API support — a card reader is a desk
// workflow and the report UI targets a wide screen (docs/requirements).
export function blockReason(c: Capabilities): BlockReason | null {
  if (c.mobile) return 'mobile'
  if (!c.fileSystemAccess || !c.wasm) return 'browser'
  return null
}
