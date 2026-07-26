const UNITS = ['B', 'KB', 'MB', 'GB', 'TB'] as const

export function formatBytes(n: number): string {
  if (n < 1000) return `${n} B`
  let value = n
  let unit = 0
  while (value >= 1000 && unit < UNITS.length - 1) {
    value /= 1000
    unit += 1
  }
  return `${value.toFixed(1)} ${UNITS[unit]}`
}

export function formatDuration(seconds: number): string {
  const total = Math.round(seconds)
  const h = Math.floor(total / 3600)
  const m = Math.floor((total % 3600) / 60)
  const s = total % 60
  const mm = String(m).padStart(2, '0')
  const ss = String(s).padStart(2, '0')
  return h > 0 ? `${h}:${mm}:${ss}` : `${m}:${ss}`
}

/** Today as YYYY-MM-DD in local time — the cover-date default and export-filename stamp. */
export function todayIso(): string {
  const now = new Date()
  const mm = String(now.getMonth() + 1).padStart(2, '0')
  const dd = String(now.getDate()).padStart(2, '0')
  return `${now.getFullYear()}-${mm}-${dd}`
}

/** The exact byte count, digit-grouped — the one size figure Windows Explorer,
 *  macOS Finder and Luna all agree on. Shown beside a formatted total, never
 *  instead of one. Grouping is pinned to en-US so two machines reading the same
 *  report see the same string. */
export function formatExactBytes(n: number): string {
  return `${n.toLocaleString('en-US')} ${n === 1 ? 'byte' : 'bytes'}`
}
