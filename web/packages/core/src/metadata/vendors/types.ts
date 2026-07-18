import type { MediaInfoObjectResult, MediaInfoTrack } from '../mediainfo'
import type { ClipMetadata } from '../model'

export interface VendorEnricher {
  id: string
  detect(result: MediaInfoObjectResult): boolean
  enrich(result: MediaInfoObjectResult, base: ClipMetadata): ClipMetadata
}

export function generalTrack(result: MediaInfoObjectResult): MediaInfoTrack | undefined {
  return result.media?.track?.find((t) => t['@type'] === 'General')
}

export function videoTrack(result: MediaInfoObjectResult): MediaInfoTrack | undefined {
  return result.media?.track?.find((t) => t['@type'] === 'Video')
}

export function otherTrackWith(
  result: MediaInfoObjectResult,
  key: string,
): MediaInfoTrack | undefined {
  return result.media?.track?.find((t) => t['@type'] === 'Other' && key in t)
}

export function extraOf(track: MediaInfoTrack | undefined): Record<string, unknown> {
  if (!track) return {}
  const extra = track.extra
  if (extra && typeof extra === 'object' && !Array.isArray(extra)) {
    return extra as Record<string, unknown>
  }
  return {}
}

export function vendorString(v: unknown): string | undefined {
  return typeof v === 'string' && v.trim() !== '' ? v : undefined
}
