import { buildScanSummary, type DirectoryHandleLike, scanFolder } from '@luna-web/core'
import { rememberSource } from '@/persistence/recent-sources'
import { ensureReadPermission } from './permissions'
import { initialScanState, scanStore } from './store'

const PROGRESS_EVERY = 25 // throttle store updates on huge cards

export async function pickAndScan(): Promise<void> {
  let handle: FileSystemDirectoryHandle
  try {
    handle = await window.showDirectoryPicker()
  } catch {
    return // user cancelled the picker
  }
  await scanFrom(handle)
}

export async function scanFrom(handle: FileSystemDirectoryHandle): Promise<void> {
  if (!(await ensureReadPermission(handle))) {
    scanStore.setState((s) => ({
      ...s,
      phase: 'error',
      error: 'Read permission was denied for this folder.',
    }))
    return
  }
  scanStore.setState(() => ({ ...initialScanState, phase: 'scanning', sourceName: handle.name }))
  try {
    // Boundary cast: the real FileSystemDirectoryHandle satisfies DirectoryHandleLike
    // at runtime; TS's lib types yield base FileSystemHandle from entries().
    const result = await scanFolder(handle as unknown as DirectoryHandleLike, (p) => {
      if (p.filesSeen === 1 || p.filesSeen % PROGRESS_EVERY === 0) {
        scanStore.setState((s) => ({
          ...s,
          progress: { filesSeen: p.filesSeen, clipsFound: p.clipsFound },
        }))
      }
    })
    scanStore.setState((s) => ({
      ...s,
      phase: 'summary',
      clips: result.clips,
      raw: result.raw,
      progress: {
        filesSeen: result.clips.length + result.raw.length,
        clipsFound: result.clips.length,
      },
      summary: buildScanSummary(result.clips, result.raw),
    }))
  } catch (err) {
    scanStore.setState((s) => ({
      ...s,
      phase: 'error',
      error: err instanceof Error ? err.message : String(err),
    }))
    return
  }
  rememberSource(handle, Date.now()).catch(() => {
    // best-effort: losing a recent-sources entry must never sink a completed scan
  })
}

export function confirmScan(): void {
  scanStore.setState((s) => (s.phase === 'summary' ? { ...s, phase: 'confirmed' } : s))
}

export function resetScan(): void {
  scanStore.setState(() => initialScanState)
}
