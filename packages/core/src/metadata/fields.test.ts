import { expect, test } from 'bun:test'
import { CAMERA_FIELDS } from './fields'

test('camera fields are unique and in canonical display order', () => {
  const keys = CAMERA_FIELDS.map((f) => f.key)
  expect(new Set(keys).size).toBe(keys.length)
  expect(keys).toEqual([
    'camera',
    'iso',
    'whiteBalance',
    'lens',
    'focalLength',
    'aperture',
    'shutter',
    'gamma',
  ])
})
