import {
  normalizeReportSummaries,
  type ReportModel,
  type ReportSummary,
  summarizeReport,
} from '@luna-web/core'
import { getDb } from './db'

let persistenceRequested = false

// Saved reports are deliberate user data: ask the browser to protect this
// origin from storage-pressure eviction. Best-effort, once per session —
// Chromium grants silently based on engagement, no prompt.
function requestPersistence(): void {
  if (persistenceRequested) return
  persistenceRequested = true
  navigator.storage?.persist?.().catch(() => {})
}

export async function saveReport(model: ReportModel<Blob>): Promise<ReportSummary> {
  requestPersistence()
  const summary = summarizeReport(
    model,
    { id: crypto.randomUUID(), savedAt: Date.now() },
    (image) => image.size,
  )
  const db = await getDb()
  // One transaction over both stores: a summary must never exist without its
  // model (or vice versa) — quota failures abort both writes together.
  const tx = db.transaction(['reportSummaries', 'reportModels'], 'readwrite')
  void tx.objectStore('reportSummaries').put(summary, summary.id)
  void tx.objectStore('reportModels').put(model, summary.id)
  await tx.done
  return summary
}

export async function listReportSummaries(): Promise<ReportSummary[]> {
  const db = await getDb()
  return normalizeReportSummaries(await db.getAll('reportSummaries'))
}

function numberOr(value: unknown, fallback: number): number {
  return typeof value === 'number' && Number.isFinite(value) ? value : fallback
}

/** Boundary: this record is a structured clone of a ReportModel<Blob> WE wrote in
 *  saveReport, but any past app version may have written it — validate the coarse
 *  shape, default what a newer version added, then trust it. Pure and exported so
 *  the back-compat path is testable without IndexedDB; a real schemaVersion +
 *  migration is audit item 2.2. */
export function normalizeReportRecord(raw: unknown): ReportModel<Blob> | null {
  if (typeof raw !== 'object' || raw === null) return null
  const record = raw as Record<string, unknown>
  if (!Array.isArray(record.reels) || typeof record.stats !== 'object' || record.stats === null) {
    return null
  }
  const stats = record.stats as Record<string, unknown>
  const normalized = {
    ...record,
    unreadableEntries: Array.isArray(record.unreadableEntries) ? record.unreadableEntries : [],
    stats: {
      ...stats,
      unreadableFileCount: numberOr(stats.unreadableFileCount, 0),
      unreadableDirectoryCount: numberOr(stats.unreadableDirectoryCount, 0),
      clipsMissingMetadataCount: numberOr(stats.clipsMissingMetadataCount, 0),
    },
  }
  // Widened through `unknown`: `normalized` carries the raw record's index signature,
  // which does not overlap ReportModel structurally enough for a direct assertion.
  return normalized as unknown as ReportModel<Blob>
}

export async function loadReportModel(id: string): Promise<ReportModel<Blob> | null> {
  const db = await getDb()
  return normalizeReportRecord(await db.get('reportModels', id))
}

export async function deleteReport(id: string): Promise<void> {
  const db = await getDb()
  const tx = db.transaction(['reportSummaries', 'reportModels'], 'readwrite')
  void tx.objectStore('reportSummaries').delete(id)
  void tx.objectStore('reportModels').delete(id)
  await tx.done
}
