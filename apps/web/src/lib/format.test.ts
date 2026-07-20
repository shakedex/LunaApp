import { expect, test } from 'bun:test'
import { todayIso } from './format'

test('todayIso is a YYYY-MM-DD date', () => {
  expect(todayIso()).toMatch(/^\d{4}-\d{2}-\d{2}$/)
})
