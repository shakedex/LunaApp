import type { ReportStats, UnreadableEntryRef } from '@luna-web/core'
import { TriangleAlert } from 'lucide-react'
import { Alert, AlertDescription, AlertTitle } from '@/components/ui/alert'
import { reportShortfalls } from './shortfall-lines'

// Enough to recognise which entries broke without turning a caveat into a list
// view. The full set, with reasons, is in /activity.
const MAX_LISTED = 5

// Renders nothing for a complete report — the common case must add no chrome.
export function IncompleteNotice({
  stats,
  unreadableEntries,
}: {
  stats: ReportStats
  unreadableEntries: UnreadableEntryRef[]
}) {
  const lines = reportShortfalls(stats)
  if (lines.length === 0) return null

  return (
    <Alert className="mb-6">
      {/* Not the destructive variant: an incomplete report is a caveat the reader
          must carry, not an error they can act on. The `!` is load-bearing —
          Alert's own *:[svg]:text-current rule outranks a plain text-warning. */}
      <TriangleAlert className="text-warning!" />
      <AlertTitle>This report is missing data</AlertTitle>
      <AlertDescription>
        <div className="space-y-1">
          {lines.map((line) => (
            <div key={line}>{line}</div>
          ))}
          {unreadableEntries.length > 0 && (
            <ul className="text-muted-foreground-dim pt-1 font-mono text-xs">
              {unreadableEntries.slice(0, MAX_LISTED).map((e) => (
                <li key={e.relativePath} className="truncate" title={e.reason}>
                  {e.relativePath}
                  {e.kind === 'directory' && '/'}
                </li>
              ))}
              {unreadableEntries.length > MAX_LISTED && (
                <li>+{unreadableEntries.length - MAX_LISTED} more</li>
              )}
            </ul>
          )}
        </div>
      </AlertDescription>
    </Alert>
  )
}
