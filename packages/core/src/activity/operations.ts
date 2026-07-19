import { LOG_LEVELS, type LogEntry, type LogLevel } from './log'

export type OperationKind = 'app' | 'scan' | 'process' | 'export'

const OPERATION_KINDS: readonly OperationKind[] = ['app', 'scan', 'process', 'export']

export interface Operation {
  id: number
  kind: OperationKind
  label: string
  startedAt: number // epoch ms, caller-supplied
}

export interface OperationGroup {
  operation: Operation
  entries: LogEntry[]
}

// Bucket for entries written outside any user operation (or whose operation
// was evicted by the cap): always rendered last.
export const GENERAL_OPERATION: Operation = { id: 0, kind: 'app', label: 'General', startedAt: 0 }

export function groupLogByOperation(
  entries: readonly LogEntry[],
  operations: readonly Operation[],
): OperationGroup[] {
  const known = new Map<number, LogEntry[]>()
  for (const operation of operations) known.set(operation.id, [])
  const general: LogEntry[] = []
  for (const entry of entries) {
    const bucket = entry.operationId !== undefined ? known.get(entry.operationId) : undefined
    if (bucket) bucket.push(entry)
    else general.push(entry)
  }
  const groups: OperationGroup[] = []
  const byNewest = [...operations].sort((a, b) => b.startedAt - a.startedAt || b.id - a.id)
  for (const operation of byNewest) {
    const bucket = known.get(operation.id)
    if (bucket && bucket.length > 0) groups.push({ operation, entries: bucket })
  }
  if (general.length > 0) groups.push({ operation: GENERAL_OPERATION, entries: general })
  return groups
}

export interface ActivitySnapshot {
  operations: Operation[]
  entries: LogEntry[]
}

// Defensive read of a persisted snapshot: any past or future app version may
// have written it. Structurally invalid members are dropped, never repaired.
export function normalizeActivitySnapshot(raw: unknown): ActivitySnapshot {
  if (typeof raw !== 'object' || raw === null) return { operations: [], entries: [] }
  const record = raw as { operations?: unknown; entries?: unknown }
  const operations = Array.isArray(record.operations)
    ? record.operations.filter(isValidOperation)
    : []
  const entries = Array.isArray(record.entries) ? record.entries.filter(isValidEntry) : []
  return { operations, entries }
}

function isValidOperation(value: unknown): value is Operation {
  if (typeof value !== 'object' || value === null) return false
  const op = value as Record<string, unknown>
  return (
    typeof op.id === 'number' &&
    Number.isFinite(op.id) &&
    OPERATION_KINDS.includes(op.kind as OperationKind) &&
    typeof op.label === 'string' &&
    typeof op.startedAt === 'number' &&
    Number.isFinite(op.startedAt)
  )
}

function isValidEntry(value: unknown): value is LogEntry {
  if (typeof value !== 'object' || value === null) return false
  const entry = value as Record<string, unknown>
  const operationIdOk =
    entry.operationId === undefined ||
    (typeof entry.operationId === 'number' && Number.isFinite(entry.operationId))
  return (
    typeof entry.seq === 'number' &&
    Number.isFinite(entry.seq) &&
    typeof entry.timestamp === 'number' &&
    Number.isFinite(entry.timestamp) &&
    LOG_LEVELS.includes(entry.level as LogLevel) &&
    typeof entry.message === 'string' &&
    (entry.detail === undefined || typeof entry.detail === 'string') &&
    operationIdOk
  )
}

const keepNewest = <T>(items: readonly T[], max: number): T[] => (max <= 0 ? [] : items.slice(-max))

export function capActivitySnapshot(
  snapshot: ActivitySnapshot,
  maxOperations: number,
  maxEntries: number,
): ActivitySnapshot {
  const operations = keepNewest(
    [...snapshot.operations].sort((a, b) => a.startedAt - b.startedAt || a.id - b.id),
    maxOperations,
  )
  const kept = new Set(operations.map((o) => o.id))
  const entries = keepNewest(snapshot.entries, maxEntries).filter(
    (e) => e.operationId === undefined || e.operationId === 0 || kept.has(e.operationId),
  )
  return { operations, entries }
}
