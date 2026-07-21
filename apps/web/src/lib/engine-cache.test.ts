import { afterEach, expect, test, vi } from 'vite-plus/test'
import { cachedBlobUrl } from './engine-cache'

// Fake CacheStorage: miss until add() completes, count network fetches, and
// make add() take a tick so concurrent callers genuinely overlap.
function fakeCaches(failFirstAdd = false) {
  const store = new Map<string, Response>()
  let addCalls = 0
  const cache = {
    // Clone on every match: a real Cache API match() hands back a body that
    // hasn't been read yet, but this fake stores one Response per URL, and
    // several callers (concurrent or sequential) each call .blob() on it.
    match: async (url: string) => store.get(url)?.clone(),
    add: async (url: string) => {
      addCalls += 1
      await new Promise((r) => setTimeout(r, 10))
      if (failFirstAdd && addCalls === 1) throw new Error('network down')
      store.set(url, new Response(new Blob(['engine-bytes'])))
    },
  }
  return { caches: { open: async () => cache }, addCallCount: () => addCalls }
}

afterEach(() => {
  vi.unstubAllGlobals()
  vi.restoreAllMocks()
})

test('concurrent cold-cache callers share one network fetch', async () => {
  const fake = fakeCaches()
  vi.stubGlobal('caches', fake.caches)
  vi.spyOn(URL, 'createObjectURL').mockReturnValue('blob:fake')

  const urls = await Promise.all([
    cachedBlobUrl('https://cdn.test/one/core.wasm', 'application/wasm'),
    cachedBlobUrl('https://cdn.test/one/core.wasm', 'application/wasm'),
    cachedBlobUrl('https://cdn.test/one/core.wasm', 'application/wasm'),
  ])

  expect(fake.addCallCount()).toBe(1)
  expect(urls).toHaveLength(3)
})

test('warm cache never re-fetches', async () => {
  const fake = fakeCaches()
  vi.stubGlobal('caches', fake.caches)
  vi.spyOn(URL, 'createObjectURL').mockReturnValue('blob:fake')

  await cachedBlobUrl('https://cdn.test/two/core.wasm', 'application/wasm')
  await cachedBlobUrl('https://cdn.test/two/core.wasm', 'application/wasm')

  expect(fake.addCallCount()).toBe(1)
})

test('a failed fetch does not poison later retries', async () => {
  const fake = fakeCaches(true)
  vi.stubGlobal('caches', fake.caches)
  vi.spyOn(URL, 'createObjectURL').mockReturnValue('blob:fake')

  const first = Promise.all([
    cachedBlobUrl('https://cdn.test/three/core.wasm', 'application/wasm'),
    cachedBlobUrl('https://cdn.test/three/core.wasm', 'application/wasm'),
  ])
  await expect(first).rejects.toThrow('network down')

  // Retry after the failure: the in-flight memo must have been cleared.
  await expect(
    cachedBlobUrl('https://cdn.test/three/core.wasm', 'application/wasm'),
  ).resolves.toBeTruthy()
  expect(fake.addCallCount()).toBe(2)
})
