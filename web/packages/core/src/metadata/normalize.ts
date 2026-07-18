export function scaledNumber(v: unknown, divisor: number): number | undefined {
  let n: number | undefined
  if (typeof v === 'number') {
    n = Number.isFinite(v) ? v : undefined
  } else if (typeof v === 'string' && v.trim() !== '') {
    const parsed = Number(v)
    n = Number.isFinite(parsed) ? parsed : undefined
  }
  if (n === undefined) return undefined
  return n / divisor
}

export function degreesDisplay(v: unknown, divisor?: number): string | undefined {
  const num = scaledNumber(v, divisor ?? 1)
  if (num === undefined) return undefined
  return `${num}°`
}

export function kelvinDisplay(v: unknown): string | undefined {
  const num = scaledNumber(v, 1)
  if (num === undefined) return undefined
  return `${num} K`
}

export function tNumberDisplay(v: unknown): string | undefined {
  const num = scaledNumber(v, 1)
  if (num === undefined) return undefined
  return `T${num.toFixed(1)}`
}

export function fNumberDisplay(v: unknown): string | undefined {
  const num = scaledNumber(v, 1)
  if (num === undefined) return undefined
  return `F${num.toFixed(1)}`
}

// Sony/Canon acquisition tracks report focal length as a bare scaled number
// (e.g. "50.000") — append the unit. Leave any other shape (already-suffixed,
// non-numeric) untouched rather than guessing.
export function focalLengthDisplay(v: unknown): string | undefined {
  if (typeof v !== 'string') return undefined
  const trimmed = v.trim()
  if (trimmed === '') return undefined
  return /^\d+(\.\d+)?$/.test(trimmed) ? `${trimmed} mm` : trimmed
}
