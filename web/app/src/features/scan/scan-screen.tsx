import { useStore } from '@tanstack/react-store'
import { Button } from '@/components/ui/button'
import { startProcessing } from '@/features/process/run-processing'
import { ReportWorkspace } from '@/features/report/report-workspace'
import { formatBytes } from '@/lib/format'
import { ClipRow, RawSection } from './clip-row'
import { RecentList } from './recent-list'
import { pickAndScan, resetScan } from './run-scan'
import { scanStore } from './store'

export function ScanScreen() {
  const phase = useStore(scanStore, (s) => s.phase)
  const sourceName = useStore(scanStore, (s) => s.sourceName)
  const progress = useStore(scanStore, (s) => s.progress)
  const summary = useStore(scanStore, (s) => s.summary)
  const error = useStore(scanStore, (s) => s.error)
  const raw = useStore(scanStore, (s) => s.raw)
  const processedCount = useStore(scanStore, (s) => s.processedCount)
  const thumbDoneCount = useStore(scanStore, (s) => s.thumbDoneCount)
  const clips = useStore(scanStore, (s) => s.clips)
  const thumbTotal = useStore(scanStore, (s) => Object.keys(s.thumbStatus).length)

  return (
    <main className="mx-auto flex min-h-screen max-w-4xl flex-col items-center justify-center gap-6 px-6 py-12">
      <h1 className="font-heading text-3xl font-semibold">Luna Web</h1>

      {phase === 'idle' && (
        <>
          <p className="text-muted-foreground">
            Pick a footage folder — everything stays on this device.
          </p>
          <Button onClick={() => void pickAndScan()}>Pick folder</Button>
          <RecentList />
        </>
      )}

      {phase === 'scanning' && (
        <p className="text-muted-foreground" aria-live="polite">
          Scanning {sourceName}…{' '}
          {progress ? `${progress.filesSeen} files, ${progress.clipsFound} clips` : ''}
        </p>
      )}

      {phase === 'summary' && summary && (
        <section className="w-full rounded-lg border p-6">
          <h2 className="mb-4 text-xl font-medium">{sourceName}</h2>
          <dl className="mb-4 grid grid-cols-3 gap-4 text-center">
            <div>
              <dt className="text-muted-foreground text-sm">Clips</dt>
              <dd className="text-2xl">{summary.clipCount}</dd>
            </div>
            <div>
              <dt className="text-muted-foreground text-sm">Total size</dt>
              <dd className="text-2xl">{formatBytes(summary.totalClipSizeBytes)}</dd>
            </div>
            <div>
              <dt className="text-muted-foreground text-sm">RAW (unsupported)</dt>
              <dd className="text-2xl">{summary.rawCount}</dd>
            </div>
          </dl>
          {summary.rawCount > 0 && (
            <p className="text-muted-foreground mb-4 text-sm">
              {summary.rawCount} ARRIRAW/R3D/BRAW file(s) were detected but cannot be decoded in a
              browser — they will be listed without thumbnails.
            </p>
          )}
          <div className="flex gap-3">
            <Button onClick={() => void startProcessing()}>
              Process {summary.clipCount} clips
            </Button>
            <Button variant="outline" onClick={resetScan}>
              Cancel
            </Button>
          </div>
        </section>
      )}

      {(phase === 'processing' || phase === 'thumbnailing') && (
        <section className="w-full">
          <div className="mb-4 flex items-center justify-between">
            <h2 className="text-xl font-medium" aria-live="polite">
              {phase === 'processing'
                ? `Reading metadata… ${processedCount}/${clips.length}`
                : `Generating thumbnails… ${thumbDoneCount}/${thumbTotal}`}
            </h2>
            <Button variant="outline" onClick={resetScan}>
              Start over
            </Button>
          </div>
          <ul className="divide-y rounded-lg border">
            {clips.map((c) => (
              <ClipRow key={c.id} clipId={c.id} />
            ))}
          </ul>
          <RawSection raw={raw} />
        </section>
      )}

      {phase === 'processed' && <ReportWorkspace />}

      {phase === 'error' && (
        <section className="text-center">
          <p className="text-destructive mb-4">{error}</p>
          <Button variant="outline" onClick={resetScan}>
            Back
          </Button>
        </section>
      )}
    </main>
  )
}
