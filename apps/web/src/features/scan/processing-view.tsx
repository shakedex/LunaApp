import type { ClipRef } from '@luna-web/core'
import { useSelector } from '@tanstack/react-store'
import { Button } from '@/components/ui/button'
import { Progress } from '@/components/ui/progress'
import { useObjectUrl } from '@/lib/use-object-url'
import { ClipTile } from './clip-tile'
import { resetScan } from './run-scan'
import { scanStore } from './store'

export function ProcessingView() {
  const phase = useSelector(scanStore, (s) => s.phase)
  const clips = useSelector(scanStore, (s) => s.clips)
  const clipStatus = useSelector(scanStore, (s) => s.clipStatus)
  const thumbStatus = useSelector(scanStore, (s) => s.thumbStatus)
  const processedCount = useSelector(scanStore, (s) => s.processedCount)
  const thumbDoneCount = useSelector(scanStore, (s) => s.thumbDoneCount)

  const inThumb = phase === 'thumbnailing'
  const statusOf = (id: string) =>
    inThumb ? (thumbStatus[id] ?? 'queued') : (clipStatus[id] ?? 'queued')

  const active = clips.filter((c) => {
    const s = statusOf(c.id)
    return s === 'processing' || s === 'decoding'
  })
  const failed = clips.filter((c) => statusOf(c.id) === 'failed')
  const doneClips = clips.filter((c) => statusOf(c.id) === 'done')
  const queuedCount = clips.length - active.length - failed.length - doneClips.length

  const done = inThumb ? thumbDoneCount : processedCount
  const total = inThumb ? Object.keys(thumbStatus).length || clips.length : clips.length
  const pct = total > 0 ? (done / total) * 100 : 0

  return (
    <section className="space-y-5">
      <div className="space-y-3">
        <div className="flex items-end justify-between gap-4">
          <div>
            <h1 className="text-lg font-semibold" aria-live="polite">
              {inThumb ? 'Generating thumbnails' : 'Reading metadata'}
            </h1>
            <p className="text-muted-foreground font-mono text-sm tabular-nums">
              {done} / {total} clips
              {queuedCount > 0 && ` · ${queuedCount} queued`}
            </p>
          </div>
          <Button variant="outline" size="sm" onClick={resetScan}>
            Start over
          </Button>
        </div>
        <Progress value={pct} />
      </div>

      {active.length > 0 && (
        <div>
          <h2 className="text-muted-foreground mb-2 text-xs font-medium tracking-wide uppercase">
            In progress
          </h2>
          <div className="grid grid-cols-2 gap-3 sm:grid-cols-3 lg:grid-cols-4">
            {active.map((clip) => (
              <ClipTile key={clip.id} clip={clip} />
            ))}
          </div>
        </div>
      )}

      {doneClips.length > 0 && (
        <div>
          <h2 className="text-muted-foreground mb-2 text-xs font-medium tracking-wide uppercase">
            Completed
            <span className="ml-1.5 font-mono tabular-nums">{doneClips.length}</span>
          </h2>
          <div className="flex flex-wrap gap-1.5">
            {doneClips.slice(-12).map((clip) => (
              <DoneThumb key={clip.id} clip={clip} />
            ))}
            {doneClips.length > 12 && (
              <span className="text-muted-foreground self-center font-mono text-xs tabular-nums">
                +{doneClips.length - 12} more
              </span>
            )}
          </div>
        </div>
      )}

      {failed.length > 0 && (
        <div>
          <h2 className="text-destructive mb-2 text-xs font-medium tracking-wide uppercase">
            Failed
            <span className="ml-1.5 font-mono tabular-nums">{failed.length}</span>
          </h2>
          <div className="grid grid-cols-2 gap-3 sm:grid-cols-3 lg:grid-cols-4">
            {failed.map((clip) => (
              <ClipTile key={clip.id} clip={clip} />
            ))}
          </div>
        </div>
      )}
    </section>
  )
}

// Compact strip thumb for a finished clip — fades in as the worker completes it.
function DoneThumb({ clip }: { clip: ClipRef }) {
  const frames = useSelector(scanStore, (s) => s.thumbsById[clip.id])
  const first = frames?.find((f) => f.outcome === 'Success' && f.image)?.image
  const url = useObjectUrl(first)
  return (
    <div
      className="bg-card animate-in fade-in zoom-in-95 aspect-video w-16 overflow-hidden rounded border duration-200"
      title={clip.fileName}
    >
      {url ? (
        <img src={url} alt="" className="h-full w-full object-cover" />
      ) : (
        <div className="bg-muted/40 h-full w-full" />
      )}
    </div>
  )
}
