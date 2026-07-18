import { buildReportModel } from '@luna-web/core'
import { useStore } from '@tanstack/react-store'
import { useMemo } from 'react'
import { Button } from '@/components/ui/button'
import { ClipRow, RawSection } from '@/features/scan/clip-row'
import { resetScan } from '@/features/scan/run-scan'
import { scanStore } from '@/features/scan/store'
import { formatBytes, formatDuration } from '@/lib/format'
import { CoverForm } from './cover-form'
import { coverStore } from './cover-store'

export function ReportWorkspace() {
  const clips = useStore(scanStore, (s) => s.clips)
  const raw = useStore(scanStore, (s) => s.raw)
  const metadataById = useStore(scanStore, (s) => s.metadataById)
  const thumbsById = useStore(scanStore, (s) => s.thumbsById)
  const cover = useStore(coverStore)

  const model = useMemo(
    () => buildReportModel({ clips, raw, metadataById, thumbsById, cover }),
    [clips, raw, metadataById, thumbsById, cover],
  )

  return (
    <section className="w-full">
      <div className="mb-4 flex items-center justify-between">
        <h2 className="text-xl font-medium">{model.cover.projectTitle || 'Camera report'}</h2>
        <Button variant="outline" onClick={resetScan}>
          Start over
        </Button>
      </div>

      <dl className="mb-6 grid grid-cols-5 gap-4 text-center">
        <Stat label="Cards" value={String(model.stats.cardCount)} />
        <Stat label="Clips" value={String(model.stats.clipCount)} />
        <Stat label="Duration" value={formatDuration(model.stats.totalDurationSeconds)} />
        <Stat label="Size" value={formatBytes(model.stats.totalSizeBytes)} />
        <Stat label="RAW" value={String(model.stats.rawCount)} />
      </dl>

      <div className="mb-6">
        <CoverForm />
      </div>

      {model.reels.map((reel) => (
        <section key={reel.name} className="mb-6">
          <h3 className="mb-2 flex items-baseline gap-3 text-lg font-medium">
            {reel.name}
            <span className="text-muted-foreground text-sm">
              {reel.clips.length} clips ·{' '}
              {formatBytes(reel.clips.reduce((n, c) => n + c.sizeBytes, 0))}
            </span>
          </h3>
          <ul className="divide-y rounded-lg border">
            {reel.clips.map((clip) => (
              <ClipRow key={clip.id} clipId={clip.id} />
            ))}
          </ul>
        </section>
      ))}

      <RawSection raw={model.raw} />
      <p className="text-muted-foreground mt-3 text-sm">
        Exports (PDF/CSV) arrive in the next milestone.
      </p>
    </section>
  )
}

function Stat({ label, value }: { label: string; value: string }) {
  return (
    <div>
      <dt className="text-muted-foreground text-sm">{label}</dt>
      <dd className="text-2xl">{value}</dd>
    </div>
  )
}
