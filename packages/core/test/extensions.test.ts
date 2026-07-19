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

  // Reclassification (FINDINGS.md, Plan 08): .r3d/.braw/.crm now get full
  // metadata + a content-aware thumbnail route instead of a raw-notice dead
  // end, so they flow through the app as clips like any other supported format.
  test('accepts reclassified RAW formats as clips', () => {
    expect(isSupportedMediaExtension('shot.r3d')).toBe(true)
    expect(isSupportedMediaExtension('shot.braw')).toBe(true)
    expect(isSupportedMediaExtension('shot.crm')).toBe(true)
    expect(isSupportedMediaExtension('shot.R3D')).toBe(true) // case-insensitive
  })

  test('rejects unsupported RAW and non-media files', () => {
    // .ari (single-frame ARRIRAW stills) is the only extension still excluded
    // — no embedded preview, no metadata worth surfacing as a clip.
    expect(isSupportedMediaExtension('shot.ari')).toBe(false)
    expect(isSupportedMediaExtension('notes.txt')).toBe(false)
    expect(isSupportedMediaExtension('noext')).toBe(false)
  })
})

describe('isKnownRawExtension', () => {
  test('only .ari remains a raw notice after reclassification', () => {
    expect(isKnownRawExtension('shot.ari')).toBe(true)
    expect(isKnownRawExtension('clip.mov')).toBe(false)
  })

  // Deliberate change: .r3d/.braw moved OUT of UNSUPPORTED_RAW_EXTENSIONS into
  // SUPPORTED_MEDIA_EXTENSIONS above, so they no longer flag as "known but
  // undecodable" raw — they are clips now (thumbnailRouteFor gives them a
  // 'preview' or 'none' route instead of being excluded from scanning).
  test('reclassified RAW extensions no longer flag as raw notices', () => {
    expect(isKnownRawExtension('shot.R3D')).toBe(false)
    expect(isKnownRawExtension('shot.braw')).toBe(false)
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
