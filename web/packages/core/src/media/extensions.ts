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
] as const

export const UNSUPPORTED_RAW_EXTENSIONS = ['.r3d', '.braw', '.ari'] as const

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
