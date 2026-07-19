import { useSelector } from '@tanstack/react-store'
import { Button } from '@/components/ui/button'
import { Progress } from '@/components/ui/progress'
import { ClipTile } from './clip-tile'
import { RawSection } from './raw-section'
import { resetScan } from './run-scan'
import { scanStore } from './store'

export function ProcessingView() {
  const phase = useSelector(scanStore, (s) => s.phase)
  const clips = useSelector(scanStore, (s) => s.clips)
  const raw = useSelector(scanStore, (s) => s.raw)
  const processedCount = useSelector(scanStore, (s) => s.processedCount)
  const thumbDoneCount = useSelector(scanStore, (s) => s.thumbDoneCount)
  const thumbTotal = useSelector(scanStore, (s) => Object.keys(s.thumbStatus).length)

  const inThumb = phase === 'thumbnailing'
  const done = inThumb ? thumbDoneCount : processedCount
  const total = inThumb ? thumbTotal : clips.length
  const pct = total > 0 ? (done / total) * 100 : 0

  return (
    <section className="space-y-5">
      <div className="space-y-3">
        <div className="flex items-end justify-between gap-4">
          <div>
            <h2 className="text-lg font-semibold" aria-live="polite">
              {inThumb ? 'Generating thumbnails' : 'Reading metadata'}
            </h2>
            <p className="text-muted-foreground font-mono text-sm tabular-nums">
              {done} / {total} clips
            </p>
          </div>
          <Button variant="outline" size="sm" onClick={resetScan}>
            Start over
          </Button>
        </div>
        <Progress value={pct} />
      </div>

      <div className="grid grid-cols-2 gap-3 sm:grid-cols-3 lg:grid-cols-4">
        {clips.map((clip) => (
          <ClipTile key={clip.id} clip={clip} />
        ))}
      </div>

      <RawSection raw={raw} />
    </section>
  )
}
