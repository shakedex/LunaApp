import { type ActivitySnapshot, normalizeActivitySnapshot } from '@luna-web/core'
import { getDb } from './db'

const ACTIVITY_KEY = 'log'

export async function loadActivity(): Promise<ActivitySnapshot> {
  const db = await getDb()
  return normalizeActivitySnapshot(await db.get('activity', ACTIVITY_KEY))
}

export async function saveActivity(snapshot: ActivitySnapshot): Promise<void> {
  const db = await getDb()
  await db.put('activity', snapshot, ACTIVITY_KEY)
}

export async function clearPersistedActivity(): Promise<void> {
  const db = await getDb()
  await db.delete('activity', ACTIVITY_KEY)
}
