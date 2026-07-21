import { afterEach, expect, test, vi } from 'vite-plus/test'
import { withCancellation } from './run-processing'
import { isDecoderFailure, isTimeout } from './run-thumbnails'

// The helper's poll interval; the fake-clock tests must outrun it.
const POLL_MS = 250

function never(): Promise<string> {
  return new Promise<string>(() => {})
}

afterEach(() => {
  vi.useRealTimers()
})

test('rejects a superseded item without waiting for its work to finish', async () => {
  let cancelled = false
  const raced = withCancellation(never, () => cancelled, 'A001C002.mov')
  cancelled = true
  await expect(raced).rejects.toThrow('A001C002.mov: cancelled')
})

test('never starts work that is already cancelled', async () => {
  let started = false
  const raced = withCancellation(
    async () => {
      started = true
      return 'frames'
    },
    () => true,
    'A001C002.mov',
  )
  await expect(raced).rejects.toThrow('cancelled')
  expect(started).toBe(false)
})

// isDecoderFailure() routes a message into the ffmpeg cascade and isTimeout()
// into the preview salvage — a cancellation must read as neither, or a
// superseded lane's clip gets re-queued instead of ending.
test('the rejection message is neither a decoder failure nor a timeout', async () => {
  const err = await withCancellation(never, () => true, 'A001C002.mov').catch((e: unknown) => e)
  const msg = err instanceof Error ? err.message : String(err)
  expect(isDecoderFailure(msg)).toBe(false)
  expect(isTimeout(msg)).toBe(false)
  expect(msg).toBe('A001C002.mov: cancelled')
})

test('clears its poll timer when the work settles first', async () => {
  vi.useFakeTimers()
  await expect(
    withCancellation(
      () => Promise.resolve('frames'),
      () => false,
      'A001C002.mov',
    ),
  ).resolves.toBe('frames')
  expect(vi.getTimerCount()).toBe(0)
})

test('clears its poll timer when the work rejects first', async () => {
  vi.useFakeTimers()
  await expect(
    withCancellation(
      () => Promise.reject(new Error('boom')),
      () => false,
      'A001C002.mov',
    ),
  ).rejects.toThrow('boom')
  expect(vi.getTimerCount()).toBe(0)
})

test('clears its poll timer when cancellation wins', async () => {
  vi.useFakeTimers()
  let cancelled = false
  const raced = withCancellation(never, () => cancelled, 'A001C002.mov')
  // Attach the handler before advancing: the fake clock fires synchronously,
  // and an unhandled rejection fails the run even though the test passes.
  const settled = raced.catch((err: unknown) => err)
  cancelled = true
  await vi.advanceTimersByTimeAsync(POLL_MS)
  expect(await settled).toEqual(new Error('A001C002.mov: cancelled'))
  expect(vi.getTimerCount()).toBe(0)
})
