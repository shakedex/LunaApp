import { type DBSchema, type IDBPDatabase, openDB } from 'idb'
import type { StoredRecentSource } from './recent-sources'

interface LunaDb extends DBSchema {
  recentSources: { key: number; value: StoredRecentSource }
  // Value is `unknown` on purpose: records may have been written by any past
  // or future app version — normalizeSettings() is the only trusted reader.
  settings: { key: string; value: unknown }
}

let dbPromise: Promise<IDBPDatabase<LunaDb>> | null = null

export function getDb(): Promise<IDBPDatabase<LunaDb>> {
  dbPromise ??= openDB<LunaDb>('luna-web', 2, {
    upgrade(db, oldVersion) {
      // Guard each store by the version that introduced it: fresh installs
      // enter with oldVersion 0 and must create everything.
      if (oldVersion < 1) db.createObjectStore('recentSources', { autoIncrement: true })
      if (oldVersion < 2) db.createObjectStore('settings')
    },
  })
  return dbPromise
}
