import { useSelector } from '@tanstack/react-store'
import { CircleAlert, TriangleAlert } from 'lucide-react'
import { StatTile } from '@/components/stat-tile'
import { Alert, AlertDescription, AlertTitle } from '@/components/ui/alert'
import { Button } from '@/components/ui/button'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import {
  Empty,
  EmptyContent,
  EmptyDescription,
  EmptyHeader,
  EmptyMedia,
  EmptyTitle,
} from '@/components/ui/empty'
import { Spinner } from '@/components/ui/spinner'
import { startProcessing } from '@/features/process/run-processing'
import { ReportWorkspace } from '@/features/report/report-workspace'
import { formatBytes } from '@/lib/format'
import { Dropzone } from './dropzone'
import { ProcessingView } from './processing-view'
import { resetScan } from './run-scan'
import { scanStore } from './store'

export function ScanScreen() {
  const phase = useSelector(scanStore, (s) => s.phase)
  const sourceName = useSelector(scanStore, (s) => s.sourceName)
  const progress = useSelector(scanStore, (s) => s.progress)
  const summary = useSelector(scanStore, (s) => s.summary)
  const error = useSelector(scanStore, (s) => s.error)

  return (
    <div
      key={phase}
      className="flex animate-in fade-in slide-in-from-bottom-2 flex-col gap-6 duration-200"
    >
      {phase === 'idle' && <Dropzone />}

      {phase === 'scanning' && (
        <div className="flex flex-col items-center gap-4 py-24 text-center" aria-live="polite">
          <Spinner className="size-6" />
          <div className="space-y-1">
            <p className="font-medium">Scanning {sourceName}</p>
            <p className="text-muted-foreground font-mono text-sm tabular-nums">
              {progress
                ? `${progress.filesSeen} files · ${progress.clipsFound} clips`
                : 'Reading folder…'}
            </p>
          </div>
        </div>
      )}

      {phase === 'summary' && summary && (
        <Card className="w-full">
          <CardHeader>
            <CardTitle className="truncate">{sourceName}</CardTitle>
          </CardHeader>
          <CardContent className="space-y-6">
            <div className="grid grid-cols-3 gap-4">
              <StatTile label="Clips" value={String(summary.clipCount)} />
              <StatTile label="Total size" value={formatBytes(summary.totalClipSizeBytes)} />
              <StatTile label="RAW (unsupported)" value={String(summary.rawCount)} />
            </div>
            {summary.rawCount > 0 && (
              <Alert className="border-amber-500/30 [&>svg]:text-amber-400">
                <TriangleAlert />
                <AlertTitle>
                  {summary.rawCount} ARRIRAW frame file(s) detected — not listed as clips
                </AlertTitle>
                <AlertDescription>
                  .ari stills can't be decoded to a thumbnail in the browser, so they're excluded
                  from the report. BRAW, R3D, and CRM clips are listed with full metadata —
                  thumbnails come from embedded posters or .rtn sidecars where available.
                </AlertDescription>
              </Alert>
            )}
            <div className="flex gap-3">
              <Button onClick={() => void startProcessing()}>
                Process {summary.clipCount} clips
              </Button>
              <Button variant="outline" onClick={resetScan}>
                Cancel
              </Button>
            </div>
          </CardContent>
        </Card>
      )}

      {(phase === 'processing' || phase === 'thumbnailing') && <ProcessingView />}

      {phase === 'processed' && <ReportWorkspace />}

      {phase === 'error' && (
        <Empty>
          <EmptyHeader>
            <EmptyMedia variant="icon">
              <CircleAlert className="text-destructive" />
            </EmptyMedia>
            <EmptyTitle>Something went wrong</EmptyTitle>
            <EmptyDescription>{error}</EmptyDescription>
          </EmptyHeader>
          <EmptyContent>
            <Button variant="outline" onClick={resetScan}>
              Back
            </Button>
          </EmptyContent>
        </Empty>
      )}
    </div>
  )
}
