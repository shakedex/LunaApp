export interface Capabilities {
  fileSystemAccess: boolean
  webCodecs: boolean
  wasm: boolean
}

export function detectCapabilities(): Capabilities {
  return {
    fileSystemAccess: typeof window !== 'undefined' && 'showDirectoryPicker' in window,
    webCodecs: typeof window !== 'undefined' && 'VideoDecoder' in window,
    wasm: typeof WebAssembly !== 'undefined',
  }
}

// File System Access + WASM are required. WebCodecs is optional: when absent,
// every clip decodes through the ffmpeg.wasm path (spec §8.1, §10.2).
export function isBrowserSupported(c: Capabilities): boolean {
  return c.fileSystemAccess && c.wasm
}
