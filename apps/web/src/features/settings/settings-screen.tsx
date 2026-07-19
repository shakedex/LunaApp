import {
  type CoverFields,
  clampWorkerPoolCap,
  WORKER_POOL_CAP_MAX,
  WORKER_POOL_CAP_MIN,
} from '@luna-web/core'
import { useSelector } from '@tanstack/react-store'
import { useState } from 'react'
import { Button } from '@/components/ui/button'
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '@/components/ui/card'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import { coverStore } from '@/features/report/cover-store'
import { clearLocalData } from '@/persistence/clear'
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

const DEFAULT_FIELDS: ReadonlyArray<{
  key: 'projectTitle' | 'productionCompany' | 'dit' | 'director' | 'dp'
  label: string
}> = [
  { key: 'projectTitle', label: 'Project' },
  { key: 'productionCompany', label: 'Production company' },
  { key: 'dit', label: 'DIT' },
  { key: 'director', label: 'Director' },
  { key: 'dp', label: 'DP' },
]

export function SettingsScreen() {
  const workerPoolCap = useSelector(settingsStore, (s) => s.workerPoolCap)
  const generateThumbnails = useSelector(settingsStore, (s) => s.generateThumbnails)
  const coverDefaults = useSelector(settingsStore, (s) => s.coverDefaults)
  const [confirmingClear, setConfirmingClear] = useState(false)
  const cores = navigator.hardwareConcurrency || 2

  const patchDefault = (key: (typeof DEFAULT_FIELDS)[number]['key'], value: string) => {
    const next = { ...settingsStore.state.coverDefaults }
    if (value.trim() === '') delete next[key]
    else next[key] = value
    void updateSettings({ coverDefaults: next })
  }

  return (
    <div className="mx-auto flex max-w-2xl flex-col gap-6">
      <h1 className="text-2xl font-semibold tracking-tight">Settings</h1>

      <Card>
        <CardHeader>
          <CardTitle>Processing</CardTitle>
          <CardDescription>
            Applies to the next run. Worker count is bounded by your CPU ({cores} cores detected).
          </CardDescription>
        </CardHeader>
        <CardContent className="flex flex-col gap-4">
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
              {WORKER_POOL_CAP_MIN}–{WORKER_POOL_CAP_MAX}
            </span>
          </div>
          <label className="text-muted-foreground flex w-fit cursor-pointer items-center gap-2 text-sm">
            <input
              type="checkbox"
              className="accent-primary size-4"
              checked={generateThumbnails}
              onChange={(e) => void updateSettings({ generateThumbnails: e.currentTarget.checked })}
            />
            Generate thumbnails by default (override per run on the scan summary)
          </label>
        </CardContent>
      </Card>

      <Card>
        <CardHeader>
          <CardTitle>Report defaults</CardTitle>
          <CardDescription>
            Pre-filled into the cover on every launch. The report date always defaults to today and
            is never saved. Edits save as you leave each field.
          </CardDescription>
        </CardHeader>
        <CardContent className="flex flex-col gap-4">
          <div className="grid grid-cols-1 gap-3 sm:grid-cols-2">
            {DEFAULT_FIELDS.map(({ key, label }) => (
              <div key={key} className="flex flex-col gap-1.5">
                <Label htmlFor={`default-${key}`}>{label}</Label>
                <Input
                  id={`default-${key}`}
                  defaultValue={coverDefaults[key] ?? ''}
                  onBlur={(event) => {
                    if ((coverDefaults[key] ?? '') !== event.currentTarget.value)
                      patchDefault(key, event.currentTarget.value)
                  }}
                />
              </div>
            ))}
            <div className="flex flex-col gap-1.5">
              <Label htmlFor="default-logo">Logo</Label>
              <div className="flex items-center gap-2">
                <Input
                  id="default-logo"
                  type="file"
                  accept="image/*"
                  className="max-w-56"
                  onChange={(event) => {
                    const file = event.currentTarget.files?.[0]
                    if (file)
                      void updateSettings({
                        coverDefaults: { ...settingsStore.state.coverDefaults, logo: file },
                      })
                  }}
                />
                {coverDefaults.logo !== undefined && (
                  <Button
                    variant="ghost"
                    size="sm"
                    onClick={() => {
                      const { logo: _drop, ...rest } = settingsStore.state.coverDefaults
                      void updateSettings({ coverDefaults: rest })
                    }}
                  >
                    Remove
                  </Button>
                )}
              </div>
            </div>
          </div>
          <div className="flex items-center gap-2">
            <Button
              variant="outline"
              size="sm"
              onClick={() =>
                void updateSettings({ coverDefaults: coverDefaultsFrom(coverStore.state) })
              }
            >
              Copy from current cover
            </Button>
            <Button
              variant="ghost"
              size="sm"
              disabled={Object.keys(coverDefaults).length === 0}
              onClick={() => void updateSettings({ coverDefaults: {} })}
            >
              Clear defaults
            </Button>
          </div>
        </CardContent>
      </Card>

      <Card>
        <CardHeader>
          <CardTitle>Local data</CardTitle>
          <CardDescription>
            Luna stores settings, report defaults, recent folders, the activity log, and the cached
            decode engine on this device. Footage is never stored.
          </CardDescription>
        </CardHeader>
        <CardContent>
          {confirmingClear ? (
            <div className="flex flex-wrap items-center gap-2">
              <span className="text-destructive text-sm">
                Delete all locally stored Luna data and reload?
              </span>
              <Button variant="destructive" size="sm" onClick={() => void clearLocalData()}>
                Delete everything
              </Button>
              <Button variant="ghost" size="sm" onClick={() => setConfirmingClear(false)}>
                Cancel
              </Button>
            </div>
          ) : (
            <Button variant="outline" size="sm" onClick={() => setConfirmingClear(true)}>
              Clear local data…
            </Button>
          )}
        </CardContent>
      </Card>
    </div>
  )
}
