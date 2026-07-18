export type ThumbnailRoute = 'mediabunny' | 'ffmpeg' | 'preview' | 'none'

const MEDIABUNNY_EXTENSIONS = new Set(['.mp4', '.m4v', '.mov', '.mkv', '.webm', '.3gp'])
// .mts/.m2ts default to ffmpeg: AVCHD's 192-byte BDAV TS variant is
// unverified in mediabunny (spec §10.2). Promote after real-file QA.
const FFMPEG_EXTENSIONS = new Set(['.mxf', '.avi', '.mts', '.m2ts', '.wmv', '.flv'])

// Extensions with no ffmpeg/mediabunny decode path but a free embedded/sidecar
// preview (FINDINGS.md "Verified box offsets"): `.crm` → its `PRVW` uuid box;
// `.r3d` → the `.rtn` REDTHUMBNAIL sidecar when present. Routed unconditionally
// by extension — the extractor (Plan 08 Task 2/4) decides frame-vs-placeholder.
const PREVIEW_ONLY_EXTENSIONS = new Set(['.crm', '.r3d'])

// `.braw` gets full metadata (BRAW enricher) but no browser-decodable frame and
// no embedded preview (FINDINGS.md: "no embedded preview tag... BRAW frames
// need the Blackmagic RAW SDK") — clip with a placeholder thumbnail.
const NO_THUMBNAIL_EXTENSIONS = new Set(['.braw'])

// Verified against web/tools/out/S_cam_S006_A003C0011_210922_0000_ronin4d_proresraw.mov.json
// (DJI Ronin-4D ProRes RAW, mediainfo `Video` track): Format = "ProRes",
// Format_Profile = "RAW", CodecID = "aprn". Contrast with
// web/tools/out/CAMERA_RONIN-4D_..._4K_ProRes4444_25FPS.mov.json (plain ProRes
// 4444): Format = "ProRes", Format_Profile = "4444", CodecID = "ap4h" — same
// `Format`, different profile/codecID. IMPORTANT CAVEAT: today's
// `mapMediaInfoToClipMetadata` (src/metadata/mediainfo.ts) only reads
// `video.Format` into `ClipMetadata.codec`, so for BOTH clips above
// `currentMapping.codec` is the bare string "ProRes" — the RAW/4444 distinction
// is not yet preserved on that field. This pattern intentionally matches the
// distinguishing profile/codecID strings (not bare "ProRes") so a caller must
// supply a richer codec string (e.g. `${Format} ${Format_Profile}`, or the
// CodecID) for the branch to fire; wiring that through is a follow-up (the
// mediabunny→ffmpeg cascade is the safety net documented in the plan if
// detection misses in the meantime).
export const PRORES_RAW_CODEC_PATTERN = /prores\s*raw|\baprn\b/i

export function thumbnailRouteFor(extension: string, codec?: string): ThumbnailRoute {
  if (PREVIEW_ONLY_EXTENSIONS.has(extension)) return 'preview'
  if (NO_THUMBNAIL_EXTENSIONS.has(extension)) return 'none'
  if (MEDIABUNNY_EXTENSIONS.has(extension)) {
    if (codec && PRORES_RAW_CODEC_PATTERN.test(codec)) return 'preview'
    return 'mediabunny'
  }
  if (FFMPEG_EXTENSIONS.has(extension)) return 'ffmpeg'
  return 'none'
}

/** @deprecated Use {@link thumbnailRouteFor}. Kept so existing callers compile;
 * migrate to `thumbnailRouteFor(extension, codec)` in Task 4. Note this now
 * genuinely returns `'preview'` for `.crm`/`.r3d` — it is a true delegate, not
 * a lossy narrowing to the old `'mediabunny' | 'ffmpeg' | 'none'` domain. */
export type DecodePath = ThumbnailRoute

/** @deprecated Use {@link thumbnailRouteFor}. */
export function decodePathFor(extension: string): DecodePath {
  return thumbnailRouteFor(extension)
}
