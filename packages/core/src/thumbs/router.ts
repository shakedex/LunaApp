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
// need the Blackmagic RAW SDK") — clip with a placeholder thumbnail. `.ari`
// (single-frame ARRIRAW stills) is the same story: first-class clip, no
// browser-paintable preview.
const NO_THUMBNAIL_EXTENSIONS = new Set(['.braw', '.ari'])

// Verified against tools/analysis/out/S_cam_S006_A003C0011_210922_0000_ronin4d_proresraw.mov.json
// (DJI Ronin-4D ProRes RAW, mediainfo `Video` track): Format = "ProRes",
// Format_Profile = "RAW", CodecID = "aprn". Contrast with
// tools/analysis/out/CAMERA_RONIN-4D_..._4K_ProRes4444_25FPS.mov.json (plain ProRes
// 4444): Format = "ProRes", Format_Profile = "4444", CodecID = "ap4h" — same
// `Format`, different profile/codecID. `mapMediaInfoToClipMetadata`
// (src/metadata/mediainfo.ts, commit 80efa25) now threads this distinction
// through: when `Format_Profile` is `"RAW"` it yields `${Format} RAW` (e.g.
// `"ProRes RAW"`) instead of the bare `Format` string, so `ClipMetadata.codec`
// already matches this pattern for the Ronin-4D clip above without the
// caller needing to build a richer string itself. This pattern still also
// matches the raw CodecID (`aprn`) directly, for callers that only have that.
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

/** @deprecated Use {@link thumbnailRouteFor}. The app-side thumbnail queue
 * (`run-thumbnails.ts`, Task 4) has migrated to
 * `thumbnailRouteFor(clip.extension, codec)` and passes the clip's codec
 * through; this alias is kept only so any other caller still compiles. Note
 * this now genuinely returns `'preview'` for `.crm`/`.r3d` — it is a true
 * delegate, not a lossy narrowing to the old
 * `'mediabunny' | 'ffmpeg' | 'none'` domain. */
export type DecodePath = ThumbnailRoute

/** @deprecated Use {@link thumbnailRouteFor}. */
export function decodePathFor(extension: string): DecodePath {
  return thumbnailRouteFor(extension)
}
