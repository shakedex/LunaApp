import type { RawNotice, ThumbnailFrame } from '@luna-web/core'
import { useStore } from '@tanstack/react-store'
import { useEffect, useState } from 'react'
import { Button } from '@/components/ui/button'
import { startProcessing } from '@/features/process/run-processing'
import { ReportWorkspace } from '@/features/report/report-workspace'
import { formatBytes, formatDuration } from '@/lib/format'
import { RecentList } from './recent-list'
import { pickAndScan, resetScan } from './run-scan'
import { scanStore } from './store'

export function ScanScreen() {
  const state = useStore(scanStore)

  return (
    <main className="mx-auto flex min-h-screen max-w-4xl flex-col items-center justify-center gap-6 px-6 py-12">
      <h1 className="font-heading text-3xl font-semibold">Luna Web</h1>

      {state.phase === 'idle' && (
        <>
          <p className="text-muted-foreground">
            Pick a footage folder — everything stays on this device.
          </p>
          <Button onClick={() => void pickAndScan()}>Pick folder</Button>
          <RecentList />
        </>
      )}

      {state.phase === 'scanning' && (
        <p className="text-muted-foreground" aria-live="polite">
          Scanning {state.sourceName}…{' '}
          {state.progress
            ? `${state.progress.filesSeen} files, ${state.progress.clipsFound} clips`
            : ''}
        </p>
      )}

      {state.phase === 'summary' && state.summary && (
        <section className="w-full rounded-lg border p-6">
          <h2 className="mb-4 text-xl font-medium">{state.sourceName}</h2>
          <dl className="mb-4 grid grid-cols-3 gap-4 text-center">
            <div>
              <dt className="text-muted-foreground text-sm">Clips</dt>
              <dd className="text-2xl">{state.summary.clipCount}</dd>
            </div>
            <div>
              <dt className="text-muted-foreground text-sm">Total size</dt>
              <dd className="text-2xl">{formatBytes(state.summary.totalClipSizeBytes)}</dd>
            </div>
            <div>
              <dt className="text-muted-foreground text-sm">RAW (unsupported)</dt>
              <dd className="text-2xl">{state.summary.rawCount}</dd>
            </div>
          </dl>
          {state.summary.rawCount > 0 && (
            <p className="text-muted-foreground mb-4 text-sm">
              {state.summary.rawCount} ARRIRAW/R3D/BRAW file(s) were detected but cannot be decoded
              in a browser — they will be listed without thumbnails.
            </p>
          )}
          <div className="flex gap-3">
            <Button onClick={() => void startProcessing()}>
              Process {state.summary.clipCount} clips
            </Button>
            <Button variant="outline" onClick={resetScan}>
              Cancel
            </Button>
          </div>
        </section>
      )}

      {(state.phase === 'processing' || state.phase === 'thumbnailing') && (
        <section className="w-full">
          <div className="mb-4 flex items-center justify-between">
            <h2 className="text-xl font-medium" aria-live="polite">
              {state.phase === 'processing'
                ? `Reading metadata… ${state.processedCount}/${state.clips.length}`
                : `Generating thumbnails… ${state.thumbDoneCount}/${Object.keys(state.thumbStatus).length}`}
            </h2>
            <Button variant="outline" onClick={resetScan}>
              Start over
            </Button>
          </div>
          <ul className="divide-y rounded-lg border">
            {state.clips.map((c) => (
              <ClipRow key={c.id} clipId={c.id} />
            ))}
          </ul>
          <RawSection raw={state.raw} />
        </section>
      )}

      {state.phase === 'processed' && <ReportWorkspace />}

      {state.phase === 'error' && (
        <section className="text-center">
          <p className="text-destructive mb-4">{state.error}</p>
          <Button variant="outline" onClick={resetScan}>
            Back
          </Button>
        </section>
      )}
    </main>
  )
}

export function ClipRow({ clipId }: { clipId: string }) {
  const clip = useStore(scanStore, (s) => s.clips.find((c) => c.id === clipId))
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
            <div key={i} className="bg-muted h-14 w-24 animate-pulse rounded" />
          ))}
        </div>
      ) : (
        <ThumbStrip frames={frames} />
      )}
      <div className="grid grid-cols-[1fr_auto_auto_auto_auto_auto] items-center gap-x-4">
        <span className="truncate">{clip.relativePath}</span>
        <span className="text-muted-foreground tabular-nums">
          {m?.width !== undefined && m?.height !== undefined ? `${m.width}×${m.height}` : '—'}
        </span>
        <span className="text-muted-foreground">{m?.codec ?? '—'}</span>
        <span className="text-muted-foreground tabular-nums">
          {m?.frameRate !== undefined ? `${m.frameRate} fps` : '—'}
        </span>
        <span className="text-muted-foreground tabular-nums">
          {m?.durationSeconds !== undefined ? formatDuration(m.durationSeconds) : '—'}
        </span>
        <span className="text-muted-foreground tabular-nums">
          {status === 'done' ? (
            formatBytes(clip.sizeBytes)
          ) : (
            <StatusBadge status={status} error={error} />
          )}
        </span>
      </div>
    </li>
  )
}

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

function ThumbStrip({ frames }: { frames: ThumbnailFrame<Blob>[] | undefined }) {
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
            className="h-14 rounded object-cover"
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

function StatusBadge({ status, error }: { status: string; error?: string }) {
  if (status === 'failed') {
    return (
      <span className="text-destructive" title={error}>
        failed
      </span>
    )
  }
  return <span className="text-muted-foreground">{status}…</span>
}
