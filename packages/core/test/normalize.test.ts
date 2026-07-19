import { describe, expect, test } from 'bun:test'
import {
  degreesDisplay,
  kelvinDisplay,
  scaledNumber,
  tNumberDisplay,
} from '../src/metadata/normalize'

describe('scaledNumber', () => {
  test('scales string and number inputs', () => {
    expect(scaledNumber('1728', 10)).toBe(172.8)
    expect(scaledNumber(24000, 1000)).toBe(24)
  })
  test('rejects NaN, empty, and non-finite', () => {
    expect(scaledNumber('abc', 10)).toBeUndefined()
    expect(scaledNumber('', 10)).toBeUndefined()
    expect(scaledNumber(undefined, 10)).toBeUndefined()
    expect(scaledNumber(Number.POSITIVE_INFINITY, 10)).toBeUndefined()
  })
})

describe('degreesDisplay', () => {
  test('formats with divisor', () => {
    expect(degreesDisplay('1728', 10)).toBe('172.8°')
  })
  test('formats without divisor', () => {
    expect(degreesDisplay('180')).toBe('180°')
  })
  test('undefined on junk', () => {
    expect(degreesDisplay('x')).toBeUndefined()
  })
})

describe('kelvinDisplay', () => {
  test('formats kelvin', () => {
    expect(kelvinDisplay('5600')).toBe('5600 K')
    expect(kelvinDisplay(3200)).toBe('3200 K')
  })
  test('undefined on junk', () => {
    expect(kelvinDisplay('')).toBeUndefined()
  })
})

describe('tNumberDisplay', () => {
  test('rounds to one decimal with T prefix', () => {
    expect(tNumberDisplay('1.922025')).toBe('T1.9')
    expect(tNumberDisplay(2.05)).toBe('T2.0')
  })
  test('undefined on junk', () => {
    expect(tNumberDisplay('nope')).toBeUndefined()
  })
})
