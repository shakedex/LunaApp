export {
  fileExtensionOf,
  isKnownRawExtension,
  isSupportedMediaExtension,
  SUPPORTED_MEDIA_EXTENSIONS,
  UNSUPPORTED_RAW_EXTENSIONS,
} from './media/extensions'
export type { MediaInfoObjectResult, MediaInfoTrack } from './metadata/mediainfo'
export { mapMediaInfoToClipMetadata } from './metadata/mediainfo'
export type { ClipMetadata } from './metadata/model'
export type { PoolHandlers, PoolHooks, PoolOptions } from './pool/run-pool'
export { runPool } from './pool/run-pool'
export type {
  BlobLike,
  DirectoryHandleLike,
  FileHandleLike,
  FileSystemEntryLike,
} from './scan/handles'
export { isJunkName } from './scan/junk'
export type { ClipRef, RawNotice, ScanSummary } from './scan/model'
export { buildScanSummary } from './scan/model'
export type { ScanProgress, ScanResult } from './scan/walker'
export { scanFolder } from './scan/walker'
