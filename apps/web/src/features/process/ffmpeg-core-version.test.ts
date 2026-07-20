import { expect, test } from 'bun:test'
import { join } from 'node:path'

// FFMPEG_CORE_VERSION in ffmpeg-engine.ts mirrors the pinned @ffmpeg/core
// devDependency because the package's exports map blocks importing its
// version (see the comment there). This test fails when one is bumped
// without the other.
test('FFMPEG_CORE_VERSION matches the @ffmpeg/core devDependency', async () => {
  const source = await Bun.file(join(import.meta.dir, 'ffmpeg-engine.ts')).text()
  const constant = source.match(/const FFMPEG_CORE_VERSION = '([^']+)'/)?.[1]
  const pkg = await Bun.file(join(import.meta.dir, '../../../package.json')).json()
  const declared = (pkg.devDependencies['@ffmpeg/core'] as string).replace(/^[~^]/, '')
  expect(constant).toBe(declared)
})
