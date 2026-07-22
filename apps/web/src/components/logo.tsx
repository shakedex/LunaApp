import { cn } from '@/lib/utils'

export function Logo({ className }: { className?: string }) {
  return (
    <img
      src="/luna-logo-lg.webp"
      alt="Luna"
      width={317}
      height={333}
      loading="eager"
      decoding="async"
      // The glow scales with the mark, and the caller already sets the height —
      // so it's an override in the same class string rather than a second prop
      // that has to be kept in sync. twMerge keeps whichever drop-shadow wins.
      className={cn('drop-shadow-glow', className)}
    />
  )
}
