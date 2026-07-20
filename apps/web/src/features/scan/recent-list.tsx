import type { ReportSummary } from '@luna-web/core'
import { Link } from '@tanstack/react-router'
import { FileText, FolderOpen, RefreshCw, TriangleAlert, X } from 'lucide-react'
import { useEffect, useState } from 'react'
import { Button, buttonVariants } from '@/components/ui/button'
import { Tooltip, TooltipContent, TooltipTrigger } from '@/components/ui/tooltip'
import { cn } from '@/lib/utils'
import {
  forgetSource,
  listRecentSources,
  type StoredRecentSource,
} from '@/persistence/recent-sources'
import { listReportSummaries } from '@/persistence/report-library'
import { pickAndScan, scanFrom } from './run-scan'

type Entry = { key: number } & StoredRecentSource

export function RecentList() {
  const [entries, setEntries] = useState<Entry[]>([])
  const [reports, setReports] = useState<ReportSummary[]>([])

  useEffect(() => {
    void listRecentSources().then(setEntries)
    void listReportSummaries().then(setReports)
  }, [])

  const savedReportFor = (name: string): ReportSummary | undefined =>
    reports.filter((r) => r.sourceRoot === name).sort((a, b) => b.savedAt - a.savedAt)[0]

  if (entries.length === 0) return null

  return (
    <section className="w-full max-w-md text-left">
      <h2 className="text-muted-foreground mb-2 text-xs font-medium tracking-wide uppercase">
        Recent folders
      </h2>
      <ul className="flex flex-col gap-1.5">
        {entries.map((e) => {
          const saved = savedReportFor(e.name)
          return (
            <li key={e.key}>
              <div className="bg-card hover:border-input flex items-center justify-between rounded-lg border px-4 py-3 transition-colors">
                <button
                  type="button"
                  className="flex min-w-0 items-center gap-2 rounded text-left text-sm outline-none focus-visible:ring-3 focus-visible:ring-ring/50"
                  onClick={() => void (e.stale ? pickAndScan() : scanFrom(e.handle, e.key))}
                >
                  {e.stale ? (
                    <TriangleAlert className="size-4 shrink-0 text-warning" />
                  ) : (
                    <FolderOpen className="text-muted-foreground size-4 shrink-0" />
                  )}
                  <span className="min-w-0">
                    <span
                      className={
                        e.stale ? 'text-muted-foreground truncate block' : 'truncate block'
                      }
                    >
                      {e.name}
                    </span>
                    {e.stale && (
                      <span className="text-muted-foreground block text-xs">
                        Folder unavailable — click to pick again
                      </span>
                    )}
                  </span>
                </button>
                <div className="flex shrink-0 items-center gap-1">
                  <Tooltip>
                    <TooltipTrigger
                      render={
                        <Button
                          variant="ghost"
                          size="icon-sm"
                          aria-label={`Scan ${e.name} again`}
                          onClick={() => void (e.stale ? pickAndScan() : scanFrom(e.handle, e.key))}
                        />
                      }
                    >
                      <RefreshCw />
                    </TooltipTrigger>
                    <TooltipContent>Scan again</TooltipContent>
                  </Tooltip>

                  {saved && (
                    <Tooltip>
                      <TooltipTrigger
                        render={
                          <Link
                            to="/reports/$reportId/"
                            params={{ reportId: saved.id }}
                            aria-label={`Open saved report for ${e.name}`}
                            className={cn(buttonVariants({ variant: 'ghost', size: 'icon-sm' }))}
                          />
                        }
                      >
                        <FileText />
                      </TooltipTrigger>
                      <TooltipContent>Open saved report</TooltipContent>
                    </Tooltip>
                  )}

                  <Tooltip>
                    <TooltipTrigger
                      render={
                        <Button
                          variant="ghost"
                          size="icon-sm"
                          aria-label={`Remove ${e.name} from recent folders`}
                          onClick={() => {
                            void forgetSource(e.key).then(() =>
                              listRecentSources().then(setEntries),
                            )
                          }}
                        />
                      }
                    >
                      <X />
                    </TooltipTrigger>
                    <TooltipContent>Remove from recent</TooltipContent>
                  </Tooltip>
                </div>
              </div>
            </li>
          )
        })}
      </ul>
    </section>
  )
}
