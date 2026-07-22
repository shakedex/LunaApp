import { invoke } from '@tauri-apps/api/core'

/** True when running inside the Luna desktop (Tauri) shell. */
export function isTauri(): boolean {
  // invoke() dereferences window.__TAURI_INTERNALS__, so we check it here
  // rather than the SDK's isTauri, which reads a different global (globalThis.isTauri).
  return typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window
}

/** Desktop shell version, or null in the browser. */
export async function getDesktopVersion(): Promise<string | null> {
  if (!isTauri()) return null
  return invoke<string>('app_version')
}
