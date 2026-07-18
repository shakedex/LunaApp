import { useEffect, useState } from 'react'
import { Button } from '@/components/ui/button'
import {
  forgetSource,
  listRecentSources,
  type StoredRecentSource,
} from '@/persistence/recent-sources'
import { scanFrom } from './run-scan'

type Entry = { key: number } & StoredRecentSource

export function RecentList() {
  const [entries, setEntries] = useState<Entry[]>([])

  useEffect(() => {
    void listRecentSources().then(setEntries)
  }, [])

  if (entries.length === 0) return null

  return (
    <section className="w-full max-w-md">
      <h2 className="text-muted-foreground mb-2 text-sm font-medium">Recent folders</h2>
      <ul className="divide-y rounded-lg border">
        {entries.map((e) => (
          <li key={e.key} className="flex items-center justify-between px-4 py-2">
            <button
              type="button"
              className="truncate text-left text-sm hover:underline"
              onClick={() => void scanFrom(e.handle)}
            >
              {e.name}
            </button>
            <Button
              variant="ghost"
              size="sm"
              aria-label={`Remove ${e.name} from recent folders`}
              onClick={() => {
                void forgetSource(e.key).then(() => listRecentSources().then(setEntries))
              }}
            >
              ✕
            </Button>
          </li>
        ))}
      </ul>
    </section>
  )
}
