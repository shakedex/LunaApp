export const SUPPORTED_MEDIA_EXTENSIONS = [
  '.mov',
  '.mp4',
  '.mxf',
  '.avi',
  '.mkv',
  '.m4v',
  '.mts',
  '.m2ts',
  '.3gp',
  '.webm',
  '.wmv',
  '.flv',
  // Reclassified: RAW formats become first-class clips with metadata + a
  // content-aware thumbnail route (mediainfo/exiftool + embedded-preview
  // extraction) instead of a raw-notice dead end (FINDINGS.md, Plan 08).
  '.braw',
  '.r3d',
  '.crm',
] as const

// `.ari` (single-frame ARRIRAW stills) is the only extension left here — no
// embedded preview, no metadata worth surfacing as a clip (FINDINGS.md).
export const UNSUPPORTED_RAW_EXTENSIONS = ['.ari'] as const

export function fileExtensionOf(fileName: string): string {
  const dot = fileName.lastIndexOf('.')
  return dot <= 0 ? '' : fileName.slice(dot).toLowerCase()
}

export function isSupportedMediaExtension(fileName: string): boolean {
  return (SUPPORTED_MEDIA_EXTENSIONS as readonly string[]).includes(fileExtensionOf(fileName))
}

export function isKnownRawExtension(fileName: string): boolean {
  return (UNSUPPORTED_RAW_EXTENSIONS as readonly string[]).includes(fileExtensionOf(fileName))
}
