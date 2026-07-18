import { describe, expect, test } from 'bun:test'
import { runPool } from '../src/pool/run-pool'

interface Lane {
  id: number
}

function makeHooks(
  behavior: (lane: Lane, item: string, attemptCounts: Map<string, number>) => Promise<string>,
) {
  let laneSeq = 0
  const created: Lane[] = []
  const destroyed: Lane[] = []
  const attempts = new Map<string, number>()
  return {
    created,
    destroyed,
    attempts,
    hooks: {
      createLane: () => {
        const lane = { id: laneSeq++ }
        created.push(lane)
        return lane
      },
      destroyLane: (lane: Lane) => {
        destroyed.push(lane)
      },
      run: (lane: Lane, item: string) => {
        attempts.set(item, (attempts.get(item) ?? 0) + 1)
        return behavior(lane, item, attempts)
      },
    },
  }
}

describe('runPool', () => {
  test('processes every item exactly once and destroys all lanes', async () => {
    const t = makeHooks(async (_l, item) => `ok:${item}`)
    const ok: string[] = []
    await runPool(
      ['a', 'b', 'c', 'd', 'e'],
      t.hooks,
      { onItemSuccess: (_i, r) => ok.push(r), onItemFailure: () => {} },
      { concurrency: 2 },
    )
    expect(ok.sort()).toEqual(['ok:a', 'ok:b', 'ok:c', 'ok:d', 'ok:e'])
    expect(t.created.length).toBe(2)
    expect(t.destroyed.length).toBe(t.created.length)
  })

  test('zero items: resolves without creating lanes', async () => {
    const t = makeHooks(async () => 'never')
    await runPool(
      [],
      t.hooks,
      { onItemSuccess: () => {}, onItemFailure: () => {} },
      { concurrency: 4 },
    )
    expect(t.created.length).toBe(0)
  })

  test('retries once on a FRESH lane, then succeeds', async () => {
    const t = makeHooks(async (_l, item, attempts) => {
      if (item === 'b' && attempts.get('b') === 1) throw new Error('flaky')
      return `ok:${item}`
    })
    const ok: string[] = []
    const failed: string[] = []
    await runPool(
      ['a', 'b'],
      t.hooks,
      { onItemSuccess: (_i, r) => ok.push(r), onItemFailure: (i) => failed.push(i) },
      { concurrency: 1 },
    )
    expect(ok.sort()).toEqual(['ok:a', 'ok:b'])
    expect(failed).toEqual([])
    expect(t.attempts.get('b')).toBe(2)
    // failure destroyed the lane and a fresh one was created for the retry
    expect(t.created.length).toBe(2)
    expect(t.destroyed.length).toBe(2)
  })

  test('marks item failed after maxAttempts and continues with later items', async () => {
    const t = makeHooks(async (_l, item) => {
      if (item === 'bad') throw new Error('always broken')
      return `ok:${item}`
    })
    const ok: string[] = []
    const failed: Array<[string, string]> = []
    await runPool(
      ['bad', 'good'],
      t.hooks,
      {
        onItemSuccess: (_i, r) => ok.push(r),
        onItemFailure: (i, e) => failed.push([i, e instanceof Error ? e.message : String(e)]),
      },
      { concurrency: 1, maxAttempts: 2 },
    )
    expect(failed).toEqual([['bad', 'always broken']])
    expect(ok).toEqual(['ok:good'])
    expect(t.attempts.get('bad')).toBe(2)
    expect(t.destroyed.length).toBe(t.created.length)
  })

  test('cancellation stops claiming new items', async () => {
    let cancelled = false
    const t = makeHooks(async (_l, item) => {
      if (item === 'a') cancelled = true // cancel after the first item starts
      return `ok:${item}`
    })
    const ok: string[] = []
    await runPool(
      ['a', 'b', 'c'],
      t.hooks,
      { onItemSuccess: (_i, r) => ok.push(r), onItemFailure: () => {} },
      { concurrency: 1, isCancelled: () => cancelled },
    )
    expect(ok).toEqual(['ok:a']) // b and c never claimed
    expect(t.destroyed.length).toBe(t.created.length)
  })

  test('createLane rejection propagates to the caller', async () => {
    const hooks = {
      createLane: () => {
        throw new Error('no lanes today')
      },
      destroyLane: () => {},
      run: async () => 'never',
    }
    await expect(
      runPool(
        ['a'],
        hooks,
        { onItemSuccess: () => {}, onItemFailure: () => {} },
        { concurrency: 1 },
      ),
    ).rejects.toThrow('no lanes today')
  })

  test('createLane rejection during a retry propagates (not swallowed as item failure)', async () => {
    let createCalls = 0
    const hooks = {
      createLane: () => {
        createCalls += 1
        if (createCalls > 1) throw new Error('no lanes left')
        return { id: 1 }
      },
      destroyLane: () => {},
      run: async () => {
        throw new Error('run always fails') // forces a retry → recreate
      },
    }
    const failed: string[] = []
    await expect(
      runPool(
        ['a'],
        hooks,
        { onItemSuccess: () => {}, onItemFailure: (i: string) => failed.push(i) },
        { concurrency: 1 },
      ),
    ).rejects.toThrow('no lanes left')
    expect(failed).toEqual([]) // the createLane fault must NOT masquerade as an item failure
  })

  test('non-positive concurrency clamps to one lane and still processes', async () => {
    const t = makeHooks(async (_l, item) => `ok:${item}`)
    const ok: string[] = []
    await runPool(
      ['a', 'b'],
      t.hooks,
      { onItemSuccess: (_i, r) => ok.push(r), onItemFailure: () => {} },
      { concurrency: 0 },
    )
    expect(ok.sort()).toEqual(['ok:a', 'ok:b'])
    expect(t.created.length).toBe(1)
  })
})
