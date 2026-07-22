import { describe, expect, test } from 'bun:test'
import { rewriteChangelog, unreleasedBody } from './lib/changelog'
import { nextVersion, parseVersion } from './lib/version'

describe('parseVersion', () => {
  test('parses a 0.x.y version', () => {
    expect(parseVersion('0.12.0')).toEqual({ major: 0, minor: 12, patch: 0 })
  })

  test('rejects major >= 1 (ZeroVer)', () => {
    expect(() => parseVersion('1.0.0')).toThrow(/ZeroVer/)
  })

  test('rejects a non-semver string', () => {
    expect(() => parseVersion('nope')).toThrow()
  })
})

describe('nextVersion', () => {
  test('minor bump increments minor and zeroes patch', () => {
    expect(nextVersion('0.12.3', 'minor')).toBe('0.13.0')
  })

  test('patch bump increments patch and keeps minor', () => {
    expect(nextVersion('0.12.3', 'patch')).toBe('0.12.4')
  })
})

const SAMPLE = `---
title: Changelog
---

All notable changes to Luna Web.

## [Unreleased]

### Added
- A new thing.

## [0.12.0] - 2026-07-20

### Added
- First tracked release.

[unreleased]: https://github.com/shakedex/LunaApp/compare/v0.12.0...HEAD
[0.12.0]: https://github.com/shakedex/LunaApp/releases/tag/v0.12.0
`

describe('unreleasedBody', () => {
  test('returns the entries when [Unreleased] has content', () => {
    expect(unreleasedBody(SAMPLE)).toContain('A new thing')
  })

  test('returns empty string when [Unreleased] is empty', () => {
    const empty = SAMPLE.replace('### Added\n- A new thing.\n\n', '')
    expect(unreleasedBody(empty)).toBe('')
  })

  test('throws when there is no [Unreleased] heading', () => {
    expect(() => unreleasedBody('# Changelog\n\n## [0.12.0] - 2026-07-20\n')).toThrow()
  })
})

describe('rewriteChangelog', () => {
  const out = rewriteChangelog(SAMPLE, {
    version: '0.13.0',
    date: '2026-08-01',
    prevVersion: '0.12.0',
  })

  test('leaves a fresh empty [Unreleased] above the new dated version', () => {
    expect(out).toMatch(/## \[Unreleased\]\s*\n\s*\n## \[0\.13\.0\] - 2026-08-01/)
  })

  test('moves the prior unreleased entries under the new version', () => {
    expect(out).toContain('- A new thing.')
  })

  test('rewrites the [unreleased] compare link to the new version', () => {
    expect(out).toContain(
      '[unreleased]: https://github.com/shakedex/LunaApp/compare/v0.13.0...HEAD',
    )
  })

  test('adds a compare link for the new version', () => {
    expect(out).toContain('[0.13.0]: https://github.com/shakedex/LunaApp/compare/v0.12.0...v0.13.0')
  })
})
