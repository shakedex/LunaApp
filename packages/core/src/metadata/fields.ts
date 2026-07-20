import type { ClipMetadata } from './model'

/** Camera-metadata fields in canonical display order — the single source for
 *  every renderer that lists camera fields (clip cards, PDF). Add a field
 *  here and it surfaces on screen and in the PDF at once (each renderer maps
 *  its own presentation by key). */
export const CAMERA_FIELDS = [
  { key: 'camera', label: 'Camera' },
  { key: 'iso', label: 'ISO' },
  { key: 'whiteBalance', label: 'White balance' },
  { key: 'lens', label: 'Lens' },
  { key: 'focalLength', label: 'Focal length' },
  { key: 'aperture', label: 'Aperture' },
  { key: 'shutter', label: 'Shutter' },
  { key: 'gamma', label: 'Gamma' },
] as const satisfies readonly { key: keyof ClipMetadata; label: string }[]

export type CameraFieldKey = (typeof CAMERA_FIELDS)[number]['key']
