import { expect, test } from 'bun:test'
import { compareReelNames } from './detect'

test('orders reel names numerically, not lexically', () => {
  expect(['A010', 'A002', 'A001'].sort(compareReelNames)).toEqual(['A001', 'A002', 'A010'])
})
