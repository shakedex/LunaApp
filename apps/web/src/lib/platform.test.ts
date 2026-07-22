import { expect, test } from 'vite-plus/test'
import { getDesktopVersion, isTauri } from './platform'

test('isTauri is false outside a Tauri webview', () => {
  expect(isTauri()).toBe(false)
})

test('getDesktopVersion resolves null outside a Tauri webview', async () => {
  expect(await getDesktopVersion()).toBeNull()
})
