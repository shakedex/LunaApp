import { describe, expect, test } from 'bun:test'
import { appendLog, formatLogText, type LogEntry, logLevelAtLeast } from './log'

function entry(seq: number, overrides: Partial<LogEntry> = {}): LogEntry {
  return {
    seq,
    timestamp: 1_700_000_000_000 + seq,
    level: 'info',
    message: `m${seq}`,
    ...overrides,
  }
}

describe('appendLog', () => {
  test('appends in order under capacity', () => {
    const a = appendLog([], entry(1), 3)
    const b = appendLog(a, entry(2), 3)
    expect(b.map((e) => e.seq)).toEqual([1, 2])
  })

  test('drops oldest entries beyond capacity (ring semantics)', () => {
    let entries: LogEntry[] = []
    for (let i = 1; i <= 5; i++) entries = appendLog(entries, entry(i), 3)
    expect(entries.map((e) => e.seq)).toEqual([3, 4, 5])
  })

  test('returns a new array, never mutating the input', () => {
    const original = [entry(1)]
    const next = appendLog(original, entry(2), 10)
    expect(original).toHaveLength(1)
    expect(next).toHaveLength(2)
    expect(next).not.toBe(original)
  })

  test('capacity below one yields an empty log', () => {
    expect(appendLog([entry(1)], entry(2), 0)).toEqual([])
  })
})

describe('logLevelAtLeast', () => {
  test('orders debug < info < warn < error', () => {
    expect(logLevelAtLeast('debug', 'info')).toBe(false)
    expect(logLevelAtLeast('info', 'info')).toBe(true)
    expect(logLevelAtLeast('warn', 'info')).toBe(true)
    expect(logLevelAtLeast('error', 'warn')).toBe(true)
    expect(logLevelAtLeast('info', 'error')).toBe(false)
    expect(logLevelAtLeast('debug', 'debug')).toBe(true)
  })
})

describe('formatLogText', () => {
  test('formats ISO timestamp, padded level, and message per line', () => {
    const text = formatLogText([
      {
        seq: 1,
        timestamp: Date.UTC(2026, 6, 19, 12, 0, 0),
        level: 'info',
        message: 'Scan started',
      },
      { seq: 2, timestamp: Date.UTC(2026, 6, 19, 12, 0, 5), level: 'error', message: 'Boom' },
    ])
    expect(text).toBe(
      '2026-07-19T12:00:00.000Z [INFO ] Scan started\n2026-07-19T12:00:05.000Z [ERROR] Boom',
    )
  })

  test('detail lines are indented under their entry', () => {
    const text = formatLogText([
      {
        seq: 1,
        timestamp: Date.UTC(2026, 6, 19, 12, 0, 0),
        level: 'warn',
        message: 'Thumb failed',
        detail: 'NO_DECODER: aprn',
      },
    ])
    expect(text).toBe('2026-07-19T12:00:00.000Z [WARN ] Thumb failed\n    NO_DECODER: aprn')
  })

  test('empty log formats to an empty string', () => {
    expect(formatLogText([])).toBe('')
  })
})
