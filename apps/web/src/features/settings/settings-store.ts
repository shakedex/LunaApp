import { defaultSettings, type Settings } from '@luna-web/core'
import { Store } from '@tanstack/store'
import { setCoverFields } from '@/features/report/cover-store'
import { logger } from '@/lib/logger'
import { loadSettings, saveSettings } from '@/persistence/settings'

export const settingsStore = new Store<Settings<Blob>>(defaultSettings<Blob>())

// Called once before first render (main.tsx): the cover form reads its store
// once per mount, so persisted cover defaults must land before React does.
export async function hydrateSettings(): Promise<void> {
  const loaded = await loadSettings()
  settingsStore.setState(() => loaded)
  if (Object.keys(loaded.coverDefaults).length > 0) {
    // Seed cover fields from defaults. `date` is never part of coverDefaults
    // (normalizeSettings strips it) so the today-seed in coverStore survives.
    setCoverFields(loaded.coverDefaults)
  }
}

export async function updateSettings(
  patch: Partial<Omit<Settings<Blob>, 'schemaVersion'>>,
): Promise<void> {
  settingsStore.setState((s) => ({ ...s, ...patch }))
  await saveSettings(settingsStore.state)
  logger.debug('Settings saved')
}
