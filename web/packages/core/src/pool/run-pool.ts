// Generic lane pool extracted from the metadata orchestration (Plan 03).
// A "lane" is a reusable resource (e.g. a worker) owned by one concurrent
// loop. Failure recycles the lane (destroy + fresh create) and retries the
// same item up to maxAttempts. Cancellation is cooperative: checked before
// each claim; in-flight work finishes and its result is delivered to the
// handlers (callers drop late writes via their own guards).
export interface PoolHooks<L, T, R> {
  createLane(): L | Promise<L>
  destroyLane(lane: L): void
  run(lane: L, item: T): Promise<R>
}

export interface PoolHandlers<T, R> {
  onItemStart?(item: T): void
  onItemSuccess(item: T, result: R): void
  onItemFailure(item: T, error: unknown): void
}

export interface PoolOptions {
  concurrency: number
  maxAttempts?: number
  isCancelled?(): boolean
}

export async function runPool<L, T, R>(
  items: readonly T[],
  hooks: PoolHooks<L, T, R>,
  handlers: PoolHandlers<T, R>,
  options: PoolOptions,
): Promise<void> {
  if (items.length === 0) return
  const maxAttempts = options.maxAttempts ?? 2
  const isCancelled = options.isCancelled ?? (() => false)
  const laneCount = Math.max(1, Math.min(options.concurrency, items.length))
  let nextIndex = 0

  async function laneLoop(): Promise<void> {
    let lane: L | null = null
    try {
      lane = await hooks.createLane()
      for (;;) {
        if (isCancelled()) return
        const index = nextIndex++
        if (index >= items.length) return
        const item = items[index] as T
        handlers.onItemStart?.(item)
        let attempt = 0
        for (;;) {
          try {
            if (lane === null) lane = await hooks.createLane()
            const result = await hooks.run(lane, item)
            handlers.onItemSuccess(item, result)
            break
          } catch (err) {
            if (lane !== null) hooks.destroyLane(lane)
            lane = null
            attempt += 1
            if (attempt >= maxAttempts) {
              handlers.onItemFailure(item, err)
              break
            }
          }
        }
      }
    } finally {
      if (lane !== null) hooks.destroyLane(lane)
    }
  }

  await Promise.all(Array.from({ length: laneCount }, () => laneLoop()))
}
