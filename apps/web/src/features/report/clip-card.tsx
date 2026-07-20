import {
  CAMERA_FIELDS,
  type CameraFieldKey,
  joinPath,
  type ReportClip,
  type ThumbnailFrame,
} from '@luna-web/core'
import {
  Aperture,
  Camera,
  Clock,
  Film,
  Focus,
  Gauge,
  HardDrive,
  ImageOff,
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
import { Tooltip, TooltipContent, TooltipTrigger } from '@/components/ui/tooltip'
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

const CAMERA_ICONS: Record<CameraFieldKey, LucideIcon> = {
  camera: Camera,
  iso: Film,
  whiteBalance: Thermometer,
  lens: Focus,
  focalLength: Ruler,
  aperture: Aperture,
  shutter: Timer,
  gamma: SlidersHorizontal,
}

function cameraRows(clip: ReportClip<Blob>): Row[] {
  const m = clip.metadata
  return CAMERA_FIELDS.map((f) => ({ icon: CAMERA_ICONS[f.key], label: f.label, value: m[f.key] }))
}

function MetaRow({ icon: Icon, label, value }: { icon: LucideIcon; label: string; value: string }) {
  return (
    <div className="border-border/40 flex items-baseline justify-between gap-3 border-b py-1 text-sm break-inside-avoid">
      <dt className="text-muted-foreground flex items-center gap-1.5">
        <Icon className="size-3.5 shrink-0" />
        {label}
      </dt>
      <dd className="min-w-0">
        <Tooltip>
          <TooltipTrigger
            render={
              <span
                // biome-ignore lint/a11y/noNoninteractiveTabindex: Base UI's TooltipTrigger renders a <button> by default; swapping to a <span> via `render` needs an explicit tabIndex so the tooltip still opens on keyboard focus (Base UI does not inject one for custom render elements).
                tabIndex={0}
                className="block max-w-56 truncate text-right font-mono tabular-nums"
              />
            }
          >
            {value}
          </TooltipTrigger>
          <TooltipContent className="font-mono">{value}</TooltipContent>
        </Tooltip>
      </dd>
    </div>
  )
}

function MetaList({ clip }: { clip: ReportClip<Blob> }) {
  const groups = [
    { title: 'Technical', rows: technicalRows(clip).filter((r) => r.value), emptyNote: undefined },
    {
      title: 'Camera',
      rows: cameraRows(clip).filter((r) => r.value),
      emptyNote: 'No camera metadata in this file',
    },
  ] as const
  return (
    <div className="gap-x-6 self-start sm:columns-2">
      {groups.map(({ title, rows, emptyNote }) => (
        <section key={title} className="mb-3">
          <div className="text-muted-foreground mb-2 text-2xs font-medium tracking-wider uppercase">
            {title}
          </div>
          {rows.length === 0 ? (
            <p className="text-muted-foreground-dim text-xs italic">{emptyNote ?? '—'}</p>
          ) : (
            <dl>
              {rows.map((row) => (
                <MetaRow
                  key={row.label}
                  icon={row.icon}
                  label={row.label}
                  value={row.value as string}
                />
              ))}
            </dl>
          )}
        </section>
      ))}
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
      <div className="bg-muted/40 text-muted-foreground flex aspect-video w-full flex-col items-center justify-center gap-1.5 rounded-lg border border-dashed text-sm">
        <ImageOff className="size-5" />
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
          <span className="bg-background/70 absolute right-2 bottom-2 rounded px-1.5 py-0.5 font-mono text-2xs tabular-nums backdrop-blur">
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
                'aspect-video w-20 shrink-0 overflow-hidden rounded border outline-none transition focus-visible:ring-3 focus-visible:ring-ring/50',
                i === active
                  ? 'border-primary ring-primary ring-1'
                  : 'border-border opacity-60 hover:opacity-100',
              )}
            >
              {urls[i] ? (
                <img src={urls[i] as string} alt="" className="h-full w-full object-cover" />
              ) : (
                <span className="bg-muted text-muted-foreground flex h-full items-center justify-center text-2xs">
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
        <MetaList clip={clip} />
      </div>
    </Card>
  )
}
