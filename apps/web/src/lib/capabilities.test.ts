import { expect, test } from 'vite-plus/test'
import { type Capabilities, blockReason, isMobileDevice } from './capabilities'

const desktop = { userAgent: 'Mozilla/5.0 (Windows NT 10.0; Win64; x64)', maxTouchPoints: 0 }

const capable: Capabilities = {
  fileSystemAccess: true,
  webCodecs: true,
  wasm: true,
  mobile: false,
}

test('desktop UA strings are not mobile', () => {
  expect(isMobileDevice(desktop)).toBe(false)
  expect(
    isMobileDevice({
      userAgent: 'Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7)',
      maxTouchPoints: 0,
    }),
  ).toBe(false)
  // A touchscreen laptop reports touch points but is still a desktop.
  expect(isMobileDevice({ ...desktop, maxTouchPoints: 10 })).toBe(false)
})

test('phones and tablets are mobile', () => {
  expect(
    isMobileDevice({
      userAgent: 'Mozilla/5.0 (iPhone; CPU iPhone OS 17_0 like Mac OS X)',
      maxTouchPoints: 5,
    }),
  ).toBe(true)
  expect(
    isMobileDevice({
      userAgent: 'Mozilla/5.0 (Linux; Android 14; Pixel 8) Mobile Safari',
      maxTouchPoints: 5,
    }),
  ).toBe(true)
  // Android tablet: no "Mobi" token, and UA-CH reports mobile: false.
  expect(
    isMobileDevice({
      userAgent: 'Mozilla/5.0 (Linux; Android 14; SM-X910) Safari/537.36',
      maxTouchPoints: 5,
      uaMobileHint: false,
    }),
  ).toBe(true)
})

test('iPadOS is mobile despite its desktop UA', () => {
  expect(
    isMobileDevice({
      userAgent: 'Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7)',
      maxTouchPoints: 5,
    }),
  ).toBe(true)
})

test('the UA-CH mobile hint is honoured when the UA string says nothing', () => {
  expect(isMobileDevice({ ...desktop, uaMobileHint: true })).toBe(true)
})

test('mobile is blocked even when every API is present', () => {
  expect(blockReason({ ...capable, mobile: true })).toBe('mobile')
})

test('a desktop browser missing File System Access or WASM is blocked', () => {
  expect(blockReason({ ...capable, fileSystemAccess: false })).toBe('browser')
  expect(blockReason({ ...capable, wasm: false })).toBe('browser')
})

test('WebCodecs is optional', () => {
  expect(blockReason({ ...capable, webCodecs: false })).toBe(null)
})
