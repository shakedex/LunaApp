import { buildReportModel } from '@luna-web/core'
import { useStore } from '@tanstack/react-store'
import { useMemo } from 'react'
import { StatTile } from '@/components/stat-tile'
import { Button } from '@/components/ui/button'
import { Card, CardContent } from '@/components/ui/card'
import { ExportButtons } from '@/features/export/export-buttons'
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
        <div className="flex items-center gap-2">
          <ExportButtons report={model} />
          <Button variant="outline" onClick={resetScan}>
            Start over
          </Button>
        </div>
      </div>

      <Card className="mb-6">
        <CardContent>
          <div className="grid grid-cols-5 gap-4">
            <StatTile label="Cards" value={String(model.stats.cardCount)} />
            <StatTile label="Clips" value={String(model.stats.clipCount)} />
            <StatTile label="Duration" value={formatDuration(model.stats.totalDurationSeconds)} />
            <StatTile label="Size" value={formatBytes(model.stats.totalSizeBytes)} />
            <StatTile label="RAW" value={String(model.stats.rawCount)} />
          </div>
        </CardContent>
      </Card>

      <div className="mb-6">
        <CoverForm />
      </div>

      {model.reels.map((reel) => (
        <section key={reel.name} className="mb-6">
          <h3 className="bg-background/80 sticky top-14 z-10 mb-2 flex items-baseline gap-3 py-2 text-lg font-medium backdrop-blur">
            {reel.name}
            <span className="text-muted-foreground font-mono text-sm tabular-nums">
              {reel.clips.length} clips · {formatBytes(reel.stats.totalSizeBytes)}
            </span>
          </h3>
          <Card className="overflow-hidden py-0">
            <ul className="divide-y">
              {reel.clips.map((clip) => (
                <ClipRow key={clip.id} clipId={clip.id} />
              ))}
            </ul>
          </Card>
        </section>
      ))}

      <RawSection raw={model.raw} />
    </section>
  )
}
