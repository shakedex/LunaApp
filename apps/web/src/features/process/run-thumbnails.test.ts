import { expect, test } from 'vite-plus/test'
import { isDecoderFailure, isTimeout } from './run-thumbnails'

// withTimeout formats "<label>: timed out after <n>s"; label is clip.fileName.
test('a timeout is recognized regardless of the clip name', () => {
  expect(isTimeout('A001C002.mxf: timed out after 180s')).toBe(true)
  // Filenames are attacker-free but arbitrary: a clip literally named
  // "container" or "timed out" must not change how the message classifies.
  expect(isTimeout('container_A001.mov: timed out after 180s')).toBe(true)
  expect(isTimeout('NO_DECODER')).toBe(false)
  expect(isTimeout('A001C002.mxf: unrecognized format')).toBe(false)
})

test('a decoder failure is not claimed by the timeout predicate, and vice versa', () => {
  const decoder = 'A001C002.mxf: NO_DECODER'
  expect(isDecoderFailure(decoder)).toBe(true)
  expect(isTimeout(decoder)).toBe(false)

  const timeout = 'A001C002.mxf: timed out after 180s'
  expect(isTimeout(timeout)).toBe(true)
  expect(isDecoderFailure(timeout)).toBe(false)
})

test('a clip name containing decoder keywords does not fake a decoder failure', () => {
  // The old predicate tested the whole message, so these filenames matched.
  expect(isDecoderFailure('container_A001.mov: timed out after 180s')).toBe(false)
  expect(isDecoderFailure('format_test.mxf: timed out after 180s')).toBe(false)
  expect(isDecoderFailure('recognized_B002.mxf: read error')).toBe(false)
})

// mediabunny failures reach isDecoderFailure() unprefixed (the worker throws
// 'NO_DECODER'; Comlink re-throws the message verbatim), so the cascade must
// survive a message with no "<name>: " prefix at all.
test('an unprefixed decoder failure still cascades', () => {
  expect(isDecoderFailure('NO_DECODER')).toBe(true)
  expect(isDecoderFailure('Input has an unsupported or unrecognizable format.')).toBe(true)
})

// withCancellation formats "<label>: cancelled" and must route to neither
// predicate — a superseded lane ends, it does not salvage.
test('a cancellation is neither a decoder failure nor a timeout', () => {
  expect(isDecoderFailure('A001C002.mxf: cancelled')).toBe(false)
  expect(isTimeout('A001C002.mxf: cancelled')).toBe(false)
})
