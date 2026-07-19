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

  const model = useMemo(
    () =>
      buildReportModel<Blob>({
        clips,
        otherFiles,
        metadataById,
        thumbsById,
        cover,
        sourceRoot: sourceName ?? '',
      }),
    [clips, otherFiles, metadataById, thumbsById, cover, sourceName],
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
