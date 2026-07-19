export type LogLevel = 'debug' | 'info' | 'warn' | 'error'

// Ascending severity — index order is the comparison order.
export const LOG_LEVELS: readonly LogLevel[] = ['debug', 'info', 'warn', 'error']

export interface LogEntry {
  seq: number
  timestamp: number // epoch ms, caller-supplied (core stays clock-free)
  level: LogLevel
  message: string
  detail?: string
}

// Capped append over an immutable array — the app holds entries in a TanStack
// store, so ring semantics are "new array, oldest dropped", not index juggling.
export function appendLog(
  entries: readonly LogEntry[],
  entry: LogEntry,
  capacity: number,
): LogEntry[] {
  if (capacity < 1) return []
  const next = [...entries, entry]
  return next.length > capacity ? next.slice(next.length - capacity) : next
}

export function logLevelAtLeast(level: LogLevel, min: LogLevel): boolean {
  return LOG_LEVELS.indexOf(level) >= LOG_LEVELS.indexOf(min)
}

export function formatLogText(entries: readonly LogEntry[]): string {
  return entries
    .map((e) => {
      const head = `${new Date(e.timestamp).toISOString()} [${e.level.toUpperCase().padEnd(5)}] ${e.message}`
      return e.detail !== undefined ? `${head}\n    ${e.detail}` : head
    })
    .join('\n')
}
