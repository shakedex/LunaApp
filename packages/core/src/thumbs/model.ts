export type ThumbnailOutcome =
  | 'Success'
  | 'NoDecoder'
  | 'SeekFailed'
  | 'DecodeFailed'
  | 'ContainerOpenFailed'
  | 'NotAttempted'

// Generic over the image payload so core stays DOM-free; the app uses
// ThumbnailFrame<Blob>.
export interface ThumbnailFrame<TImage = unknown> {
  positionRatio: number
  timestampSeconds: number
  image?: TImage
  mime?: string
  outcome: ThumbnailOutcome
}

export const THUMBNAIL_POSITIONS: readonly number[] = [0.1, 0.5, 0.9]

export function thumbnailTimestamps(durationSeconds: number): number[] {
  if (!Number.isFinite(durationSeconds) || durationSeconds <= 0) return [0]
  return THUMBNAIL_POSITIONS.map((p) => Math.min(p * durationSeconds, durationSeconds * 0.999))
}
