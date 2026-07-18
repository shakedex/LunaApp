import type { ClipRef } from '@luna-web/core'
import { useStore } from '@tanstack/react-store'
import { Button } from '@/components/ui/button'
import { startProcessing } from '@/features/process/run-processing'
import { formatBytes, formatDuration } from '@/lib/format'
import { RecentList } from './recent-list'
import { pickAndScan, resetScan } from './run-scan'
import { type ScanState, scanStore } from './store'

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

      {(state.phase === 'processing' || state.phase === 'processed') && (
        <section className="w-full">
          <div className="mb-4 flex items-center justify-between">
            <h2 className="text-xl font-medium" aria-live="polite">
              {state.phase === 'processing'
                ? `Reading metadata… ${state.processedCount}/${state.clips.length}`
                : `${state.clips.length} clips processed`}
            </h2>
            <Button variant="outline" onClick={resetScan}>
              Start over
            </Button>
          </div>
          <ClipTable state={state} />
          {state.raw.length > 0 && (
            <section className="mt-6">
              <h3 className="text-muted-foreground mb-2 text-sm font-medium">
                RAW files (not decodable in browser)
              </h3>
              <ul className="divide-y rounded-lg border">
                {state.raw.map((r) => (
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
          )}
          {state.phase === 'processed' && (
            <p className="text-muted-foreground mt-3 text-sm">
              Thumbnails arrive in the next milestone.
            </p>
          )}
        </section>
      )}

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

function ClipTable({ state }: { state: ScanState }) {
  return (
    <ul className="divide-y rounded-lg border">
      {state.clips.map((c) => (
        <ClipRow key={c.id} clip={c} state={state} />
      ))}
    </ul>
  )
}

function ClipRow({ clip, state }: { clip: ClipRef; state: ScanState }) {
  const status = state.clipStatus[clip.id] ?? 'queued'
  const m = state.metadataById[clip.id]
  return (
    <li className="grid grid-cols-[1fr_auto_auto_auto_auto_auto] items-center gap-x-4 px-4 py-2 text-sm">
      <span className="truncate">{clip.relativePath}</span>
      <span className="text-muted-foreground tabular-nums">
        {m?.width && m?.height ? `${m.width}×${m.height}` : '—'}
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
          <StatusBadge status={status} error={state.clipErrors[clip.id]} />
        )}
      </span>
    </li>
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
