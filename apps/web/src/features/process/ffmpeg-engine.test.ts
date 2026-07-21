import { afterEach, beforeEach, expect, test, vi } from 'vite-plus/test'
import { createFfmpegEngine } from './ffmpeg-engine'

// createFfmpegEngine owns its FFmpeg instance, so module mocking is the seam.
// The fake has to model the @ffmpeg/ffmpeg behaviour the sticky-dispose flag
// exists for: terminate() before load() has created the worker does nothing
// and leaves no mark, so the instance would load again afterwards.
const hooks = vi.hoisted(() => ({
  download: Promise.resolve(),
  load: Promise.resolve(),
  loadCalls: 0,
  terminateCalls: 0,
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

vi.mock('@/lib/engine-cache', () => ({
  cachedBlobUrl: async (url: string) => {
    await hooks.download
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
  hooks.loadCalls = 0
  hooks.terminateCalls = 0
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

test('disposing inside load() terminates the worker load created', async () => {
  const load = deferred()
  hooks.load = load.promise
  const engine = createFfmpegEngine()
  const settled = engine.thumbnails(clip(), [0], 320).catch((err: unknown) => err)
  await vi.waitFor(() => {
    expect(hooks.loadCalls).toBe(1)
  })

  engine.dispose() // terminate() #1: no worker yet, so it does nothing
  load.resolve()

  expect(await settled).toBeInstanceOf(Error)
  expect(hooks.terminateCalls).toBe(2) // #2 is the one that actually tears down
  expect(revoked).toHaveLength(2)
})

test('a disposed engine stays disposed when the pool retries the item', async () => {
  const engine = createFfmpegEngine()
  engine.dispose()
  await expect(engine.thumbnails(clip(), [0], 320)).rejects.toThrow(/disposed/)
  await expect(engine.thumbnails(clip(), [0], 320)).rejects.toThrow(/disposed/)
  expect(hooks.loadCalls).toBe(0)
})
