import { expect, test } from 'vite-plus/test'
import { joinPath } from './paths'

test('joins root and relative with a slash', () => {
  expect(joinPath('CARD_A', 'A001/clip.mov')).toBe('CARD_A/A001/clip.mov')
})

test('passes the relative path through when root is unknown', () => {
  expect(joinPath('', 'clip.mov')).toBe('clip.mov')
})
