import { cn } from '@/lib/utils'

export function StatTile({
  label,
  value,
  unit,
  accent = false,
  dim = false,
  title,
}: {
  label: string
  value: string
  // Small trailing unit, dimmed and de-emphasized next to the value (e.g. "GB").
  unit?: string
  // Tint the value with the primary accent — used for the headline metric.
  accent?: boolean
  // Fade the value when it carries no weight (e.g. a zero count).
  dim?: boolean
  // Hover hint on the value — used to expose an exact byte count beside a
  // rounded one. Additive: never the only place a number is readable.
  title?: string
}) {
  return (
    <div className="flex flex-col gap-1">
      <span className="text-muted-foreground text-xs font-medium tracking-wide uppercase">
        {label}
      </span>
      <span
        className={cn(
          'font-mono text-2xl tabular-nums',
          accent && 'text-primary',
          dim && 'text-muted-foreground-dim',
        )}
        title={title}
      >
        {value}
        {unit && <span className="text-muted-foreground ml-1 text-base font-normal">{unit}</span>}
      </span>
    </div>
  )
}
