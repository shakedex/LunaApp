import { getDb } from './db'

export interface StoredRecentSource {
  name: string
  handle: FileSystemDirectoryHandle
  lastUsedAt: number
}

const MAX_RECENT = 10

export async function listRecentSources(): Promise<Array<{ key: number } & StoredRecentSource>> {
  const db = await getDb()
  const [keys, values] = await Promise.all([
    db.getAllKeys('recentSources'),
    db.getAll('recentSources'),
  ])
  return keys
    .map((key, i) => ({ key, ...values[i] }) as { key: number } & StoredRecentSource)
    .sort((a, b) => b.lastUsedAt - a.lastUsedAt)
}

export async function rememberSource(
  handle: FileSystemDirectoryHandle,
  now: number,
): Promise<void> {
  const db = await getDb()
  const [keys, values] = await Promise.all([
    db.getAllKeys('recentSources'),
    db.getAll('recentSources'),
  ])

  const duplicateKeys: number[] = []
  for (let i = 0; i < values.length; i++) {
    const existing = values[i]
    if (existing && (await existing.handle.isSameEntry(handle)))
      duplicateKeys.push(keys[i] as number)
  }

  const tx = db.transaction('recentSources', 'readwrite')
  for (const k of duplicateKeys) void tx.store.delete(k)
  void tx.store.add({ name: handle.name, handle, lastUsedAt: now })
  await tx.done

  const all = await listRecentSources()
  if (all.length > MAX_RECENT) {
    const tx2 = db.transaction('recentSources', 'readwrite')
    for (const stale of all.slice(MAX_RECENT)) void tx2.store.delete(stale.key)
    await tx2.done
  }
}

export async function forgetSource(key: number): Promise<void> {
  const db = await getDb()
  await db.delete('recentSources', key)
}
