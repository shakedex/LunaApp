import { AlertDialog as AlertDialogPrimitive } from '@base-ui/react/alert-dialog'
import type { ReportSummary } from '@luna-web/core'
import { Link } from '@tanstack/react-router'
import { Trash2 } from 'lucide-react'
import { useEffect, useState } from 'react'
import {
  AlertDialog,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
  AlertDialogTrigger,
} from '@/components/ui/alert-dialog'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { Empty, EmptyDescription, EmptyHeader, EmptyTitle } from '@/components/ui/empty'
import { Skeleton } from '@/components/ui/skeleton'
import { formatBytes } from '@/lib/format'
import { deleteReport, listReportSummaries } from '@/persistence/report-library'

function savedAtLabel(savedAt: number): string {
  return new Date(savedAt).toLocaleString(undefined, { hour12: false })
}

export function ReportLibraryScreen() {
  const [summaries, setSummaries] = useState<ReportSummary[] | null>(null)
  const [usage, setUsage] = useState<string | null>(null)

  useEffect(() => {
    void listReportSummaries().then(setSummaries)
    void navigator.storage?.estimate?.().then((estimate) => {
      if (estimate?.usage !== undefined && estimate.quota !== undefined) {
        setUsage(`${formatBytes(estimate.usage)} used of ${formatBytes(estimate.quota)} available`)
      }
    })
  }, [])

  return (
    <div className="mx-auto flex max-w-3xl flex-col gap-4">
      <div className="flex flex-wrap items-baseline justify-between gap-3">
        <h1 className="text-2xl font-semibold tracking-tight">Report Library</h1>
        {usage && (
          <span className="text-muted-foreground font-mono text-xs tabular-nums">{usage}</span>
        )}
      </div>

      {summaries === null ? (
        <div className="flex flex-col gap-2" aria-hidden>
          {[0, 1, 2].map((i) => (
            <div
              key={i}
              className="bg-card flex items-center justify-between gap-3 rounded-lg border px-4 py-3"
            >
              <div className="min-w-0 flex-1 space-y-2">
                <Skeleton className="h-4 w-48" />
                <Skeleton className="h-3 w-72" />
              </div>
              <Skeleton className="h-5 w-28" />
            </div>
          ))}
        </div>
      ) : summaries.length === 0 ? (
        <Empty className="py-12">
          <EmptyHeader>
            <EmptyTitle>No saved reports yet</EmptyTitle>
            <EmptyDescription>Finish a run and use "Save report" to keep it here.</EmptyDescription>
          </EmptyHeader>
        </Empty>
      ) : (
        <ul className="flex flex-col gap-2">
          {summaries.map((s) => (
            <li key={s.id}>
              <div className="bg-card hover:border-input flex items-center justify-between gap-3 rounded-lg border px-4 py-3 transition-colors">
                <Link
                  to="/reports/$reportId/"
                  params={{ reportId: s.id }}
                  className="min-w-0 flex-1"
                >
                  <span className="block truncate font-medium">{s.title}</span>
                  <span className="text-muted-foreground block truncate font-mono text-xs tabular-nums">
                    {s.sourceRoot || '—'} · {s.clipCount} clips · {formatBytes(s.totalSizeBytes)} ·
                    saved {savedAtLabel(s.savedAt)}
                  </span>
                </Link>
                <div className="flex shrink-0 items-center gap-2">
                  <Badge variant={s.hasThumbnails ? 'secondary' : 'outline'}>
                    {s.hasThumbnails
                      ? `Thumbnails · ${formatBytes(s.storedFrameBytes)}`
                      : 'Data only'}
                  </Badge>
                  <AlertDialog>
                    <AlertDialogTrigger
                      render={
                        <Button
                          variant="ghost"
                          size="icon-sm"
                          aria-label={`Delete saved report ${s.title}`}
                        />
                      }
                    >
                      <Trash2 />
                    </AlertDialogTrigger>
                    <AlertDialogContent>
                      <AlertDialogHeader>
                        <AlertDialogTitle>Delete "{s.title}"?</AlertDialogTitle>
                        <AlertDialogDescription>
                          Removes this saved report
                          {s.hasThumbnails ? ' and its stored thumbnails' : ''} from this device.
                          This can't be undone.
                        </AlertDialogDescription>
                      </AlertDialogHeader>
                      <AlertDialogFooter>
                        <AlertDialogCancel>Cancel</AlertDialogCancel>
                        <AlertDialogPrimitive.Close
                          data-slot="alert-dialog-action"
                          render={<Button variant="destructive" />}
                          onClick={() => {
                            void deleteReport(s.id).then(() =>
                              listReportSummaries().then(setSummaries),
                            )
                          }}
                        >
                          Delete
                        </AlertDialogPrimitive.Close>
                      </AlertDialogFooter>
                    </AlertDialogContent>
                  </AlertDialog>
                </div>
              </div>
            </li>
          ))}
        </ul>
      )}
    </div>
  )
}
