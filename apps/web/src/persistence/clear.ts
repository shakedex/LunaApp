import { deleteDB } from 'idb'
import { ENGINE_CACHE_NAME } from '@/lib/engine-cache'
import { closeDb, DB_NAME } from './db'

// Settings "Clear local data": everything Luna keeps on this machine —
// settings, cover defaults, recent folders, the activity log, and the cached
// ffmpeg engine. Footage is never stored, so there is nothing else to clear.
export async function clearLocalData(): Promise<void> {
  await closeDb()
  await deleteDB(DB_NAME, {
    blocked: () => console.warn('[luna] Waiting for other tabs to release the database…'),
  })
  await caches.delete(ENGINE_CACHE_NAME).catch(() => false)
  window.location.reload()
}
