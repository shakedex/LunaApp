import { buildReportModel } from '@luna-web/core'
import { useSelector } from '@tanstack/react-store'
import { useMemo } from 'react'
import { Button } from '@/components/ui/button'
import { ExportButtons } from '@/features/export/export-buttons'
import { resetScan } from '@/features/scan/run-scan'
import { scanStore } from '@/features/scan/store'
import { CoverForm } from './cover-form'
import { coverStore } from './cover-store'
import { ReportView } from './report-view'
import { SaveReportButton } from './save-report-button'

export function ReportWorkspace() {
  const clips = useSelector(scanStore, (s) => s.clips)
  const otherFiles = useSelector(scanStore, (s) => s.otherFiles)
  const metadataById = useSelector(scanStore, (s) => s.metadataById)
  const thumbsById = useSelector(scanStore, (s) => s.thumbsById)
  const sourceName = useSelector(scanStore, (s) => s.sourceName)
  const cover = useSelector(coverStore)
  const unreadable = useSelector(scanStore, (s) => s.unreadable)
  // METADATA failures only. thumbErrors is deliberately not used: a clip we
  // could not make a thumbnail for is not a failed clip (2026-07-21 ruling).
  const clipErrors = useSelector(scanStore, (s) => s.clipErrors)

  const model = useMemo(
    () =>
      buildReportModel<Blob>({
        clips,
        otherFiles,
        metadataById,
        thumbsById,
        cover,
        sourceRoot: sourceName ?? '',
        unreadable,
        failedClipIds: Object.keys(clipErrors),
      }),
    [clips, otherFiles, metadataById, thumbsById, cover, sourceName, unreadable, clipErrors],
  )

  return (
    <ReportView
      model={model}
      actions={
        <>
          <ExportButtons report={model} />
          <SaveReportButton model={model} />
          <Button variant="outline" onClick={resetScan}>
            Start over
          </Button>
        </>
      }
    >
      <div className="mb-6">
        <CoverForm />
      </div>
    </ReportView>
  )
}
