import type { ClipRef, RawNotice, ThumbnailFrame } from '@luna-web/core'
import { useStore } from '@tanstack/react-store'
import { memo, useEffect, useState } from 'react'
import { Badge } from '@/components/ui/badge'
import { Skeleton } from '@/components/ui/skeleton'
import { formatBytes, formatDuration } from '@/lib/format'
import { scanStore } from './store'

const clipRefCache = new WeakMap<readonly ClipRef[], Map<string, ClipRef>>()

function clipById(clips: readonly ClipRef[], id: string): ClipRef | undefined {
  let map = clipRefCache.get(clips)
  if (!map) {
    map = new Map(clips.map((c) => [c.id, c]))
    clipRefCache.set(clips, map)
  }
  return map.get(id)
}

export const ClipRow = memo(function ClipRow({ clipId }: { clipId: string }) {
  const clips = useStore(scanStore, (s) => s.clips)
  const clip = clipById(clips, clipId)
  const status = useStore(scanStore, (s) => s.clipStatus[clipId] ?? 'queued')
  const m = useStore(scanStore, (s) => s.metadataById[clipId])
  const thumbStatus = useStore(scanStore, (s) => s.thumbStatus[clipId])
  const frames = useStore(scanStore, (s) => s.thumbsById[clipId])
  const error = useStore(scanStore, (s) => s.clipErrors[clipId])

  if (!clip) return null

  return (
    <li className="flex flex-col gap-2 px-4 py-2 text-sm">
      {thumbStatus === 'queued' || thumbStatus === 'decoding' ? (
        <div className="flex gap-1">
          {[0, 1, 2].map((i) => (
            <Skeleton key={i} className="h-14 w-24 rounded-md" />
          ))}
        </div>
      ) : (
        <ThumbStrip frames={frames} />
      )}
      <div className="grid grid-cols-[1fr_auto_auto_auto_auto_auto] items-center gap-x-4">
        <span className="truncate font-mono text-[0.8rem]">{clip.relativePath}</span>
        <span className="text-muted-foreground font-mono tabular-nums">
          {m?.width !== undefined && m?.height !== undefined ? `${m.width}×${m.height}` : '—'}
        </span>
        <span className="text-muted-foreground font-mono">{m?.codec ?? '—'}</span>
        <span className="text-muted-foreground font-mono tabular-nums">
          {m?.frameRate !== undefined ? `${m.frameRate} fps` : '—'}
        </span>
        <span className="text-muted-foreground font-mono tabular-nums">
          {m?.durationSeconds !== undefined ? formatDuration(m.durationSeconds) : '—'}
        </span>
        <span className="font-mono tabular-nums">
          {status === 'done' ? (
            formatBytes(clip.sizeBytes)
          ) : (
            <StatusBadge status={status} error={error} />
          )}
        </span>
      </div>
    </li>
  )
})

export function RawSection({ raw }: { raw: RawNotice[] }) {
  if (raw.length === 0) return null
  return (
    <section className="mt-6">
      <h3 className="text-muted-foreground mb-2 text-sm font-medium">
        RAW files (not decodable in browser)
      </h3>
      <ul className="divide-y rounded-lg border">
        {raw.map((r) => (
          <li
            key={r.id}
            className="text-muted-foreground flex items-center justify-between px-4 py-2 text-sm"
          >
            <span className="truncate">{r.relativePath}</span>
            <span className="ml-4 shrink-0">RAW · {formatBytes(r.sizeBytes)}</span>
          </li>
        ))}
      </ul>
    </section>
  )
}

export function ThumbStrip({ frames }: { frames: ThumbnailFrame<Blob>[] | undefined }) {
  const [urls, setUrls] = useState<string[]>([])

  useEffect(() => {
    if (!frames) return
    const next = frames.map((f) => (f.image ? URL.createObjectURL(f.image) : ''))
    setUrls(next)
    return () => {
      for (const u of next) if (u) URL.revokeObjectURL(u)
    }
  }, [frames])

  if (!frames) return null
  return (
    <div className="flex gap-1">
      {frames.map((f, i) =>
        f.outcome === 'Success' && urls[i] ? (
          <img
            key={f.positionRatio}
            src={urls[i]}
            alt={`Frame at ${Math.round(f.positionRatio * 100)}%`}
            className="border-border h-14 rounded border object-cover transition-transform hover:scale-[1.02]"
            loading="lazy"
          />
        ) : (
          <div
            key={f.positionRatio}
            className="bg-muted text-muted-foreground flex h-14 w-24 items-center justify-center rounded text-xs"
            title={f.outcome}
          >
            {f.outcome === 'NotAttempted' ? 'RAW' : 'n/a'}
          </div>
        ),
      )}
    </div>
  )
}

export function StatusBadge({ status, error }: { status: string; error?: string }) {
  if (status === 'failed') {
    return (
      <Badge variant="destructive" title={error}>
        failed
      </Badge>
    )
  }
  return (
    <Badge variant="secondary" className="text-muted-foreground">
      {status}…
    </Badge>
  )
}
