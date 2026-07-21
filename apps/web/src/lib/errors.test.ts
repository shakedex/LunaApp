import { expect, test } from 'vite-plus/test'
import { errorMessage } from './errors'

test('unwraps an Error message', () => {
  expect(errorMessage(new Error('boom'))).toBe('boom')
})

test('stringifies non-Error throwables', () => {
  expect(errorMessage('raw string')).toBe('raw string')
  expect(errorMessage(42)).toBe('42')
  expect(errorMessage(undefined)).toBe('undefined')
})
