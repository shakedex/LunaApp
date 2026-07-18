// Reel (camera roll) grouping. Divergence from the desktop ReelDetectionService
// is deliberate and documented in the plan: desktop pattern-gates the folder
// fallback (A001 / REEL_01); here ANY top-level folder groups, because the top
// folder is the card layout. reelName from container metadata always wins.
export interface ReelInput {
  relativePath: string
  reelName?: string
}

export interface DetectedReel<T extends ReelInput> {
  name: string
  clips: T[]
}

export const UNGROUPED_REEL = 'Ungrouped'

function reelKeyFor(clip: ReelInput): string {
  const fromMetadata = clip.reelName?.trim()
  if (fromMetadata) return fromMetadata
  const slash = clip.relativePath.indexOf('/')
  if (slash > 0) return clip.relativePath.slice(0, slash)
  return UNGROUPED_REEL
}

export function detectReels<T extends ReelInput>(clips: readonly T[]): DetectedReel<T>[] {
  const byName = new Map<string, T[]>()
  for (const clip of clips) {
    const key = reelKeyFor(clip)
    const group = byName.get(key)
    if (group) group.push(clip)
    else byName.set(key, [clip])
  }
  const reels = [...byName.entries()].map(([name, group]) => ({
    name,
    clips: [...group].sort((a, b) => a.relativePath.localeCompare(b.relativePath)),
  }))
  return reels.sort((a, b) => a.name.localeCompare(b.name, undefined, { numeric: true }))
}
