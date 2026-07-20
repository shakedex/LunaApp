import type { ReportModel } from '@luna-web/core'
import { errorMessage } from '@/lib/errors'
import { beginOperation, logger } from '@/lib/logger'
import { reportFileName, saveBlob } from './save'

export interface Exporter {
  id: string
  label: string
  extension: string // 'pdf' | 'csv' (no dot)
  mime: string
  generate(report: ReportModel<Blob>): Promise<Blob>
}

// Ordered registry — mirrors the desktop's ICameraSupport modularity (spec §8.9).
// csv registers below; pdf registers in pdf-exporter.ts (Task 5).
export const exporters: Exporter[] = []

export async function runExport(exporter: Exporter, report: ReportModel<Blob>): Promise<void> {
  beginOperation('export', `Export ${exporter.label}: ${report.cover.projectTitle ?? 'report'}`)
  logger.info(`Export started: ${exporter.label}`)
  try {
    const blob = await exporter.generate(report)
    await saveBlob(
      blob,
      reportFileName(report.cover.projectTitle, report.cover.date, exporter.extension),
      exporter.mime,
    )
    logger.info(`Export finished: ${exporter.label}`)
  } catch (err) {
    logger.error(`Export failed: ${exporter.label}`, errorMessage(err))
    throw err
  }
}
