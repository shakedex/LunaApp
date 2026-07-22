import { expect, test } from 'vite-plus/test'
import { getDesktopVersion, isTauri } from './platform'

test('isTauri is false outside a Tauri webview', () => {
  expect(isTauri()).toBe(false)
})

test('getDesktopVersion resolves null outside a Tauri webview', async () => {
  expect(await getDesktopVersion()).toBeNull()
})

test('isTauri is true when the Tauri IPC bridge is present', () => {
  const g = globalThis as { window?: unknown }
  g.window = { __TAURI_INTERNALS__: {} }
  try {
    expect(isTauri()).toBe(true)
  } finally {
    delete g.window
  }
})
