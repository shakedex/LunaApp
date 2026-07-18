import type { ClipMetadata } from './model'

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

export function mapMediaInfoToClipMetadata(result: MediaInfoObjectResult): ClipMetadata {
  const tracks = result.media?.track ?? []
  const general = tracks.find((t) => t['@type'] === 'General')
  const video = tracks.find((t) => t['@type'] === 'Video')
  const timecode = tracks.find(
    (t) => t['@type'] === 'Other' && str(t.Type)?.toLowerCase() === 'time code',
  )
  return {
    width: num(video?.Width),
    height: num(video?.Height),
    codec: str(video?.Format),
    frameRate: num(video?.FrameRate),
    // MediaInfo JSON reports General.Duration in seconds — verify in manual QA.
    durationSeconds: num(general?.Duration),
    colorSpace: str(video?.colour_primaries) ?? str(video?.ColorSpace),
    startTimecode: str(timecode?.TimeCode_FirstFrame),
    reelName: str(general?.Reel_Name) ?? str(video?.Reel_Name),
  }
}
