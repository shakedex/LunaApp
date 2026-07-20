import {
  CAMERA_FIELDS,
  type CameraFieldKey,
  type ClipMetadata,
  fileExtensionOf,
  joinPath,
} from '@luna-web/core'
import { formatBytes, formatDuration } from '@/lib/format'
import type { PdfClip, PdfReel } from './pdf-prepare'

/** One ` · `-separated segment of a clip fact line (spec §4.5). */
export interface Fact {
  label?: string
  value: string
  mono?: boolean
}

/** Zero-width space after each '/' so paths wrap at segment boundaries (§3). */
export function breakablePath(path: string): string {
  return path.replaceAll('/', '/​')
}

/**
 * Spec §4.4: a reel grouped by its top-level folder shows root/folder; a reel
 * grouped by embedded reelName (clips scattered across folders) shows root only.
 */
export function reelPath(root: string, reel: PdfReel): string {
  const isFolderReel =
    reel.clips.length > 0 && reel.clips.every((c) => c.relativePath.startsWith(`${reel.name}/`))
  return isFolderReel ? joinPath(root, reel.name) : root
}

/** `MOV · 1443 frames (57.7s) · 7.09 GB` — size always exists (§4.5 line 1). */
export function fileFacts(clip: PdfClip): Fact[] {
  const facts: Fact[] = []
  const ext = fileExtensionOf(clip.relativePath).slice(1).toUpperCase()
  if (ext) facts.push({ value: ext })
  const { durationSeconds, frameRate } = clip.metadata
  if (durationSeconds !== undefined && frameRate !== undefined) {
    facts.push({
      value: `${Math.round(durationSeconds * frameRate)} frames (${formatDuration(durationSeconds)})`,
      mono: true,
    })
  } else if (durationSeconds !== undefined) {
    facts.push({ value: formatDuration(durationSeconds), mono: true })
  }
  facts.push({ value: formatBytes(clip.sizeBytes), mono: true })
  return facts
}

/** `3840×2160 (1.78:1) · Apple ProRes 4444 · 25 fps` (§4.5 line 2). */
export function videoFacts(metadata: ClipMetadata): Fact[] {
  const facts: Fact[] = []
  if (metadata.width && metadata.height) {
    const aspect = `${(metadata.width / metadata.height).toFixed(2).replace(/\.?0+$/, '')}:1`
    facts.push({ value: `${metadata.width}×${metadata.height} (${aspect})`, mono: true })
  }
  if (metadata.codec) facts.push({ value: metadata.codec })
  if (metadata.frameRate !== undefined) {
    facts.push({ value: `${Number(metadata.frameRate.toFixed(3))} fps`, mono: true })
  }
  return facts
}

// PDF-only presentation: short labels where a bare value is ambiguous, mono
// for numeric-ish values. Field set and order come from CAMERA_FIELDS.
const PDF_CAMERA_PRESENTATION: Record<CameraFieldKey, { label?: string; mono?: boolean }> = {
  camera: {},
  iso: { label: 'ISO', mono: true },
  whiteBalance: { label: 'WB', mono: true },
  lens: {},
  focalLength: { mono: true },
  aperture: { mono: true },
  shutter: { label: 'Shutter', mono: true },
  gamma: {},
}

/** Camera extras, only what exists (§4.5 line 4). */
export function cameraFacts(metadata: ClipMetadata): Fact[] {
  const facts: Fact[] = []
  for (const f of CAMERA_FIELDS) {
    const value = metadata[f.key]
    if (!value) continue
    const p = PDF_CAMERA_PRESENTATION[f.key]
    facts.push({
      ...(p.label !== undefined && { label: p.label }),
      value,
      ...(p.mono && { mono: true }),
    })
  }
  if (metadata.colorSpace) facts.push({ value: metadata.colorSpace })
  return facts
}
