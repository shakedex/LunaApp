import type { CoverFields } from '@luna-web/core'
import { Store } from '@tanstack/store'

// Deliberately separate from scanStore: a DIT types the cover once per day;
// "Start over" (resetScan) must never clobber it. Persisted cover defaults
// are seeded into this store at boot by hydrateSettings() (Plan 09).
// Seed the date so the form's displayed default and the exported model agree
// from the start — without this, model.cover.date is undefined unless the
// field is blurred, while the UI shows today (final-review finding, Plan 05).
export const coverStore = new Store<CoverFields<Blob>>({
  date: new Date().toISOString().slice(0, 10),
})

export function setCoverFields(patch: Partial<CoverFields<Blob>>): void {
  coverStore.setState((s) => ({ ...s, ...patch }))
}
