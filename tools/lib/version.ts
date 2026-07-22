export type Bump = 'minor' | 'patch'
export interface Version {
  major: 0
  minor: number
  patch: number
}

/** Parse a `0.MINOR.PATCH` version, asserting the ZeroVer major of 0. */
export function parseVersion(input: string): Version {
  const match = /^(\d+)\.(\d+)\.(\d+)$/.exec(input.trim())
  if (!match) throw new Error(`Not a semver version: "${input}"`)
  const major = Number(match[1])
  const minor = Number(match[2])
  const patch = Number(match[3])
  if (major !== 0) {
    throw new Error(
      `ZeroVer violated: "${input}" has major ${major}. Luna stays 0.x forever — there is never a 1.0.`,
    )
  }
  return { major: 0, minor, patch }
}

/** Compute the next version for a minor or patch bump. Major is locked to 0. */
export function nextVersion(current: string, bump: Bump): string {
  const v = parseVersion(current)
  if (bump === 'minor') return `0.${v.minor + 1}.0`
  if (bump === 'patch') return `0.${v.minor}.${v.patch + 1}`
  throw new Error(`Unknown bump "${String(bump)}" (expected "minor" or "patch")`)
}
