import { buildScanSummary, type DirectoryHandleLike, scanFolder } from '@luna-web/core'
import { cancelProcessing } from '@/features/process/run-processing'
import { logger } from '@/lib/logger'
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
  const phase = scanStore.state.phase
  if (phase === 'scanning' || phase === 'processing') return
  if (!(await ensureReadPermission(handle))) {
    logger.warn(`Read permission denied for "${handle.name}"`)
    scanStore.setState((s) => ({
      ...s,
      phase: 'error',
      error: 'Read permission was denied for this folder.',
    }))
    return
  }
  scanStore.setState(() => ({ ...initialScanState, phase: 'scanning', sourceName: handle.name }))
  logger.info(`Scanning "${handle.name}"…`)
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
    logger.info(
      `Scan of "${handle.name}" complete`,
      `${result.clips.length} clips, ${result.raw.length} RAW notices`,
    )
  } catch (err) {
    logger.error('Scan failed', err instanceof Error ? err.message : String(err))
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

export function resetScan(): void {
  logger.debug('Start over — scan state reset')
  cancelProcessing()
  scanStore.setState(() => initialScanState)
}
