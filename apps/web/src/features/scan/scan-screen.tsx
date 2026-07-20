import { useSelector } from '@tanstack/react-store'
import { ArrowRight, CircleAlert, FolderCheck } from 'lucide-react'
import { useEffect } from 'react'
import { StatTile } from '@/components/stat-tile'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { Card } from '@/components/ui/card'
import {
  Empty,
  EmptyContent,
  EmptyDescription,
  EmptyHeader,
  EmptyMedia,
  EmptyTitle,
} from '@/components/ui/empty'
import { Kbd } from '@/components/ui/kbd'
import { Spinner } from '@/components/ui/spinner'
import { startProcessing } from '@/features/process/run-processing'
import { ReportWorkspace } from '@/features/report/report-workspace'
import { formatBytes } from '@/lib/format'
import { cn } from '@/lib/utils'
import { CompositionBar } from './composition-bar'
import { Dropzone } from './dropzone'
import { ProcessingView } from './processing-view'
import { resetScan } from './run-scan'
import { scanStore } from './store'

// "MOV, MXF · ready to process" — a glance-able identity line from the clip
// formats (most common first), matching the composition bar's ordering.
function formatSummaryLine(byExtension: Record<string, number>): string {
  const formats = Object.entries(byExtension)
    .sort((a, b) => b[1] - a[1])
    .map(([ext]) => ext.replace(/^\./, '').toUpperCase())
    .join(', ')
  return formats ? `${formats} · ready to process` : 'Ready to process'
}

// Split "2.6 GB" into value + unit so the tile can de-emphasise the unit.
function splitBytes(n: number): { value: string; unit: string } {
  const parts = formatBytes(n).split(' ')
  return { value: parts[0] ?? '', unit: parts[1] ?? '' }
}

export function ScanScreen() {
  const phase = useSelector(scanStore, (s) => s.phase)
  const sourceName = useSelector(scanStore, (s) => s.sourceName)
  const progress = useSelector(scanStore, (s) => s.progress)
  const summary = useSelector(scanStore, (s) => s.summary)
  const error = useSelector(scanStore, (s) => s.error)
  const generateThumbnails = useSelector(scanStore, (s) => s.generateThumbnails)

  // On the summary screen, Enter starts the run and Esc backs out — the two CTA
  // actions, reachable without pointing at the buttons.
  useEffect(() => {
    if (phase !== 'summary') return
    function onKey(e: KeyboardEvent) {
      if (e.key === 'Enter') {
        e.preventDefault()
        void startProcessing()
      } else if (e.key === 'Escape') {
        e.preventDefault()
        resetScan()
      }
    }
    window.addEventListener('keydown', onKey)
    return () => window.removeEventListener('keydown', onKey)
  }, [phase])

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
        <Card className="w-full gap-0 py-0">
          <div className="flex items-center gap-3 px-5 py-4">
            <span className="bg-primary/10 text-primary flex size-10 shrink-0 items-center justify-center rounded-lg">
              <FolderCheck className="size-5" />
            </span>
            <div className="min-w-0 flex-1">
              <div className="truncate font-medium">{sourceName}</div>
              <div className="text-muted-foreground truncate font-mono text-xs">
                {formatSummaryLine(summary.byExtension)}
              </div>
            </div>
            <Badge
              variant="secondary"
              className="bg-primary/10 text-primary gap-1.5 rounded-full px-2.5"
            >
              <span className="bg-primary size-1.5 rounded-full" />
              Ready
            </Badge>
          </div>

          <div className="divide-border grid grid-cols-3 divide-x border-t">
            <div className="px-5 py-4">
              <StatTile label="Clips" value={String(summary.clipCount)} accent />
            </div>
            <div className="px-5 py-4">
              <StatTile
                label="Other files"
                value={String(summary.otherFileCount)}
                dim={summary.otherFileCount === 0}
              />
            </div>
            <div className="px-5 py-4">
              <StatTile label="Total size" {...splitBytes(summary.totalSizeBytes)} />
            </div>
          </div>

          <div className="border-t px-5 py-4">
            <CompositionBar
              byExtension={summary.byExtension}
              otherFileCount={summary.otherFileCount}
            />
          </div>

          <div className="flex items-center gap-4 border-t px-5 py-4">
            <div className="flex-1">
              <div className="text-sm font-medium">Thumbnails</div>
              <div className="text-muted-foreground mt-0.5 text-xs">
                Off skips frame decoding for a faster, metadata-only report.
              </div>
            </div>
            <fieldset
              aria-label="Thumbnails"
              className="bg-secondary flex shrink-0 gap-0.5 rounded-lg border p-0.5"
            >
              {(
                [
                  ['On', true],
                  ['Off', false],
                ] as const
              ).map(([text, on]) => (
                <button
                  key={text}
                  type="button"
                  aria-pressed={generateThumbnails === on}
                  onClick={() => scanStore.setState((s) => ({ ...s, generateThumbnails: on }))}
                  className={cn(
                    'rounded-md px-3 py-1.5 text-sm font-medium transition',
                    generateThumbnails === on
                      ? 'bg-primary text-primary-foreground'
                      : 'text-muted-foreground hover:text-foreground',
                  )}
                >
                  {text}
                </button>
              ))}
            </fieldset>
          </div>

          <div className="flex items-center gap-3 border-t px-5 py-4">
            <Button size="lg" className="shadow-glow" onClick={() => void startProcessing()}>
              Process {summary.clipCount} {summary.clipCount === 1 ? 'clip' : 'clips'}
              <ArrowRight className="size-4" />
            </Button>
            <Button variant="outline" size="lg" onClick={resetScan}>
              Cancel
            </Button>
            <div className="text-muted-foreground ml-auto hidden items-center gap-3 text-xs sm:flex">
              <span className="flex items-center gap-1.5">
                <Kbd>Enter</Kbd> process
              </span>
              <span className="flex items-center gap-1.5">
                <Kbd>Esc</Kbd> cancel
              </span>
            </div>
          </div>
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
