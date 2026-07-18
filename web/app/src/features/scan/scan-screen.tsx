import { useStore } from '@tanstack/react-store'
import { Button } from '@/components/ui/button'
import { formatBytes } from '@/lib/format'
import { RecentList } from './recent-list'
import { confirmScan, pickAndScan, resetScan } from './run-scan'
import { scanStore } from './store'

export function ScanScreen() {
  const state = useStore(scanStore)

  return (
    <main className="mx-auto flex min-h-screen max-w-3xl flex-col items-center justify-center gap-6 px-6 py-12">
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
            <Button onClick={confirmScan}>Process {state.summary.clipCount} clips</Button>
            <Button variant="outline" onClick={resetScan}>
              Cancel
            </Button>
          </div>
        </section>
      )}

      {state.phase === 'confirmed' && (
        <section className="w-full">
          <div className="mb-4 flex items-center justify-between">
            <h2 className="text-xl font-medium">{state.clips.length} clips ready</h2>
            <Button variant="outline" onClick={resetScan}>
              Start over
            </Button>
          </div>
          <ul className="divide-y rounded-lg border">
            {state.clips.map((c) => (
              <li key={c.id} className="flex items-center justify-between px-4 py-2 text-sm">
                <span className="truncate">{c.relativePath}</span>
                <span className="text-muted-foreground ml-4 shrink-0">
                  {formatBytes(c.sizeBytes)}
                </span>
              </li>
            ))}
          </ul>
          <p className="text-muted-foreground mt-3 text-sm">
            Metadata and thumbnails arrive in the next milestone.
          </p>

          {state.raw.length > 0 && (
            <div className="mt-6">
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
                    <span className="ml-4 flex shrink-0 items-center gap-2">
                      {formatBytes(r.sizeBytes)}
                      <span className="rounded border px-1.5 py-0.5 text-xs">RAW</span>
                    </span>
                  </li>
                ))}
              </ul>
            </div>
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
