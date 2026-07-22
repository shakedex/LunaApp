import { describe, expect, test } from 'bun:test'

const CONF_PATH = 'apps/desktop/src-tauri/tauri.conf.json'

interface TauriConfig {
  build: {
    frontendDist: string
    beforeBuildCommand: string
  }
}

const conf = JSON.parse(await Bun.file(CONF_PATH).text()) as TauriConfig

// Tripwire, not a config validator: these two fields are the fix for an absolute-path leak
// (see apps/desktop/CHANGELOG.md / the leak-fix commit). Reverting either one is silent —
// the app still builds and runs — but it re-embeds the build machine's absolute filesystem
// path in every shipped binary. If you're "simplifying" this back, don't: read up on the
// leak fix first.
describe('tauri.conf.json desktop build config', () => {
  test('frontendDist points at the worker-free desktop build, not dist (which leaks an absolute path)', () => {
    expect(conf.build.frontendDist).toBe('../../web/dist-desktop')
  })

  test('beforeBuildCommand runs build:desktop, not plain build (which produces the leaking dist)', () => {
    expect(conf.build.beforeBuildCommand).toContain('build:desktop')
  })
})
