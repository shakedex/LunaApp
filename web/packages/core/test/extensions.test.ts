import { describe, expect, test } from 'bun:test'
import {
  fileExtensionOf,
  isKnownRawExtension,
  isSupportedMediaExtension,
} from '../src/media/extensions'

describe('isSupportedMediaExtension', () => {
  test('accepts generic camera formats regardless of case', () => {
    expect(isSupportedMediaExtension('A001C001.mov')).toBe(true)
    expect(isSupportedMediaExtension('clip.MP4')).toBe(true)
    expect(isSupportedMediaExtension('roll.mxf')).toBe(true)
  })

  test('rejects unsupported RAW and non-media files', () => {
    expect(isSupportedMediaExtension('shot.r3d')).toBe(false)
    expect(isSupportedMediaExtension('shot.braw')).toBe(false)
    expect(isSupportedMediaExtension('shot.ari')).toBe(false)
    expect(isSupportedMediaExtension('notes.txt')).toBe(false)
    expect(isSupportedMediaExtension('noext')).toBe(false)
  })
})

describe('isKnownRawExtension', () => {
  test('flags vendor RAW that is detected but not decodable', () => {
    expect(isKnownRawExtension('shot.R3D')).toBe(true)
    expect(isKnownRawExtension('shot.braw')).toBe(true)
    expect(isKnownRawExtension('clip.mov')).toBe(false)
  })
})

describe('fileExtensionOf', () => {
  test('lowercases and keeps the dot', () => {
    expect(fileExtensionOf('CLIP.MOV')).toBe('.mov')
    expect(fileExtensionOf('a.tar.gz')).toBe('.gz')
  })
  test('empty for no extension and dotfiles', () => {
    expect(fileExtensionOf('noext')).toBe('')
    expect(fileExtensionOf('.DS_Store')).toBe('')
  })
})
