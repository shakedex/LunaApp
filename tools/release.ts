#!/usr/bin/env bun
/**
 * Luna Web release tool — ZeroVer (0.MINOR.PATCH, there is never a 1.0).
 *
 *   bun run release <minor|patch> [--dry-run] [--no-push]
 *
 * Bumps apps/web/package.json, promotes the changelog's [Unreleased] section to a dated
 * version, commits, tags, and pushes (`--no-push` to skip). Deploys then run from the
 * Cloudflare dashboard git integration (see DEPLOY.md); nothing here runs wrangler deploy.
 */
import { $ } from 'bun'

const REPO = 'shakedex/LunaApp'
const PKG_PATH = 'apps/web/package.json'
const CHANGELOG_PATH = 'apps/docs/src/content/docs/changelog.md'

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
      `ZeroVer violated: "${input}" has major ${major}. Luna Web stays 0.x forever — there is never a 1.0.`,
    )
  }
  return { major: 0, minor, patch }
}

/** Compute the next version for a minor or patch bump. Major is locked to 0. */
export function nextVersion(current: string, bump: Bump): string {
  const v = parseVersion(current)
  if (bump === 'minor') return `0.${v.minor + 1}.0`
  if (bump === 'patch') return `0.${v.minor}.${v.patch + 1}`
  throw new Error(`Unknown bump "${bump}" (expected "minor" or "patch")`)
}

/**
 * The body of the `## [Unreleased]` section: everything up to the next `## [` heading,
 * with link-reference definitions stripped. Empty means there is nothing to release.
 * Throws if the changelog has no `## [Unreleased]` heading.
 */
export function unreleasedBody(text: string): string {
  const headingIndex = text.search(/^## \[Unreleased\][^\n]*$/m)
  if (headingIndex === -1) throw new Error('Changelog has no "## [Unreleased]" heading')
  const afterHeading = text.slice(headingIndex).replace(/^## \[Unreleased\][^\n]*\n/, '')
  const nextHeading = afterHeading.search(/^## \[/m)
  const body = nextHeading === -1 ? afterHeading : afterHeading.slice(0, nextHeading)
  return body.replace(/^\[[^\]]+\]:.*$/gm, '').trim()
}

/**
 * Promote `## [Unreleased]` to `## [version] - date`, leaving a fresh empty `[Unreleased]`
 * above it, and update the link-reference definitions at the bottom.
 */
export function rewriteChangelog(
  text: string,
  opts: { version: string; date: string; prevVersion: string },
): string {
  const { version, date, prevVersion } = opts
  if (!/^## \[Unreleased\][^\n]*$/m.test(text)) {
    throw new Error('Changelog has no "## [Unreleased]" heading')
  }

  let out = text.replace(
    /^## \[Unreleased\][^\n]*$/m,
    `## [Unreleased]\n\n## [${version}] - ${date}`,
  )

  const unreleasedLink = `[unreleased]: https://github.com/${REPO}/compare/v${version}...HEAD`
  const versionLink = `[${version}]: https://github.com/${REPO}/compare/v${prevVersion}...v${version}`
  if (/^\[unreleased\]:.*$/im.test(out)) {
    out = out.replace(/^\[unreleased\]:.*$/im, `${unreleasedLink}\n${versionLink}`)
  } else {
    out = `${out.trimEnd()}\n\n${unreleasedLink}\n${versionLink}\n`
  }
  return out
}

function usage(): never {
  console.error('Usage: bun run release <minor|patch> [--dry-run] [--no-push]')
  process.exit(1)
}

async function main(): Promise<void> {
  const argv = process.argv.slice(2)
  const dryRun = argv.includes('--dry-run')
  const noPush = argv.includes('--no-push')
  const bump = argv.find((a) => !a.startsWith('--'))

  if (bump === 'major') {
    throw new Error('ZeroVer: no major bumps. Luna Web is 0.x forever — use "minor" or "patch".')
  }
  if (bump !== 'minor' && bump !== 'patch') usage()

  const pkgText = await Bun.file(PKG_PATH).text()
  const current = (JSON.parse(pkgText) as { version: string }).version
  parseVersion(current) // asserts the current version is 0.x
  const next = nextVersion(current, bump)

  const changelogText = await Bun.file(CHANGELOG_PATH).text()
  const body = unreleasedBody(changelogText)

  if (dryRun) {
    console.log(`[dry-run] ${current} -> ${next}`)
    console.log(`[dry-run] [Unreleased] entries:\n${body || '(none)'}`)
    return
  }

  const status = (await $`git status --porcelain`.text()).trim()
  if (status) {
    throw new Error('Working tree is dirty. Commit or stash before releasing.')
  }
  if (!body) {
    throw new Error('Nothing to release: add entries under [Unreleased] in the changelog first.')
  }

  const date = new Date().toISOString().slice(0, 10)
  await Bun.write(PKG_PATH, pkgText.replace(/("version"\s*:\s*)"[^"]+"/, `$1"${next}"`))
  await Bun.write(
    CHANGELOG_PATH,
    rewriteChangelog(changelogText, { version: next, date, prevVersion: current }),
  )

  await $`git add -- ${PKG_PATH} ${CHANGELOG_PATH}`
  await $`git commit -m ${`chore(release): v${next}`}`
  await $`git tag -a ${`v${next}`} -m ${`v${next}`}`

  if (noPush) {
    console.log(`Released v${next} locally (--no-push). Push with: git push --follow-tags`)
    return
  }
  await $`git push --follow-tags`
  console.log(
    `Released and pushed v${next}. Cloudflare rebuilds master automatically (see DEPLOY.md).`,
  )
}

if (import.meta.main) {
  main().catch((err: unknown) => {
    console.error(err instanceof Error ? err.message : err)
    process.exit(1)
  })
}
