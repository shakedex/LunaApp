import { expect, test } from 'vite-plus/test'
import { formatBytes, formatExactBytes, todayIso } from './format'

test('todayIso is a YYYY-MM-DD date', () => {
  expect(todayIso()).toMatch(/^\d{4}-\d{2}-\d{2}$/)
})

test('formatBytes uses decimal units matching the labels', () => {
  expect(formatBytes(999)).toBe('999 B')
  expect(formatBytes(1000)).toBe('1.0 KB')
  expect(formatBytes(1_500_000)).toBe('1.5 MB')
  expect(formatBytes(37_200_000_000)).toBe('37.2 GB')
})

test('todayIso matches the local calendar date', () => {
  const now = new Date()
  const expected = `${now.getFullYear()}-${String(now.getMonth() + 1).padStart(2, '0')}-${String(now.getDate()).padStart(2, '0')}`
  expect(todayIso()).toBe(expected)
})

test('formatExactBytes groups digits and is the figure every OS agrees on', () => {
  expect(formatExactBytes(0)).toBe('0 bytes')
  expect(formatExactBytes(1)).toBe('1 byte')
  expect(formatExactBytes(999)).toBe('999 bytes')
  expect(formatExactBytes(2_600_000_000)).toBe('2,600,000,000 bytes')
})
