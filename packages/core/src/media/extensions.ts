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
  // RAW formats are first-class clips ("a file is a file", 2026-07-19
  // backlog): metadata where extractable, honest placeholder thumbnails where
  // the browser can't paint a frame (.braw, .ari — FINDINGS.md).
  '.braw',
  '.r3d',
  '.crm',
  '.ari',
] as const

export function fileExtensionOf(fileName: string): string {
  const dot = fileName.lastIndexOf('.')
  return dot <= 0 ? '' : fileName.slice(dot).toLowerCase()
}

export function isSupportedMediaExtension(fileName: string): boolean {
  return (SUPPORTED_MEDIA_EXTENSIONS as readonly string[]).includes(fileExtensionOf(fileName))
}
