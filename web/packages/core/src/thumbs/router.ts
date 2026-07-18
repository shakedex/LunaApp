export type DecodePath = 'mediabunny' | 'ffmpeg' | 'none'

const MEDIABUNNY_EXTENSIONS = new Set(['.mp4', '.m4v', '.mov', '.mkv', '.webm', '.3gp'])
// .mts/.m2ts default to ffmpeg: AVCHD's 192-byte BDAV TS variant is
// unverified in mediabunny (spec §10.2). Promote after real-file QA.
const FFMPEG_EXTENSIONS = new Set(['.mxf', '.avi', '.mts', '.m2ts', '.wmv', '.flv'])

export function decodePathFor(extension: string): DecodePath {
  if (MEDIABUNNY_EXTENSIONS.has(extension)) return 'mediabunny'
  if (FFMPEG_EXTENSIONS.has(extension)) return 'ffmpeg'
  return 'none'
}
