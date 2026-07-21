// Reel (camera roll) grouping. Priority per clip: the deepest folder in its
// path that looks like a reel (A001, s004, REEL_01) > the reel name the camera
// embedded in the clip's metadata > its first folder past the wrapper prefix >
// Ungrouped. A wrapper prefix (a lone top folder like CAMERA/ shared by every
// scanned file) is measured once by wrapperPrefixDepth, so picking a master
// project folder groups the same as picking the wrapper folder itself.
export interface ReelInput {
  relativePath: string
  reelName?: string
}

export interface DetectedReel<T extends ReelInput> {
  name: string
  clips: T[]
}

export interface DetectReelsOptions {
  /** Leading path segments to ignore, as measured by {@link wrapperPrefixDepth}. */
  prefixDepth?: number
}

export const UNGROUPED_REEL = 'Ungrouped'

// Classic reel/card folder shapes: A001, B005, s004, DW01, REEL_01, Reel 2.
// Deliberately conservative — a miss falls back to sane folder grouping,
// while a false hit would split a reel on a junk vendor folder.
const REEL_FOLDER_PATTERNS = [/^[a-z]{1,2}_?\d{2,4}$/i, /^reel[_ -]?\d+$/i]

export function isReelFolderName(name: string): boolean {
  return REEL_FOLDER_PATTERNS.some((pattern) => pattern.test(name))
}

/** Reel-name ordering used everywhere reels are listed: numeric-aware so
 *  A002 < A010. */
export function compareReelNames(a: string, b: string): number {
  return a.localeCompare(b, undefined, { numeric: true })
}

/** How many leading path segments are wrapper folders to skip when grouping:
 * a wrapper level is a single top folder shared by EVERY path that is not
 * itself reel-like and whose removal leaves every file at least one folder
 * deep. Compute over ALL scanned paths (clips + other files) so both group
 * against the same prefix. */
export function wrapperPrefixDepth(paths: readonly string[]): number {
  let depth = 0
  let current: readonly string[] = paths
  for (;;) {
    const head = current[0]
    if (!head) return depth
    const slash = head.indexOf('/')
    if (slash <= 0) return depth
    const top = head.slice(0, slash)
    if (isReelFolderName(top)) return depth
    const prefix = `${top}/`
    const stripped: string[] = []
    for (const path of current) {
      if (!path.startsWith(prefix)) return depth
      const rest = path.slice(prefix.length)
      // Stripping this level would leave a file with no folder at all.
      if (!rest.includes('/')) return depth
      stripped.push(rest)
    }
    current = stripped
    depth += 1
  }
}

function reelKeyFor(clip: ReelInput, prefixDepth: number): string {
  const segments = clip.relativePath.split('/')
  const dirs = segments.slice(prefixDepth, -1)
  for (let i = dirs.length - 1; i >= 0; i--) {
    const dir = dirs[i]
    if (dir && isReelFolderName(dir)) return dir
  }
  const fromMetadata = clip.reelName?.trim()
  if (fromMetadata) return fromMetadata
  const first = dirs[0]
  if (first) return first
  return UNGROUPED_REEL
}

export function detectReels<T extends ReelInput>(
  clips: readonly T[],
  options?: DetectReelsOptions,
): DetectedReel<T>[] {
  const prefixDepth = options?.prefixDepth ?? 0
  const byName = new Map<string, T[]>()
  for (const clip of clips) {
    const key = reelKeyFor(clip, prefixDepth)
    const group = byName.get(key)
    if (group) group.push(clip)
    else byName.set(key, [clip])
  }
  const reels = [...byName.entries()].map(([name, group]) => ({
    name,
    clips: [...group].sort((a, b) => a.relativePath.localeCompare(b.relativePath)),
  }))
  return reels.sort((a, b) => compareReelNames(a.name, b.name))
}
