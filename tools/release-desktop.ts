#!/usr/bin/env bun
/**
 * Luna Desktop release tool — ZeroVer (0.MINOR.PATCH, there is never a 1.0).
 *
 *   bun run release:desktop <minor|patch> [--dry-run] [--no-push]
 *
 * The desktop shell versions independently of the web app. Bumps all three version files
 * plus Cargo.lock, promotes apps/desktop/CHANGELOG.md's [Unreleased] section, commits, and
 * tags `desktop-v<version>` — a namespace deliberately distinct from the web app's `v<version>`.
 * Pushing the tag is what triggers .github/workflows/desktop-release.yml to build and attach
 * Windows and macOS artifacts to a DRAFT GitHub Release.
 */
import { $ } from 'bun'
import { rewriteChangelog, unreleasedBody } from './lib/changelog'
import { nextVersion, parseVersion } from './lib/version'

const PKG_PATH = 'apps/desktop/package.json'
const CARGO_PATH = 'apps/desktop/src-tauri/Cargo.toml'
const CONF_PATH = 'apps/desktop/src-tauri/tauri.conf.json'
const LOCK_PATH = 'apps/desktop/src-tauri/Cargo.lock'
const CHANGELOG_PATH = 'apps/desktop/CHANGELOG.md'
const TAG_PREFIX = 'desktop-v'

export interface DesktopVersionFiles {
  pkg: string
  cargo: string
  conf: string
}

const PACKAGE_BLOCK = /\[package\][\s\S]*?(?=\n\[|$)/
const PACKAGE_VERSION = /^version\s*=\s*"[^"]+"/m

/**
 * Rewrite the version in all three desktop manifests. The Cargo.toml edit is bounded to the
 * `[package]` block — from the `[package]` header up to the next `[section]` header or end of
 * file — so it cannot rewrite a dependency's version, even when `[package]` has no literal
 * `version = "..."` line (e.g. `version.workspace = true`), in which case it throws instead of
 * scanning past the block.
 */
export function bumpVersionFiles(files: DesktopVersionFiles, next: string): DesktopVersionFiles {
  const match = PACKAGE_BLOCK.exec(files.cargo)
  if (!match) {
    throw new Error(`No [package] section found in ${CARGO_PATH}`)
  }
  const block = match[0]
  if (!PACKAGE_VERSION.test(block)) {
    throw new Error(`No literal version = "..." line in [package] section of ${CARGO_PATH}`)
  }
  const bumpedBlock = block.replace(PACKAGE_VERSION, `version = "${next}"`)
  const start = match.index
  const end = start + block.length
  return {
    pkg: files.pkg.replace(/("version"\s*:\s*)"[^"]+"/, `$1"${next}"`),
    conf: files.conf.replace(/("version"\s*:\s*)"[^"]+"/, `$1"${next}"`),
    cargo: files.cargo.slice(0, start) + bumpedBlock + files.cargo.slice(end),
  }
}

/** Pull the most useful detail out of a failed shell command: stderr if present, else the error's own message. */
function shellErrorDetail(err: unknown): string {
  if (err && typeof err === 'object' && 'stderr' in err) {
    const stderr = String((err as { stderr: unknown }).stderr).trim()
    if (stderr) return stderr
  }
  return err instanceof Error ? err.message : String(err)
}

function usage(): never {
  console.error('Usage: bun run release:desktop <minor|patch> [--dry-run] [--no-push]')
  process.exit(1)
}

async function main(): Promise<void> {
  const argv = process.argv.slice(2)
  const dryRun = argv.includes('--dry-run')
  const noPush = argv.includes('--no-push')
  const bump = argv.find((a) => !a.startsWith('--'))

  if (bump === 'major') {
    throw new Error('ZeroVer: no major bumps. Luna stays 0.x forever — use "minor" or "patch".')
  }
  if (bump !== 'minor' && bump !== 'patch') usage()

  const pkgText = await Bun.file(PKG_PATH).text()
  const current = (JSON.parse(pkgText) as { version: string }).version
  parseVersion(current)
  const next = nextVersion(current, bump)

  const changelogText = await Bun.file(CHANGELOG_PATH).text()
  const body = unreleasedBody(changelogText)

  if (dryRun) {
    console.log(`[dry-run] desktop ${current} -> ${next} (tag ${TAG_PREFIX}${next})`)
    console.log(`[dry-run] [Unreleased] entries:\n${body || '(none)'}`)
    return
  }

  const status = (await $`git status --porcelain`.text()).trim()
  if (status) {
    throw new Error('Working tree is dirty. Commit or stash before releasing.')
  }
  if (!body) {
    throw new Error(`Nothing to release: add entries under [Unreleased] in ${CHANGELOG_PATH}.`)
  }

  const bumped = bumpVersionFiles(
    {
      pkg: pkgText,
      cargo: await Bun.file(CARGO_PATH).text(),
      conf: await Bun.file(CONF_PATH).text(),
    },
    next,
  )
  await Bun.write(PKG_PATH, bumped.pkg)
  await Bun.write(CARGO_PATH, bumped.cargo)
  await Bun.write(CONF_PATH, bumped.conf)

  // Cargo.lock carries the `app` package's version too. Left stale, the next build rewrites it
  // and dirties the tree — which would fail this tool's own clean-tree check next time.
  // `cargo metadata` re-resolves and rewrites the lock without compiling.
  try {
    await $`cargo metadata --format-version 1 --manifest-path ${CARGO_PATH}`.quiet()
  } catch (err) {
    throw new Error(
      `cargo metadata failed, so Cargo.lock was not refreshed: ${shellErrorDetail(err)}\n` +
        `${PKG_PATH}, ${CARGO_PATH}, and ${CONF_PATH} were already rewritten — the working tree is dirty; revert or finish the release by hand.\n` +
        `(Missing Rust toolchain? Install rustup and retry.)`,
    )
  }

  const date = new Date().toISOString().slice(0, 10)
  await Bun.write(
    CHANGELOG_PATH,
    rewriteChangelog(changelogText, {
      version: next,
      date,
      prevVersion: current,
      tagPrefix: TAG_PREFIX,
    }),
  )

  await $`git add -- ${PKG_PATH} ${CARGO_PATH} ${CONF_PATH} ${LOCK_PATH} ${CHANGELOG_PATH}`
  await $`git commit -m ${`chore(release): desktop v${next}`}`
  await $`git tag -a ${`${TAG_PREFIX}${next}`} -m ${`Luna Desktop v${next}`}`

  if (noPush) {
    console.log(`Prepared desktop v${next} locally (--no-push). Push with: git push --follow-tags`)
    return
  }
  await $`git push --follow-tags`
  console.log(
    `Pushed ${TAG_PREFIX}${next}. GitHub Actions is building Windows and macOS artifacts into a DRAFT release — review and publish it manually.`,
  )
}

if (import.meta.main) {
  main().catch((err: unknown) => {
    console.error(err instanceof Error ? err.message : err)
    process.exit(1)
  })
}
