import { describe, expect, test } from 'bun:test'
import { rewriteChangelog } from './changelog'

const SAMPLE = `# Changelog

## [Unreleased]

### Added
- A desktop thing.

## [0.1.0] - 2026-07-22

### Added
- First desktop build.

[unreleased]: https://github.com/shakedex/LunaApp/compare/desktop-v0.1.0...HEAD
`

describe('rewriteChangelog with a tag prefix', () => {
  const out = rewriteChangelog(SAMPLE, {
    version: '0.2.0',
    date: '2026-08-01',
    prevVersion: '0.1.0',
    tagPrefix: 'desktop-v',
  })

  test('points the unreleased compare link at the prefixed tag', () => {
    expect(out).toContain(
      '[unreleased]: https://github.com/shakedex/LunaApp/compare/desktop-v0.2.0...HEAD',
    )
  })

  test('adds a compare link between the prefixed tags', () => {
    expect(out).toContain(
      '[0.2.0]: https://github.com/shakedex/LunaApp/compare/desktop-v0.1.0...desktop-v0.2.0',
    )
  })

  test('never emits a bare v-prefixed desktop tag', () => {
    expect(out).not.toMatch(/compare\/v0\.2\.0/)
  })

  test('defaults to the bare v prefix when none is given', () => {
    const web = rewriteChangelog(SAMPLE, {
      version: '0.2.0',
      date: '2026-08-01',
      prevVersion: '0.1.0',
    })
    expect(web).toContain('compare/v0.2.0...HEAD')
  })
})
