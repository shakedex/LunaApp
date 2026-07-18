// Browser file handles expose no hidden/system attributes; skip by name instead
// (parity with the desktop scanner's Hidden/System filter).
const JUNK_DIRECTORY_NAMES = new Set([
  'system volume information',
  '$recycle.bin',
  '__macosx',
  'lost+found',
])

export function isJunkName(name: string): boolean {
  return name.startsWith('.') || JUNK_DIRECTORY_NAMES.has(name.toLowerCase())
}
