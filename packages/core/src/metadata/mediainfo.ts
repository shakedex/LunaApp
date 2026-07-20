import type { ClipMetadata } from './model'
import { applyVendorEnrichment } from './vendors/registry'

export interface MediaInfoTrack {
  '@type': string
  [key: string]: unknown
}

export interface MediaInfoObjectResult {
  media?: { track?: MediaInfoTrack[] }
}

function num(v: unknown): number | undefined {
  if (typeof v === 'number') return Number.isFinite(v) ? v : undefined
  if (typeof v === 'string' && v.trim() !== '') {
    const n = Number(v)
    return Number.isFinite(n) ? n : undefined
  }
  return undefined
}

function str(v: unknown): string | undefined {
  return typeof v === 'string' && v.trim() !== '' ? v : undefined
}

// Sony X-OCN picture-coding UL family — MediaInfoLib has no name for X-OCN
// essence and surfaces the raw UL pair as Video.Format (FINDINGS.md
// 2026-07-20). The variant byte (XX) differs per XT/ST/LT (0601 = LT,
// corpus-confirmed twice; 0501 seen on the VENICE 2 8K clip) — name the
// family, claim no variant until the byte table is confirmed.
const XOCN_FORMAT_PATTERN = /^0E060D0302020100-0E0604010206[0-9A-F]{2}01$/i

function codecDisplay(baseCodec: string | undefined): string | undefined {
  if (baseCodec && XOCN_FORMAT_PATTERN.test(baseCodec)) return 'X-OCN'
  return baseCodec
}

export function mapMediaInfoToClipMetadata(result: MediaInfoObjectResult): ClipMetadata {
  const tracks = result.media?.track ?? []
  const general = tracks.find((t) => t['@type'] === 'General')
  const video = tracks.find((t) => t['@type'] === 'Video')
  const timecode = tracks.find(
    (t) => t['@type'] === 'Other' && str(t.Type)?.toLowerCase() === 'time code',
  )
  const baseCodec = codecDisplay(str(video?.Format))
  const profile = str(video?.Format_Profile)
  const codec = baseCodec && profile?.toUpperCase() === 'RAW' ? `${baseCodec} RAW` : baseCodec
  const base: ClipMetadata = {
    width: num(video?.Width),
    height: num(video?.Height),
    codec,
    frameRate: num(video?.FrameRate),
    // MediaInfo JSON reports General.Duration in seconds — verify in manual QA.
    durationSeconds: num(general?.Duration),
    colorSpace: str(video?.colour_primaries) ?? str(video?.ColorSpace),
    startTimecode: str(timecode?.TimeCode_FirstFrame),
    reelName: str(general?.Reel_Name) ?? str(video?.Reel_Name),
  }
  return applyVendorEnrichment(result, base)
}
