import { buildScanSummary, type DirectoryHandleLike, scanFolder } from '@luna-web/core'
import { cancelProcessing } from '@/features/process/run-processing'
import { beginOperation, logger } from '@/lib/logger'
import { markSourceStale, rememberSource } from '@/persistence/recent-sources'
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

export async function scanFrom(
  handle: FileSystemDirectoryHandle,
  recentKey?: number,
): Promise<void> {
  const phase = scanStore.state.phase
  if (phase === 'scanning' || phase === 'processing') return
  if (!(await ensureReadPermission(handle))) {
    logger.warn(`Read permission denied for "${handle.name}"`)
    scanStore.setState((s) => ({
      ...s,
      phase: 'error',
      error: `Luna needs read permission for "${handle.name}". Pick the folder again to re-authorize.`,
    }))
    return
  }
  beginOperation('scan', `Scan: ${handle.name}`)
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
      otherFiles: result.otherFiles,
      progress: {
        filesSeen: result.clips.length + result.otherFiles.length,
        clipsFound: result.clips.length,
      },
      summary: buildScanSummary(result.clips, result.otherFiles),
    }))
    logger.info(
      `Scan of "${handle.name}" complete`,
      `${result.clips.length} clips, ${result.otherFiles.length} other files`,
    )
  } catch (err) {
    // A recent-source handle whose folder was moved/renamed/removed throws
    // NotFoundError (possibly mid-walk). Spec §15: mark the entry stale and
    // say what happened — never surface a raw DOMException message.
    if (err instanceof DOMException && err.name === 'NotFoundError') {
      if (recentKey !== undefined) markSourceStale(recentKey).catch(() => {})
      logger.warn(`Recent folder "${handle.name}" is no longer accessible`)
      scanStore.setState((s) => ({
        ...s,
        phase: 'error',
        error: `Luna can't find "${handle.name}" anymore — it may have been moved, renamed, or unplugged. Pick the folder again to continue.`,
      }))
      return
    }
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
