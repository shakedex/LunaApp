export type { LogEntry, LogLevel } from './activity/log'
export { appendLog, formatLogText, LOG_LEVELS, logLevelAtLeast } from './activity/log'
export type {
  ActivitySnapshot,
  Operation,
  OperationGroup,
  OperationKind,
} from './activity/operations'
export {
  capActivitySnapshot,
  GENERAL_OPERATION,
  groupLogByOperation,
  normalizeActivitySnapshot,
} from './activity/operations'
export { aggregateThumbnailOutcome, CSV_COLUMNS, generateReportCsv } from './export/csv'
export {
  fileExtensionOf,
  isSupportedMediaExtension,
  SUPPORTED_MEDIA_EXTENSIONS,
} from './media/extensions'
export { joinPath } from './media/paths'
export type { CameraFieldKey } from './metadata/fields'
export { CAMERA_FIELDS } from './metadata/fields'
export type { MediaInfoObjectResult, MediaInfoTrack } from './metadata/mediainfo'
export { mapMediaInfoToClipMetadata } from './metadata/mediainfo'
export type { ClipMetadata } from './metadata/model'
export { degreesDisplay, kelvinDisplay, scaledNumber, tNumberDisplay } from './metadata/normalize'
export { extractSonyMxfCameraModel, sonyCameraDisplayName } from './metadata/sony-klv'
export { applyVendorEnrichment, vendorEnrichers } from './metadata/vendors/registry'
export type { VendorEnricher } from './metadata/vendors/types'
export type { PoolHandlers, PoolHooks, PoolOptions } from './pool/run-pool'
export { runPool } from './pool/run-pool'
export type { BoxHeader } from './preview/boxes'
export { findChildBox, readBoxHeaderAt, walkTopLevelBoxes } from './preview/boxes'
export type { EmbeddedPreview } from './preview/extract'
export {
  CRM_PREVIEW_UUID,
  extractCrmPreview,
  extractMovTailPreview,
  extractRtnJpeg,
} from './preview/extract'
export type { JpegCandidate } from './preview/jpeg'
export { findValidJpegs, jpegDimensions } from './preview/jpeg'
export type { DetectedReel, ReelInput } from './reels/detect'
export { compareReelNames, detectReels, UNGROUPED_REEL } from './reels/detect'
export type { ReportSummary } from './report/library'
export { normalizeReportSummaries, summarizeReport } from './report/library'
export type {
  BuildReportInput,
  CoverFields,
  Reel,
  ReelStats,
  ReportClip,
  ReportModel,
  ReportStats,
} from './report/model'
export { buildReportModel, cardCountFrom } from './report/model'
export type {
  BlobLike,
  DirectoryHandleLike,
  FileHandleLike,
  FileSystemEntryLike,
} from './scan/handles'
export { isJunkName } from './scan/junk'
export type { ClipRef, OtherFileRef, ScanSummary } from './scan/model'
export { buildScanSummary } from './scan/model'
export type { ScanProgress, ScanResult } from './scan/walker'
export { scanFolder } from './scan/walker'
export type { Settings } from './settings/model'
export {
  clampWorkerPoolCap,
  defaultSettings,
  normalizeSettings,
  SETTINGS_SCHEMA_VERSION,
  WORKER_POOL_CAP_DEFAULT,
  WORKER_POOL_CAP_MAX,
  WORKER_POOL_CAP_MIN,
} from './settings/model'
export type { ThumbnailFrame, ThumbnailOutcome } from './thumbs/model'
export {
  THUMBNAIL_ENCODE_QUALITY,
  THUMBNAIL_POSITIONS,
  THUMBNAIL_TARGET_WIDTH,
  thumbnailTimestamps,
} from './thumbs/model'
export type { DecodePath, ThumbnailRoute } from './thumbs/router'
export { decodePathFor, PRORES_RAW_CODEC_PATTERN, thumbnailRouteFor } from './thumbs/router'
