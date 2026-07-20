import {
  formatLogText,
  GENERAL_OPERATION,
  groupLogByOperation,
  type LogLevel,
  logLevelAtLeast,
  type OperationGroup,
} from '@luna-web/core'
import { useSelector } from '@tanstack/react-store'
import { ChevronDown, Download, Trash2 } from 'lucide-react'
import { useState } from 'react'
import { Button } from '@/components/ui/button'
import { Empty, EmptyDescription, EmptyHeader, EmptyTitle } from '@/components/ui/empty'
import { ToggleGroup, ToggleGroupItem } from '@/components/ui/toggle-group'
import { saveBlob } from '@/features/export/save'
import { todayIso } from '@/lib/format'
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
  warn: 'text-warning',
  error: 'text-destructive',
}

function timeOf(timestamp: number): string {
  return new Date(timestamp).toLocaleTimeString(undefined, { hour12: false })
}

function operationTitle(group: OperationGroup): string {
  if (group.operation.id === GENERAL_OPERATION.id) return GENERAL_OPERATION.label
  return `${group.operation.label} — ${new Date(group.operation.startedAt).toLocaleString(undefined, { hour12: false })}`
}

// Download text: operation headings above their entries, newest operation first.
function downloadText(groups: OperationGroup[]): string {
  return groups
    .map(
      (g) => `${'='.repeat(4)} ${operationTitle(g)} ${'='.repeat(4)}\n${formatLogText(g.entries)}`,
    )
    .join('\n\n')
}

export function ActivityScreen() {
  const snapshot = useSelector(activityStore, (s) => s)
  const [minLevel, setMinLevel] = useState<LogLevel>('info')
  const [collapsed, setCollapsed] = useState<ReadonlySet<number>>(new Set())

  const allGroups = groupLogByOperation(snapshot.entries, snapshot.operations)
  const groups = allGroups
    .map((g) => ({ ...g, entries: g.entries.filter((e) => logLevelAtLeast(e.level, minLevel)) }))
    .filter((g) => g.entries.length > 0)

  const toggle = (id: number) =>
    setCollapsed((prev) => {
      const next = new Set(prev)
      if (next.has(id)) next.delete(id)
      else next.add(id)
      return next
    })

  return (
    <div className="mx-auto flex max-w-3xl flex-col gap-4">
      <div className="flex flex-wrap items-center justify-between gap-3">
        <h1 className="text-2xl font-semibold tracking-tight">Activity</h1>
        <div className="flex items-center gap-2">
          <ToggleGroup
            value={[minLevel]}
            onValueChange={(v) => {
              if (v.length > 0) setMinLevel(v[0] as LogLevel)
            }}
            aria-label="Minimum log level"
            size="sm"
          >
            {FILTERS.map((f) => (
              <ToggleGroupItem key={f.min} value={f.min}>
                {f.label}
              </ToggleGroupItem>
            ))}
          </ToggleGroup>
          <Button
            variant="outline"
            size="sm"
            disabled={snapshot.entries.length === 0}
            onClick={() =>
              void saveBlob(
                new Blob([downloadText(allGroups)], { type: 'text/plain' }),
                `luna-activity-${todayIso()}.txt`,
                'text/plain',
              )
            }
          >
            <Download /> Download
          </Button>
          <Button
            variant="ghost"
            size="sm"
            disabled={snapshot.entries.length === 0}
            onClick={clearActivity}
            aria-label="Clear activity log"
          >
            <Trash2 />
          </Button>
        </div>
      </div>

      {groups.length === 0 ? (
        <Empty className="py-12">
          <EmptyHeader>
            <EmptyTitle>
              {snapshot.entries.length === 0 ? 'Nothing logged yet' : 'No entries at this level'}
            </EmptyTitle>
            {snapshot.entries.length === 0 && (
              <EmptyDescription>Scan a folder and activity will show up here.</EmptyDescription>
            )}
          </EmptyHeader>
        </Empty>
      ) : (
        <div className="flex flex-col gap-3">
          {groups.map((group) => {
            const isCollapsed = collapsed.has(group.operation.id)
            return (
              <section key={group.operation.id} className="bg-card rounded-lg border">
                <button
                  type="button"
                  className="flex w-full items-center justify-between gap-2 px-3 py-2 text-left text-sm font-medium rounded-lg outline-none focus-visible:ring-3 focus-visible:ring-ring/50"
                  onClick={() => toggle(group.operation.id)}
                  aria-expanded={!isCollapsed}
                >
                  <span className="truncate">{operationTitle(group)}</span>
                  <span className="text-muted-foreground flex shrink-0 items-center gap-2 text-xs">
                    {group.entries.length} entries
                    <ChevronDown
                      className={`size-4 transition-transform ${isCollapsed ? '-rotate-90' : ''}`}
                    />
                  </span>
                </button>
                {!isCollapsed && (
                  <ol className="flex flex-col gap-1 border-t px-2 py-2 font-mono text-xs">
                    {group.entries.map((e) => (
                      <li key={e.seq} className="rounded px-2 py-0.5 leading-relaxed">
                        <span className="text-muted-foreground tabular-nums">
                          {timeOf(e.timestamp)}
                        </span>{' '}
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
              </section>
            )
          })}
        </div>
      )}
    </div>
  )
}
