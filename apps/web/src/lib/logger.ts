import {
  type ActivitySnapshot,
  appendLog,
  capActivitySnapshot,
  type LogEntry,
  type LogLevel,
  type Operation,
  type OperationKind,
} from '@luna-web/core'
import { Store } from '@tanstack/store'
import { clearPersistedActivity, loadActivity, saveActivity } from '@/persistence/activity'

// Spec §8.14 + Plan 10: operation-grouped ring, persisted to idb so a refresh
// keeps past operations readable. Caps keep the store and the record bounded.
const ENTRY_CAPACITY = 1000
const OPERATION_CAPACITY = 25
const PERSIST_DEBOUNCE_MS = 400

let seq = 0
let operationSeq = 0
let currentOperationId: number | undefined
let persistTimer: ReturnType<typeof setTimeout> | null = null

export const activityStore = new Store<ActivitySnapshot>({ operations: [], entries: [] })

// Restore the persisted snapshot BEFORE the first render (main.tsx) so new
// entries append after history and counters never collide with restored ids.
export async function hydrateActivity(): Promise<void> {
  const restored = await loadActivity()
  activityStore.setState(() => restored)
  seq = restored.entries.reduce((max, e) => Math.max(max, e.seq), 0)
  operationSeq = restored.operations.reduce((max, o) => Math.max(max, o.id), 0)
}

function schedulePersist(): void {
  if (persistTimer !== null) clearTimeout(persistTimer)
  persistTimer = setTimeout(() => {
    persistTimer = null
    saveActivity(activityStore.state).catch(() => {
      // Best-effort: a failed idb write must never affect the app.
    })
  }, PERSIST_DEBOUNCE_MS)
}

export function beginOperation(kind: OperationKind, label: string): void {
  operationSeq += 1
  currentOperationId = operationSeq
  const operation: Operation = { id: operationSeq, kind, label, startedAt: Date.now() }
  activityStore.setState((s) =>
    capActivitySnapshot(
      { operations: [...s.operations, operation], entries: s.entries },
      OPERATION_CAPACITY,
      ENTRY_CAPACITY,
    ),
  )
  schedulePersist()
}

function write(level: LogLevel, message: string, detail?: string): void {
  seq += 1
  const entry: LogEntry = {
    seq,
    timestamp: Date.now(),
    level,
    message,
    ...(detail !== undefined ? { detail } : {}),
    ...(currentOperationId !== undefined ? { operationId: currentOperationId } : {}),
  }
  activityStore.setState((s) =>
    capActivitySnapshot(
      { operations: s.operations, entries: appendLog(s.entries, entry, ENTRY_CAPACITY) },
      OPERATION_CAPACITY,
      ENTRY_CAPACITY,
    ),
  )
  const line = detail !== undefined ? `${message} — ${detail}` : message
  if (level === 'error') console.error(`[luna] ${line}`)
  else if (level === 'warn') console.warn(`[luna] ${line}`)
  else console.log(`[luna] ${line}`)
  schedulePersist()
}

export const logger = {
  debug: (message: string, detail?: string) => write('debug', message, detail),
  info: (message: string, detail?: string) => write('info', message, detail),
  warn: (message: string, detail?: string) => write('warn', message, detail),
  error: (message: string, detail?: string) => write('error', message, detail),
}

export function clearActivity(): void {
  currentOperationId = undefined
  activityStore.setState(() => ({ operations: [], entries: [] }))
  clearPersistedActivity().catch(() => {})
}
