import { cn } from '@/lib/utils'

// Clip formats cycle through the chart tokens; a typical card has 1–3 formats,
// so cycling past five is a graceful fallback rather than a real case.
const CLIP_COLORS = ['bg-chart-1', 'bg-chart-2', 'bg-chart-3', 'bg-chart-4', 'bg-chart-5'] as const

const OTHER_COLOR = 'bg-muted-foreground/40'

interface Segment {
  key: string
  label: string
  count: number
  color: string
}

// A glance at what the card is made of: clip formats by count (from the scan's
// byExtension, which counts clips only) plus a neutral segment for other files.
export function CompositionBar({
  byExtension,
  otherFileCount,
}: {
  byExtension: Record<string, number>
  otherFileCount: number
}) {
  const segments: Segment[] = Object.entries(byExtension)
    .filter(([, n]) => n > 0)
    .sort((a, b) => b[1] - a[1])
    .map(([ext, count], i) => ({
      key: ext,
      label: ext, // already lowercased with a leading dot, e.g. ".mov"
      count,
      color: CLIP_COLORS[i % CLIP_COLORS.length],
    }))

  if (otherFileCount > 0) {
    segments.push({
      key: '__other__',
      label: 'other files',
      count: otherFileCount,
      color: OTHER_COLOR,
    })
  }

  const total = segments.reduce((sum, s) => sum + s.count, 0)
  if (total === 0) return null

  return (
    <div className="space-y-2.5">
      <span className="text-muted-foreground text-xs font-medium tracking-wide uppercase">
        Composition
      </span>
      <div className="bg-muted flex h-2 gap-0.5 overflow-hidden rounded-full" aria-hidden="true">
        {segments.map((s) => (
          <div
            key={s.key}
            className={cn('h-full', s.color)}
            style={{ width: `${(s.count / total) * 100}%`, minWidth: '3px' }}
          />
        ))}
      </div>
      <ul className="flex flex-wrap gap-x-4 gap-y-1.5">
        {segments.map((s) => (
          <li
            key={s.key}
            className="text-foreground/80 flex items-center gap-1.5 font-mono text-xs"
          >
            <span className={cn('size-2 shrink-0 rounded-[2px]', s.color)} aria-hidden="true" />
            {s.label} <span className="text-muted-foreground">×{s.count}</span>
          </li>
        ))}
      </ul>
    </div>
  )
}
