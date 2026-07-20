import { type DBSchema, type IDBPDatabase, openDB } from 'idb'
import type { StoredRecentSource } from './recent-sources'

interface LunaDb extends DBSchema {
  recentSources: { key: number; value: StoredRecentSource }
  // Value is `unknown` on purpose: records may have been written by any past
  // or future app version — normalizeSettings() is the only trusted reader.
  settings: { key: string; value: unknown }
  // One record ('log') holding the persisted ActivitySnapshot; `unknown` for
  // the same reason as settings — normalizeActivitySnapshot is the only reader.
  activity: { key: string; value: unknown }
  // Report Library: summaries are tiny and listed often; models carry the
  // frame Blobs and are only read when a saved report is opened.
  reportSummaries: { key: string; value: unknown }
  reportModels: { key: string; value: unknown }
}

let dbPromise: Promise<IDBPDatabase<LunaDb>> | null = null

export const DB_NAME = 'luna-web'

export function getDb(): Promise<IDBPDatabase<LunaDb>> {
  dbPromise ??= openDB<LunaDb>(DB_NAME, 4, {
    upgrade(db, oldVersion) {
      // Guard each store by the version that introduced it: fresh installs
      // enter with oldVersion 0 and must create everything.
      if (oldVersion < 1) db.createObjectStore('recentSources', { autoIncrement: true })
      if (oldVersion < 2) db.createObjectStore('settings')
      if (oldVersion < 3) db.createObjectStore('activity')
      if (oldVersion < 4) {
        db.createObjectStore('reportSummaries')
        db.createObjectStore('reportModels')
      }
    },
  })
  return dbPromise
}

// Required before deleteDB: an open connection would block deletion forever.
export async function closeDb(): Promise<void> {
  if (!dbPromise) return
  const db = await dbPromise.catch(() => null)
  dbPromise = null
  db?.close()
}
