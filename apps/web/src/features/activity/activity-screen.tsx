import { formatLogText, type LogLevel, logLevelAtLeast } from '@luna-web/core'
import { useSelector } from '@tanstack/react-store'
import { Download, Trash2 } from 'lucide-react'
import { useState } from 'react'
import { Button } from '@/components/ui/button'
import { saveBlob } from '@/features/export/save'
import { activityStore, clearActivity } from '@/lib/logger'

const FILTERS: ReadonlyArray<{ min: LogLevel; label: string }> = [
  { min: 'debug', label: 'All' },
  { min: 'info', label: 'Info' },
  { min: 'warn', label: 'Warnings' },
  { min: 'error', label: 'Errors' },
]

const LEVEL_CLASS: Record<LogLevel, string> = {
  debug: 'text-muted-foreground',
  info: 'text-foreground',
  warn: 'text-amber-500',
  error: 'text-destructive',
}

function timeOf(timestamp: number): string {
  return new Date(timestamp).toLocaleTimeString(undefined, { hour12: false })
}

export function ActivityScreen() {
  const entries = useSelector(activityStore, (s) => s.entries)
  const [minLevel, setMinLevel] = useState<LogLevel>('info')
  const visible = entries.filter((e) => logLevelAtLeast(e.level, minLevel))

  return (
    <div className="mx-auto flex max-w-3xl flex-col gap-4">
      <div className="flex flex-wrap items-center justify-between gap-3">
        <h1 className="text-2xl font-semibold tracking-tight">Activity</h1>
        <div className="flex items-center gap-2">
          {FILTERS.map((f) => (
            <Button
              key={f.min}
              variant={minLevel === f.min ? 'secondary' : 'ghost'}
              size="sm"
              onClick={() => setMinLevel(f.min)}
            >
              {f.label}
            </Button>
          ))}
          <Button
            variant="outline"
            size="sm"
            disabled={entries.length === 0}
            onClick={() =>
              void saveBlob(
                new Blob([formatLogText(entries)], { type: 'text/plain' }),
                `luna-activity-${new Date().toISOString().slice(0, 10)}.txt`,
                'text/plain',
              )
            }
          >
            <Download /> Download
          </Button>
          <Button
            variant="ghost"
            size="sm"
            disabled={entries.length === 0}
            onClick={clearActivity}
            aria-label="Clear activity log"
          >
            <Trash2 />
          </Button>
        </div>
      </div>

      {visible.length === 0 ? (
        <p className="text-muted-foreground py-12 text-center text-sm">
          {entries.length === 0
            ? 'Nothing logged yet — scan a folder and activity will show up here.'
            : 'No entries at this level.'}
        </p>
      ) : (
        <ol className="flex flex-col-reverse gap-1 font-mono text-xs">
          {visible.map((e) => (
            <li key={e.seq} className="rounded px-2 py-1 leading-relaxed">
              <span className="text-muted-foreground tabular-nums">{timeOf(e.timestamp)}</span>{' '}
              <span className={`${LEVEL_CLASS[e.level]} uppercase`}>{e.level}</span>{' '}
              <span>{e.message}</span>
              {e.detail !== undefined && (
                <div className="text-muted-foreground truncate pl-16" title={e.detail}>
                  {e.detail}
                </div>
              )}
            </li>
          ))}
        </ol>
      )}
    </div>
  )
}
