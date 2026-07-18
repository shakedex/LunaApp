import type { ReportModel } from '@luna-web/core'
import { useState } from 'react'
import { Button } from '@/components/ui/button'
import { Spinner } from '@/components/ui/spinner'
import { exporters, runExport } from './exporter'
import './csv-exporter'
import './pdf-exporter'

export function ExportButtons({ report }: { report: ReportModel<Blob> }) {
  const [busy, setBusy] = useState<string | null>(null)
  const [error, setError] = useState<string | null>(null)

  return (
    <div className="flex items-center gap-2">
      {exporters.map((exporter) => (
        <Button
          key={exporter.id}
          variant={exporter.id === 'pdf' ? 'default' : 'outline'}
          disabled={busy !== null}
          onClick={() => {
            setError(null)
            setBusy(exporter.id)
            runExport(exporter, report)
              .catch((err) => setError(err instanceof Error ? err.message : String(err)))
              .finally(() => setBusy(null))
          }}
        >
          {busy === exporter.id ? (
            <>
              <Spinner /> Exporting…
            </>
          ) : (
            `Export ${exporter.label}`
          )}
        </Button>
      ))}
      {error && <span className="text-destructive text-sm">{error}</span>}
    </div>
  )
}
