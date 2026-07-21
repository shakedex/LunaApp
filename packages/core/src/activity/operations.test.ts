import { describe, expect, test } from 'vite-plus/test'
import type { LogEntry } from './log'
import {
  capActivitySnapshot,
  GENERAL_OPERATION,
  groupLogByOperation,
  normalizeActivitySnapshot,
  type Operation,
} from './operations'

function op(id: number, startedAt = id * 1000): Operation {
  return { id, kind: 'scan', label: `Op ${id}`, startedAt }
}

function entry(seq: number, operationId?: number): LogEntry {
  return {
    seq,
    timestamp: 1_700_000_000_000 + seq,
    level: 'info',
    message: `m${seq}`,
    ...(operationId !== undefined ? { operationId } : {}),
  }
}

describe('groupLogByOperation', () => {
  test('groups by operation, newest operation first, entry order preserved', () => {
    const groups = groupLogByOperation(
      [entry(1, 1), entry(2, 2), entry(3, 1), entry(4, 2)],
      [op(1), op(2)],
    )
    expect(groups.map((g) => g.operation.id)).toEqual([2, 1])
    expect(groups[0]?.entries.map((e) => e.seq)).toEqual([2, 4])
    expect(groups[1]?.entries.map((e) => e.seq)).toEqual([1, 3])
  })

  test('unmatched and untagged entries fall into the trailing General group', () => {
    const groups = groupLogByOperation([entry(1), entry(2, 99), entry(3, 1)], [op(1)])
    expect(groups.map((g) => g.operation.id)).toEqual([1, GENERAL_OPERATION.id])
    expect(groups[1]?.entries.map((e) => e.seq)).toEqual([1, 2])
  })

  test('operations with zero entries are omitted', () => {
    const groups = groupLogByOperation([entry(1, 1)], [op(1), op(2)])
    expect(groups.map((g) => g.operation.id)).toEqual([1])
  })

  test('empty input yields no groups', () => {
    expect(groupLogByOperation([], [])).toEqual([])
  })
})

describe('normalizeActivitySnapshot', () => {
  test('non-object input yields an empty snapshot', () => {
    expect(normalizeActivitySnapshot(undefined)).toEqual({ operations: [], entries: [] })
    expect(normalizeActivitySnapshot('junk')).toEqual({ operations: [], entries: [] })
  })

  test('structurally invalid members are filtered, valid ones survive', () => {
    const snapshot = normalizeActivitySnapshot({
      operations: [op(1), { id: 'x' }, null, { id: 2, kind: 'bogus', label: 'l', startedAt: 1 }],
      entries: [entry(1, 1), { seq: 2 }, { ...entry(3), level: 'loud' }],
    })
    expect(snapshot.operations).toEqual([op(1)])
    expect(snapshot.entries).toEqual([entry(1, 1)])
  })
})

describe('capActivitySnapshot', () => {
  test('keeps newest operations and entries, drops entries of evicted operations', () => {
    const snapshot = {
      operations: [op(1), op(2), op(3)],
      entries: [entry(1, 1), entry(2, 2), entry(3, 3), entry(4), entry(5, 3)],
    }
    const capped = capActivitySnapshot(snapshot, 2, 10)
    expect(capped.operations.map((o) => o.id)).toEqual([2, 3])
    expect(capped.entries.map((e) => e.seq)).toEqual([2, 3, 4, 5])
  })

  test('entry cap keeps the newest entries', () => {
    const snapshot = { operations: [op(1)], entries: [entry(1, 1), entry(2, 1), entry(3, 1)] }
    expect(capActivitySnapshot(snapshot, 5, 2).entries.map((e) => e.seq)).toEqual([2, 3])
  })

  test('caps of zero (or negative) keep nothing', () => {
    const snapshot = { operations: [op(1)], entries: [entry(1, 1)] }
    expect(capActivitySnapshot(snapshot, 0, 0)).toEqual({ operations: [], entries: [] })
    expect(capActivitySnapshot(snapshot, -1, -1)).toEqual({ operations: [], entries: [] })
  })

  test('explicit operationId 0 entries survive operation eviction', () => {
    const snapshot = { operations: [op(1), op(2)], entries: [entry(1, 0), entry(2, 1)] }
    const capped = capActivitySnapshot(snapshot, 1, 10)
    expect(capped.operations.map((o) => o.id)).toEqual([2])
    expect(capped.entries.map((e) => e.seq)).toEqual([1])
  })
})
