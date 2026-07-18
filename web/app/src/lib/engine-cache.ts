// Fetch a large engine binary once, keep it in the Cache API, and hand back
// a blob URL. Later visits (and offline use) hit the cache (spec §11).
const CACHE_NAME = 'luna-engines-v1'

export async function cachedBlobUrl(url: string, mime: string): Promise<string> {
  const cache = await caches.open(CACHE_NAME)
  let response = await cache.match(url)
  if (!response) {
    await cache.add(url)
    response = await cache.match(url)
    if (!response) throw new Error(`Failed to cache engine asset: ${url}`)
  }
  const blob = await response.blob()
  return URL.createObjectURL(new Blob([blob], { type: mime }))
}
