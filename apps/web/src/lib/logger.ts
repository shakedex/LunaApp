import { appendLog, type LogEntry, type LogLevel } from '@luna-web/core'
import { Store } from '@tanstack/store'

// Spec §8.14: in-memory ring buffer + console mirror. No remote sink, ever.
const CAPACITY = 500

let seq = 0

export const activityStore = new Store<LogEntry[]>([])

function write(level: LogLevel, message: string, detail?: string): void {
  seq += 1
  const entry: LogEntry = {
    seq,
    timestamp: Date.now(),
    level,
    message,
    ...(detail !== undefined ? { detail } : {}),
  }
  activityStore.setState((entries) => appendLog(entries, entry, CAPACITY))
  const line = detail !== undefined ? `${message} — ${detail}` : message
  if (level === 'error') console.error(`[luna] ${line}`)
  else if (level === 'warn') console.warn(`[luna] ${line}`)
  else console.log(`[luna] ${line}`)
}

export const logger = {
  debug: (message: string, detail?: string) => write('debug', message, detail),
  info: (message: string, detail?: string) => write('info', message, detail),
  warn: (message: string, detail?: string) => write('warn', message, detail),
  error: (message: string, detail?: string) => write('error', message, detail),
}

export function clearActivity(): void {
  activityStore.setState(() => [])
}
