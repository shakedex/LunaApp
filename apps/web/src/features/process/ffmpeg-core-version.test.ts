import { readFileSync } from 'node:fs'
import { dirname, join } from 'node:path'
import { fileURLToPath } from 'node:url'
import { expect, test } from 'vite-plus/test'

const here = dirname(fileURLToPath(import.meta.url))

// FFMPEG_CORE_VERSION in ffmpeg-engine.ts mirrors the pinned @ffmpeg/core
// devDependency because the package's exports map blocks importing its
// version (see the comment there). This test fails when one is bumped
// without the other.
test('FFMPEG_CORE_VERSION matches the @ffmpeg/core devDependency', () => {
  const source = readFileSync(join(here, 'ffmpeg-engine.ts'), 'utf8')
  const constant = source.match(/const FFMPEG_CORE_VERSION = '([^']+)'/)?.[1]
  const pkg = JSON.parse(readFileSync(join(here, '../../../package.json'), 'utf8')) as {
    devDependencies: Record<string, string>
  }
  const declared = pkg.devDependencies['@ffmpeg/core'].replace(/^[~^]/, '')
  expect(constant).toBe(declared)
})
