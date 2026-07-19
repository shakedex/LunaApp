import {
  type CoverFields,
  clampWorkerPoolCap,
  WORKER_POOL_CAP_MAX,
  WORKER_POOL_CAP_MIN,
} from '@luna-web/core'
import { useSelector } from '@tanstack/react-store'
import { Button } from '@/components/ui/button'
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '@/components/ui/card'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import { coverStore } from '@/features/report/cover-store'
import { settingsStore, updateSettings } from './settings-store'

// Persistable subset of the current cover: text fields that are non-empty,
// plus the logo. `date` is deliberately never saved (today-seeded each boot).
function coverDefaultsFrom(cover: CoverFields<Blob>): Partial<CoverFields<Blob>> {
  const out: Partial<CoverFields<Blob>> = {}
  for (const key of ['projectTitle', 'productionCompany', 'dit', 'director', 'dp'] as const) {
    const value = cover[key]
    if (typeof value === 'string' && value.trim() !== '') out[key] = value
  }
  if (cover.logo) out.logo = cover.logo
  return out
}

const DEFAULT_LABELS: ReadonlyArray<[keyof CoverFields<Blob> & string, string]> = [
  ['projectTitle', 'Project'],
  ['productionCompany', 'Production'],
  ['dit', 'DIT'],
  ['director', 'Director'],
  ['dp', 'DP'],
]

export function SettingsScreen() {
  const workerPoolCap = useSelector(settingsStore, (s) => s.workerPoolCap)
  const coverDefaults = useSelector(settingsStore, (s) => s.coverDefaults)
  const cores = navigator.hardwareConcurrency || 2

  const savedLines = DEFAULT_LABELS.filter(([key]) => typeof coverDefaults[key] === 'string')
  const hasDefaults = savedLines.length > 0 || coverDefaults.logo !== undefined

  return (
    <div className="mx-auto flex max-w-2xl flex-col gap-6">
      <h1 className="text-2xl font-semibold tracking-tight">Settings</h1>

      <Card>
        <CardHeader>
          <CardTitle>Processing</CardTitle>
          <CardDescription>
            Parallel workers for metadata and thumbnail decoding. Bounded by your CPU ({cores} cores
            detected) — higher is not always faster.
          </CardDescription>
        </CardHeader>
        <CardContent>
          <div className="flex items-center gap-3">
            <Label htmlFor="worker-cap" className="shrink-0">
              Worker cap
            </Label>
            <Input
              id="worker-cap"
              type="number"
              min={WORKER_POOL_CAP_MIN}
              max={WORKER_POOL_CAP_MAX}
              defaultValue={workerPoolCap}
              className="w-24"
              onBlur={(event) => {
                const next = clampWorkerPoolCap(event.currentTarget.valueAsNumber)
                event.currentTarget.value = String(next)
                if (next !== settingsStore.state.workerPoolCap)
                  void updateSettings({ workerPoolCap: next })
              }}
            />
            <span className="text-muted-foreground text-sm">
              {WORKER_POOL_CAP_MIN}–{WORKER_POOL_CAP_MAX}, applies to the next run
            </span>
          </div>
        </CardContent>
      </Card>

      <Card>
        <CardHeader>
          <CardTitle>Report defaults</CardTitle>
          <CardDescription>
            Cover fields pre-filled on every launch. The report date always defaults to today and is
            never saved.
          </CardDescription>
        </CardHeader>
        <CardContent className="flex flex-col gap-4">
          {hasDefaults ? (
            <dl className="grid grid-cols-[auto_1fr] gap-x-4 gap-y-1 text-sm">
              {savedLines.map(([key, label]) => (
                <div key={key} className="contents">
                  <dt className="text-muted-foreground">{label}</dt>
                  <dd className="truncate font-mono">{coverDefaults[key] as string}</dd>
                </div>
              ))}
              {coverDefaults.logo !== undefined && (
                <div className="contents">
                  <dt className="text-muted-foreground">Logo</dt>
                  <dd className="font-mono">saved</dd>
                </div>
              )}
            </dl>
          ) : (
            <p className="text-muted-foreground text-sm">No defaults saved yet.</p>
          )}
          <div className="flex items-center gap-2">
            <Button
              variant="outline"
              onClick={() =>
                void updateSettings({ coverDefaults: coverDefaultsFrom(coverStore.state) })
              }
            >
              Save current cover as defaults
            </Button>
            <Button
              variant="ghost"
              disabled={!hasDefaults}
              onClick={() => void updateSettings({ coverDefaults: {} })}
            >
              Clear defaults
            </Button>
          </div>
        </CardContent>
      </Card>
    </div>
  )
}
