import { normalizeSettings, type Settings } from '@luna-web/core'
import { getDb } from './db'

const SETTINGS_KEY = 'app'

export async function loadSettings(): Promise<Settings<Blob>> {
  const db = await getDb()
  return normalizeSettings<Blob>(await db.get('settings', SETTINGS_KEY))
}

export async function saveSettings(settings: Settings<Blob>): Promise<void> {
  const db = await getDb()
  await db.put('settings', settings, SETTINGS_KEY)
}
