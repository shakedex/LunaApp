import type { ClipRef } from '@luna-web/core'
import { useSelector } from '@tanstack/react-store'
import { Check, ImageOff, TriangleAlert } from 'lucide-react'
import { Button } from '@/components/ui/button'
import { Progress } from '@/components/ui/progress'
import { Spinner } from '@/components/ui/spinner'
import { useObjectUrl } from '@/lib/use-object-url'
import { cn } from '@/lib/utils'
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
  const queued = clips.filter((c) => statusOf(c.id) === 'queued')

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
              {queued.length > 0 && ` · ${queued.length} queued`}
            </p>
          </div>
          <Button variant="outline" size="sm" onClick={resetScan}>
            Start over
          </Button>
        </div>
        <Progress value={pct} />
      </div>

      {/* The metadata phase never produces images, so everything renders as
          compact chips. Only the thumbnail phase shows image-shaped elements. */}
      {active.length > 0 && (
        <div>
          <h2 className="text-muted-foreground mb-2 text-xs font-medium tracking-wide uppercase">
            In progress
          </h2>
          {inThumb ? (
            <div className="grid grid-cols-2 gap-3 sm:grid-cols-3 lg:grid-cols-4">
              {active.map((clip) => (
                <ClipTile key={clip.id} clip={clip} />
              ))}
            </div>
          ) : (
            <div className="flex flex-wrap items-center gap-1.5">
              {active.map((clip) => (
                <ClipChip key={clip.id} clip={clip} icon="active" />
              ))}
            </div>
          )}
        </div>
      )}

      {queued.length > 0 && (
        <div>
          <h2 className="text-muted-foreground mb-2 text-xs font-medium tracking-wide uppercase">
            Queued
            <span className="ml-1.5 font-mono tabular-nums">{queued.length}</span>
          </h2>
          <div className="flex flex-wrap items-center gap-1.5">
            {queued.slice(0, 12).map((clip) => (
              <ClipChip key={clip.id} clip={clip} />
            ))}
            {queued.length > 12 && (
              <span className="text-muted-foreground self-center font-mono text-xs tabular-nums">
                +{queued.length - 12} more
              </span>
            )}
          </div>
        </div>
      )}

      {doneClips.length > 0 && (
        <div>
          <h2 className="text-muted-foreground mb-2 text-xs font-medium tracking-wide uppercase">
            Completed
            <span className="ml-1.5 font-mono tabular-nums">{doneClips.length}</span>
          </h2>
          <div className="flex flex-wrap items-center gap-1.5">
            {inThumb
              ? doneClips.slice(-12).map((clip) => <DoneThumb key={clip.id} clip={clip} />)
              : doneClips
                  .slice(-12)
                  .map((clip) => <ClipChip key={clip.id} clip={clip} icon="done" />)}
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
          {inThumb ? (
            <div className="grid grid-cols-2 gap-3 sm:grid-cols-3 lg:grid-cols-4">
              {failed.map((clip) => (
                <ClipTile key={clip.id} clip={clip} />
              ))}
            </div>
          ) : (
            <div className="flex flex-wrap items-center gap-1.5">
              {failed.map((clip) => (
                <ClipChip key={clip.id} clip={clip} icon="failed" />
              ))}
            </div>
          )}
        </div>
      )}
    </section>
  )
}

// Compact filename chip — the metadata phase's only clip representation, and the
// queued list's in both phases. Never used inside the thumbnail strip.
function ClipChip({ clip, icon }: { clip: ClipRef; icon?: 'active' | 'done' | 'failed' }) {
  return (
    <span
      className={cn(
        'bg-card text-muted-foreground inline-flex max-w-48 items-center gap-1 rounded border px-2 py-0.5 font-mono text-2xs',
        icon === 'failed' && 'border-destructive/40 text-destructive',
      )}
      title={clip.fileName}
    >
      {icon === 'active' && <Spinner className="size-3 shrink-0" />}
      {icon === 'done' && <Check className="text-primary size-3 shrink-0" />}
      {icon === 'failed' && <TriangleAlert className="size-3 shrink-0" />}
      <span className="truncate">{clip.fileName}</span>
    </span>
  )
}

// Strip thumb for a finished clip — fades in as the worker completes it. A clip
// with no usable frame keeps the thumb shape with an icon, so the strip stays
// visually uniform (no text among images).
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
        <div className="text-muted-foreground flex h-full w-full items-center justify-center">
          <ImageOff className="size-3.5" />
        </div>
      )}
    </div>
  )
}
