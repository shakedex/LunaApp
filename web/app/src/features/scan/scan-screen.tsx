import { useStore } from '@tanstack/react-store'
import { CircleAlert, TriangleAlert } from 'lucide-react'
import { Logo } from '@/components/logo'
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
import { Progress } from '@/components/ui/progress'
import { Spinner } from '@/components/ui/spinner'
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
    <div
      key={phase}
      className="flex animate-in fade-in slide-in-from-bottom-2 flex-col gap-6 duration-200"
    >
      {phase === 'idle' && (
        <div className="flex flex-col items-center gap-6 py-16 text-center">
          <Logo className="h-16 w-auto" />
          <div className="space-y-2">
            <h1 className="text-2xl font-semibold tracking-tight">
              Camera reports, in your browser
            </h1>
            <p className="text-muted-foreground max-w-md text-sm leading-relaxed">
              Pick a footage folder to get per-clip thumbnails, metadata, and a PDF/CSV report.
              Everything stays on this device.
            </p>
          </div>
          <Button
            size="lg"
            onClick={() => void pickAndScan()}
            style={{ boxShadow: '0 0 24px oklch(0.72 0.14 245 / 0.25)' }}
          >
            Pick folder
          </Button>
          <p className="text-muted-foreground font-mono text-xs">
            MOV · MP4 · MXF · MKV · AVI · MTS · and more
          </p>
          <RecentList />
        </div>
      )}

      {phase === 'scanning' && (
        <div className="flex items-center gap-3 py-16" aria-live="polite">
          <Spinner className="size-5" />
          <p className="text-muted-foreground">
            Scanning {sourceName}…{' '}
            <span className="font-mono tabular-nums">
              {progress ? `${progress.filesSeen} files, ${progress.clipsFound} clips` : ''}
            </span>
          </p>
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
                  {summary.rawCount} RAW file(s) can't be decoded in a browser
                </AlertTitle>
                <AlertDescription>
                  ARRIRAW / R3D / BRAW files were detected. They'll be listed in the report without
                  thumbnails.
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

      {(phase === 'processing' || phase === 'thumbnailing') && (
        <section className="w-full space-y-4">
          <div className="space-y-3">
            <div className="flex items-center justify-between">
              <h2 className="text-lg font-medium" aria-live="polite">
                {phase === 'processing'
                  ? `Reading metadata… ${processedCount}/${clips.length}`
                  : `Generating thumbnails… ${thumbDoneCount}/${thumbTotal}`}
              </h2>
              <Button variant="outline" size="sm" onClick={resetScan}>
                Start over
              </Button>
            </div>
            <Progress
              value={
                phase === 'processing'
                  ? clips.length > 0
                    ? (processedCount / clips.length) * 100
                    : 0
                  : thumbTotal > 0
                    ? (thumbDoneCount / thumbTotal) * 100
                    : 0
              }
            />
          </div>
          <Card className="overflow-hidden py-0">
            <ul className="divide-y">
              {clips.map((c) => (
                <ClipRow key={c.id} clipId={c.id} />
              ))}
            </ul>
          </Card>
          <RawSection raw={raw} />
        </section>
      )}

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
