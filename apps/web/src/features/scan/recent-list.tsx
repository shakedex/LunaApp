import { FolderOpen, TriangleAlert, X } from 'lucide-react'
import { useEffect, useState } from 'react'
import { Button } from '@/components/ui/button'
import {
  forgetSource,
  listRecentSources,
  type StoredRecentSource,
} from '@/persistence/recent-sources'
import { pickAndScan, scanFrom } from './run-scan'

type Entry = { key: number } & StoredRecentSource

export function RecentList() {
  const [entries, setEntries] = useState<Entry[]>([])

  useEffect(() => {
    void listRecentSources().then(setEntries)
  }, [])

  if (entries.length === 0) return null

  return (
    <section className="w-full max-w-md text-left">
      <h2 className="text-muted-foreground mb-2 text-xs font-medium tracking-wide uppercase">
        Recent folders
      </h2>
      <ul className="flex flex-col gap-1.5">
        {entries.map((e) => (
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
                    className={e.stale ? 'text-muted-foreground truncate block' : 'truncate block'}
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
              <Button
                variant="ghost"
                size="icon-sm"
                aria-label={`Remove ${e.name} from recent folders`}
                onClick={() => {
                  void forgetSource(e.key).then(() => listRecentSources().then(setEntries))
                }}
              >
                <X />
              </Button>
            </div>
          </li>
        ))}
      </ul>
    </section>
  )
}
