import { describe, expect, test } from 'bun:test'
import { fileExtensionOf, isSupportedMediaExtension } from '../src/media/extensions'

describe('isSupportedMediaExtension', () => {
  test('accepts generic camera formats regardless of case', () => {
    expect(isSupportedMediaExtension('A001C001.mov')).toBe(true)
    expect(isSupportedMediaExtension('clip.MP4')).toBe(true)
    expect(isSupportedMediaExtension('roll.mxf')).toBe(true)
  })

  // Every camera-original format is a first-class clip ("a file is a file",
  // 2026-07-19 backlog): .r3d/.braw/.crm/.ari all flow through the app as
  // clips — metadata where extractable, honest placeholder thumbnails where not.
  test('accepts RAW camera formats as clips', () => {
    expect(isSupportedMediaExtension('shot.r3d')).toBe(true)
    expect(isSupportedMediaExtension('shot.braw')).toBe(true)
    expect(isSupportedMediaExtension('shot.crm')).toBe(true)
    expect(isSupportedMediaExtension('shot.ari')).toBe(true)
    expect(isSupportedMediaExtension('shot.R3D')).toBe(true) // case-insensitive
  })

  test('rejects non-media files', () => {
    expect(isSupportedMediaExtension('notes.txt')).toBe(false)
    expect(isSupportedMediaExtension('noext')).toBe(false)
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
