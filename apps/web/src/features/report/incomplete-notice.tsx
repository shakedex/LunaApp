import type { ReportStats, UnreadableFileRef } from '@luna-web/core'
import { TriangleAlert } from 'lucide-react'
import { Alert, AlertDescription, AlertTitle } from '@/components/ui/alert'

// Enough to recognise which files broke without turning a caveat into a list
// view. The full set, with reasons, is in /activity.
const MAX_LISTED = 5

const plural = (n: number, one: string, many: string) => `${n} ${n === 1 ? one : many}`

// Renders nothing for a complete report — the common case must add no chrome.
export function IncompleteNotice({
  stats,
  unreadableFiles,
}: {
  stats: ReportStats
  unreadableFiles: UnreadableFileRef[]
}) {
  const missing = stats.clipsMissingMetadataCount
  const unreadable = stats.unreadableFileCount
  if (missing === 0 && unreadable === 0) return null

  return (
    <Alert className="mb-6">
      {/* Not the destructive variant: an incomplete report is a caveat the
          reader must carry, not an error they can act on. */}
      <TriangleAlert className="text-warning" />
      <AlertTitle>This report is missing data</AlertTitle>
      <AlertDescription>
        <div className="space-y-1">
          {missing > 0 && (
            <div>
              {plural(missing, 'clip', 'clips')} missing metadata — duration not included in the
              totals.
            </div>
          )}
          {unreadable > 0 && (
            <div>
              {plural(unreadable, 'file', 'files')} could not be opened — not counted in the totals.
            </div>
          )}
          {unreadableFiles.length > 0 && (
            <ul className="text-muted-foreground-dim pt-1 font-mono text-xs">
              {unreadableFiles.slice(0, MAX_LISTED).map((f) => (
                <li key={f.relativePath} className="truncate" title={f.reason}>
                  {f.relativePath}
                </li>
              ))}
              {unreadableFiles.length > MAX_LISTED && (
                <li>+{unreadableFiles.length - MAX_LISTED} more</li>
              )}
            </ul>
          )}
        </div>
      </AlertDescription>
    </Alert>
  )
}
