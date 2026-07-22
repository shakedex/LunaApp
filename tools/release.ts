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
import { rewriteChangelog, unreleasedBody } from './lib/changelog'
import { nextVersion, parseVersion } from './lib/version'

const PKG_PATH = 'apps/web/package.json'
const CHANGELOG_PATH = 'apps/docs/src/content/docs/changelog.md'

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
