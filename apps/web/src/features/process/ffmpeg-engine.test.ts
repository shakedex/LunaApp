import { afterEach, beforeEach, expect, test, vi } from 'vite-plus/test'
import { createFfmpegEngine } from './ffmpeg-engine'

// createFfmpegEngine owns its FFmpeg instance, so module mocking is the seam.
// The fake has to model the @ffmpeg/ffmpeg behaviour the sticky-dispose flag
// exists for: terminate() before load() has created the worker does nothing
// and leaves no mark, so the instance would load again afterwards.
const hooks = vi.hoisted(() => ({
  download: Promise.resolve(),
  load: Promise.resolve(),
  wasmFails: false,
  loadCalls: 0,
  terminateCalls: 0,
  downloadCalls: 0,
}))

vi.mock('@ffmpeg/ffmpeg', () => ({
  FFFSType: { WORKERFS: 'WORKERFS' },
  FFmpeg: class {
    async load() {
      hooks.loadCalls++
      await hooks.load
    }
    terminate() {
      hooks.terminateCalls++
    }
  },
}))

// Only the download is faked — revokeSettled stays real, so a test can hold
// ensureLoaded's cleanup to account. The type argument is spelled relative
// because `vp lint` does not resolve the '@' alias inside an inline import
// type (it does for the specifier itself).
vi.mock('@/lib/engine-cache', async () => ({
  ...(await vi.importActual<typeof import('../../lib/engine-cache')>('@/lib/engine-cache')),
  cachedBlobUrl: async (url: string) => {
    hooks.downloadCalls++
    await hooks.download
    if (hooks.wasmFails && url.endsWith('.wasm')) throw new Error('network down')
    return `blob:${url}`
  },
}))

function deferred(): { promise: Promise<void>; resolve: () => void } {
  let resolve!: () => void
  const promise = new Promise<void>((r) => {
    resolve = r
  })
  return { promise, resolve }
}

let revoked: string[] = []

beforeEach(() => {
  hooks.download = Promise.resolve()
  hooks.load = Promise.resolve()
  hooks.wasmFails = false
  hooks.loadCalls = 0
  hooks.terminateCalls = 0
  hooks.downloadCalls = 0
  revoked = []
  vi.spyOn(URL, 'revokeObjectURL').mockImplementation((url: string) => {
    revoked.push(url)
  })
})

afterEach(() => {
  vi.restoreAllMocks()
})

function clip(): File {
  return new File([new Uint8Array(1)], 'A001C002.mov')
}

test('disposing inside the cold-start download never creates a worker', async () => {
  const download = deferred()
  hooks.download = download.promise
  const engine = createFfmpegEngine()
  const settled = engine.thumbnails(clip(), [0], 320).catch((err: unknown) => err)

  engine.dispose()
  download.resolve()

  expect(await settled).toBeInstanceOf(Error)
  expect(hooks.loadCalls).toBe(0)
  // Both object URLs go back even though the load was abandoned.
  expect(revoked).toHaveLength(2)
})

test('disposing while load() is in flight cannot leave the body running', async () => {
  const load = deferred()
  hooks.load = load.promise
  const engine = createFfmpegEngine()
  const settled = engine.thumbnails(clip(), [0], 320).catch((err: unknown) => err)
  await vi.waitFor(() => {
    expect(hooks.loadCalls).toBe(1)
  })

  // terminate() #1, from dispose(). The real terminate() would reject this
  // in-flight load() too, but the fake deliberately does not — it only
  // counts the call — so load() still resolves and the post-load `disposed`
  // guard in ensureLoaded() actually runs, which is what this test is for.
  engine.dispose()
  load.resolve()

  expect(await settled).toBeInstanceOf(Error)
  // #2 is that guard's belt-and-braces terminate(), reachable in this test
  // only because the fake diverges from the real library on the point above.
  expect(hooks.terminateCalls).toBe(2)
  expect(revoked).toHaveLength(2)
})

// Pins ensureLoaded's Promise.allSettled: under Promise.all the rejection
// escapes before the sibling's ~31 MB object URL can be revoked, and nothing
// else ever holds a reference to it.
test('a download that fails alongside a successful one revokes the survivor', async () => {
  hooks.wasmFails = true
  const engine = createFfmpegEngine()

  await expect(engine.thumbnails(clip(), [0], 320)).rejects.toThrow('network down')
  expect(revoked).toEqual([expect.stringContaining('ffmpeg-core.js')])
  expect(hooks.loadCalls).toBe(0)
})

test('a disposed engine stays disposed when the pool retries the item', async () => {
  const engine = createFfmpegEngine()
  engine.dispose()
  await expect(engine.thumbnails(clip(), [0], 320)).rejects.toThrow(/disposed/)
  await expect(engine.thumbnails(clip(), [0], 320)).rejects.toThrow(/disposed/)
  expect(hooks.loadCalls).toBe(0)
  // Both calls land on the disposed-first-line guard in thumbnails(), so
  // neither ever reaches ensureLoaded() — no download is even attempted.
  expect(hooks.downloadCalls).toBe(0)
})
