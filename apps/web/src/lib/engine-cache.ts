// Fetch a large engine binary once, keep it in the Cache API, and hand back
// a blob URL. Later visits (and offline use) hit the cache (spec §11).
export const ENGINE_CACHE_NAME = 'luna-engines-v1'

// Cold-cache single-flight: parallel pool lanes starting together must share
// one network fetch of the ~31 MB core, not race N duplicate downloads.
// Cleared on settle so a failed fetch can be retried.
const inflightAdds = new Map<string, Promise<void>>()

export async function cachedBlobUrl(url: string, mime: string): Promise<string> {
  const cache = await caches.open(ENGINE_CACHE_NAME)
  let response = await cache.match(url)
  if (!response) {
    let add = inflightAdds.get(url)
    if (!add) {
      add = cache.add(url).finally(() => inflightAdds.delete(url))
      inflightAdds.set(url, add)
    }
    await add
    response = await cache.match(url)
    if (!response) throw new Error(`Failed to cache engine asset: ${url}`)
  }
  const blob = await response.blob()
  // Per-caller object URL on purpose: each engine revokes its own URL after
  // load, so a shared URL would race one lane's revoke against another's load.
  return URL.createObjectURL(new Blob([blob], { type: mime }))
}
