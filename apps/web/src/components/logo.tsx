export function Logo({ className }: { className?: string }) {
  return (
    <img
      src="/luna-logo-lg.webp"
      alt="Luna"
      width={317}
      height={333}
      loading="eager"
      decoding="async"
      className={className}
    />
  )
}
