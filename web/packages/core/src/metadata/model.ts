// Spec §9. Standard fields are reliable across supported formats; camera
// fields populate only when the container carries them (§13) — the ffmpeg
// metadata-dictionary merge in a later plan fills most of the camera block.
export interface ClipMetadata {
  width?: number
  height?: number
  codec?: string
  frameRate?: number
  durationSeconds?: number
  colorSpace?: string
  startTimecode?: string
  reelName?: string
  camera?: string
  iso?: string
  whiteBalance?: string
  lens?: string
  focalLength?: string
  aperture?: string
  shutter?: string
  gamma?: string
}
