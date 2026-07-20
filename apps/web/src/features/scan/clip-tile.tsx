import type { ClipRef } from '@luna-web/core'
import { useSelector } from '@tanstack/react-store'
import { TriangleAlert } from 'lucide-react'
import { memo } from 'react'
import { Skeleton } from '@/components/ui/skeleton'
import { Spinner } from '@/components/ui/spinner'
import { useObjectUrl } from '@/lib/use-object-url'
import { scanStore } from './store'

export const ClipTile = memo(function ClipTile({ clip }: { clip: ClipRef }) {
  const status = useSelector(scanStore, (s) => s.clipStatus[clip.id] ?? 'queued')
  const thumbStatus = useSelector(scanStore, (s) => s.thumbStatus[clip.id])
  const frames = useSelector(scanStore, (s) => s.thumbsById[clip.id])
  const m = useSelector(scanStore, (s) => s.metadataById[clip.id])

  const firstImage = frames?.find((f) => f.outcome === 'Success' && f.image)?.image
  const url = useObjectUrl(firstImage)

  const failed = status === 'failed' || thumbStatus === 'failed'

  return (
    <div className="bg-card overflow-hidden rounded-lg border">
      <div className="bg-muted/40 relative aspect-video">
        {url ? (
          <img src={url} alt="" className="h-full w-full object-cover" />
        ) : failed ? (
          <div className="text-muted-foreground flex h-full items-center justify-center text-xs">
            no preview
          </div>
        ) : (
          <Skeleton className="h-full w-full rounded-none" />
        )}
        {failed ? (
          <span className="bg-destructive/15 text-destructive absolute top-1.5 right-1.5 flex size-5 items-center justify-center rounded-full">
            <TriangleAlert className="size-3" />
          </span>
        ) : !url ? (
          <span className="bg-background/70 absolute top-1.5 right-1.5 flex size-5 items-center justify-center rounded-full backdrop-blur">
            <Spinner className="size-3" />
          </span>
        ) : null}
      </div>
      <div className="min-w-0 p-2">
        <p className="truncate font-mono text-xs" title={clip.relativePath}>
          {clip.fileName}
        </p>
        <p className="text-muted-foreground mt-0.5 truncate font-mono text-2xs tabular-nums">
          {m?.width && m?.height ? `${m.width}×${m.height}` : '—'}
          {m?.codec ? ` · ${m.codec}` : ''}
        </p>
      </div>
    </div>
  )
})
