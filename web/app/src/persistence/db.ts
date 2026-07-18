import { type DBSchema, type IDBPDatabase, openDB } from 'idb'
import type { StoredRecentSource } from './recent-sources'

interface LunaDb extends DBSchema {
  recentSources: { key: number; value: StoredRecentSource }
}

let dbPromise: Promise<IDBPDatabase<LunaDb>> | null = null

export function getDb(): Promise<IDBPDatabase<LunaDb>> {
  dbPromise ??= openDB<LunaDb>('luna-web', 1, {
    upgrade(db) {
      db.createObjectStore('recentSources', { autoIncrement: true })
    },
  })
  return dbPromise
}
