const REPO = 'shakedex/LunaApp'

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
 * above it, and update the link-reference definitions at the bottom. `tagPrefix` selects the
 * git tag namespace the compare links point at: `v` for the web app, `desktop-v` for the shell.
 */
export function rewriteChangelog(
  text: string,
  opts: { version: string; date: string; prevVersion: string; tagPrefix?: string },
): string {
  const { version, date, prevVersion, tagPrefix = 'v' } = opts
  if (!/^## \[Unreleased\][^\n]*$/m.test(text)) {
    throw new Error('Changelog has no "## [Unreleased]" heading')
  }

  let out = text.replace(
    /^## \[Unreleased\][^\n]*$/m,
    `## [Unreleased]\n\n## [${version}] - ${date}`,
  )

  const unreleasedLink = `[unreleased]: https://github.com/${REPO}/compare/${tagPrefix}${version}...HEAD`
  const versionLink = `[${version}]: https://github.com/${REPO}/compare/${tagPrefix}${prevVersion}...${tagPrefix}${version}`
  if (/^\[unreleased\]:.*$/im.test(out)) {
    out = out.replace(/^\[unreleased\]:.*$/im, `${unreleasedLink}\n${versionLink}`)
  } else {
    out = `${out.trimEnd()}\n\n${unreleasedLink}\n${versionLink}\n`
  }
  return out
}
