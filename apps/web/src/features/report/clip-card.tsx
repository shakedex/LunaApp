import { joinPath, type ReportClip, type ThumbnailFrame } from '@luna-web/core'
import {
  Aperture,
  Camera,
  Clock,
  Film,
  Focus,
  Gauge,
  HardDrive,
  type LucideIcon,
  Palette,
  Ratio,
  Ruler,
  SlidersHorizontal,
  Thermometer,
  Timer,
  Video,
} from 'lucide-react'
import { useEffect, useState } from 'react'
import { Badge } from '@/components/ui/badge'
import { Card } from '@/components/ui/card'
import { formatBytes, formatDuration } from '@/lib/format'
import { cn } from '@/lib/utils'

type Row = { icon: LucideIcon; label: string; value: string | undefined }

function technicalRows(clip: ReportClip<Blob>): Row[] {
  const m = clip.metadata
  return [
    {
      icon: Ratio,
      label: 'Resolution',
      value: m.width && m.height ? `${m.width}×${m.height}` : undefined,
    },
    { icon: Video, label: 'Codec', value: m.codec },
    {
      icon: Gauge,
      label: 'Frame rate',
      value: m.frameRate !== undefined ? `${m.frameRate} fps` : undefined,
    },
    {
      icon: Clock,
      label: 'Duration',
      value: m.durationSeconds !== undefined ? formatDuration(m.durationSeconds) : undefined,
    },
    { icon: HardDrive, label: 'Size', value: formatBytes(clip.sizeBytes) },
    { icon: Palette, label: 'Color space', value: m.colorSpace },
  ]
}

function cameraRows(clip: ReportClip<Blob>): Row[] {
  const m = clip.metadata
  return [
    { icon: Camera, label: 'Camera', value: m.camera },
    { icon: Film, label: 'ISO', value: m.iso },
    { icon: Thermometer, label: 'White balance', value: m.whiteBalance },
    { icon: Focus, label: 'Lens', value: m.lens },
    { icon: Ruler, label: 'Focal length', value: m.focalLength },
    { icon: Aperture, label: 'Aperture', value: m.aperture },
    { icon: Timer, label: 'Shutter', value: m.shutter },
    { icon: SlidersHorizontal, label: 'Gamma', value: m.gamma },
  ]
}

function MetaColumn({
  title,
  rows,
  emptyNote,
}: {
  title: string
  rows: Row[]
  emptyNote?: string
}) {
  const present = rows.filter((r) => r.value)
  return (
    <div className="min-w-0">
      <div className="text-muted-foreground mb-2 text-[0.65rem] font-medium tracking-wider uppercase">
        {title}
      </div>
      {present.length === 0 ? (
        <p className="text-muted-foreground/70 text-xs italic">{emptyNote ?? '—'}</p>
      ) : (
        <dl className="space-y-1">
          {present.map(({ icon: Icon, label, value }) => (
            <div
              key={label}
              className="border-border/40 flex items-baseline justify-between gap-3 border-b pb-1 text-sm"
            >
              <dt className="text-muted-foreground flex items-center gap-1.5">
                <Icon className="size-3.5 shrink-0" />
                {label}
              </dt>
              <dd className="truncate text-right font-mono tabular-nums">{value}</dd>
            </div>
          ))}
        </dl>
      )}
    </div>
  )
}

function FrameViewer({ frames }: { frames: ThumbnailFrame<Blob>[] }) {
  const [active, setActive] = useState(0)
  const [urls, setUrls] = useState<(string | null)[]>([])

  useEffect(() => {
    const next = frames.map((f) => (f.image ? URL.createObjectURL(f.image) : null))
    setUrls(next)
    setActive(0)
    return () => {
      for (const u of next) if (u) URL.revokeObjectURL(u)
    }
  }, [frames])

  const hasAny = frames.some((f) => f.outcome === 'Success' && f.image)
  if (!hasAny) {
    return (
      <div className="bg-muted/40 text-muted-foreground flex aspect-video w-full items-center justify-center rounded-lg border border-dashed text-sm">
        No preview frames
      </div>
    )
  }

  const activeUrl = urls[active]
  const activeFrame = frames[active]

  return (
    <div className="space-y-2">
      <div className="bg-muted/40 relative aspect-video w-full overflow-hidden rounded-lg border">
        {activeUrl ? (
          <img
            src={activeUrl}
            alt={activeFrame ? `Frame at ${formatDuration(activeFrame.timestampSeconds)}` : 'Frame'}
            className="h-full w-full object-contain"
          />
        ) : (
          <div className="text-muted-foreground flex h-full items-center justify-center text-xs">
            {activeFrame?.outcome ?? 'n/a'}
          </div>
        )}
        {activeFrame && (
          <span className="bg-background/70 absolute right-2 bottom-2 rounded px-1.5 py-0.5 font-mono text-[0.7rem] tabular-nums backdrop-blur">
            {formatDuration(activeFrame.timestampSeconds)}
          </span>
        )}
      </div>
      {frames.length > 1 && (
        <div className="flex gap-2">
          {frames.map((f, i) => (
            <button
              key={f.positionRatio}
              type="button"
              aria-label={`Show frame at ${formatDuration(f.timestampSeconds)}`}
              onClick={() => setActive(i)}
              className={cn(
                'aspect-video w-20 shrink-0 overflow-hidden rounded border transition',
                i === active
                  ? 'border-primary ring-primary ring-1'
                  : 'border-border opacity-60 hover:opacity-100',
              )}
            >
              {urls[i] ? (
                <img src={urls[i] as string} alt="" className="h-full w-full object-cover" />
              ) : (
                <span className="bg-muted text-muted-foreground flex h-full items-center justify-center text-[0.6rem]">
                  n/a
                </span>
              )}
            </button>
          ))}
        </div>
      )}
    </div>
  )
}

export function ClipCard({ clip, sourceRoot }: { clip: ReportClip<Blob>; sourceRoot: string }) {
  const fullPath = joinPath(sourceRoot, clip.relativePath)
  return (
    <Card className="p-4">
      <div className="mb-3 flex items-baseline justify-between gap-3">
        <div className="min-w-0">
          <span className="truncate font-mono text-sm font-medium">{clip.fileName}</span>
          <p className="text-muted-foreground truncate font-mono text-xs" title={fullPath}>
            {fullPath}
          </p>
        </div>
        {clip.metadata.startTimecode && (
          <Badge variant="secondary" className="shrink-0 font-mono tabular-nums">
            TC {clip.metadata.startTimecode}
          </Badge>
        )}
      </div>
      <div className="grid gap-5 lg:grid-cols-[1.1fr_1fr]">
        <FrameViewer frames={clip.thumbnails} />
        <div className="grid grid-cols-2 gap-x-6 gap-y-4 self-start">
          <MetaColumn title="Technical" rows={technicalRows(clip)} />
          <MetaColumn
            title="Camera"
            rows={cameraRows(clip)}
            emptyNote="No camera metadata in this file"
          />
        </div>
      </div>
    </Card>
  )
}
