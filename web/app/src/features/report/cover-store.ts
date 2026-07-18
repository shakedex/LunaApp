import type { CoverFields } from '@luna-web/core'
import { Store } from '@tanstack/store'

// Deliberately separate from scanStore: a DIT types the cover once per day;
// "Start over" (resetScan) must never clobber it. Settings-persisted defaults
// arrive in Plan 08.
export const coverStore = new Store<CoverFields<Blob>>({})

export function setCoverFields(patch: Partial<CoverFields<Blob>>): void {
  coverStore.setState((s) => ({ ...s, ...patch }))
}
