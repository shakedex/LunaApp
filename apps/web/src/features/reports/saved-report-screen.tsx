import type { ReportModel } from '@luna-web/core'
import { Link } from '@tanstack/react-router'
import { useEffect, useState } from 'react'
import { buttonVariants } from '@/components/ui/button'
import { ExportButtons } from '@/features/export/export-buttons'
import { ReportView } from '@/features/report/report-view'
import { cn } from '@/lib/utils'
import { loadReportModel } from '@/persistence/report-library'

export function SavedReportScreen({ reportId }: { reportId: string }) {
  const [state, setState] = useState<
    { status: 'loading' } | { status: 'missing' } | { status: 'loaded'; model: ReportModel<Blob> }
  >({ status: 'loading' })

  useEffect(() => {
    let cancelled = false
    setState({ status: 'loading' })
    void loadReportModel(reportId).then((model) => {
      if (cancelled) return
      setState(model ? { status: 'loaded', model } : { status: 'missing' })
    })
    return () => {
      cancelled = true
    }
  }, [reportId])

  if (state.status === 'loading') return null

  if (state.status === 'missing') {
    return (
      <div className="flex flex-col items-center gap-4 py-24 text-center">
        <h1 className="text-2xl font-semibold tracking-tight">Report not found</h1>
        <p className="text-muted-foreground max-w-sm text-sm">
          This saved report doesn't exist anymore — it may have been deleted.
        </p>
        <Link to="/reports/" className={cn(buttonVariants({ variant: 'outline' }))}>
          Back to Reports
        </Link>
      </div>
    )
  }

  return (
    <ReportView
      model={state.model}
      eyebrow="Saved Report"
      actions={
        <>
          <ExportButtons report={state.model} />
          <Link to="/reports/" className={cn(buttonVariants({ variant: 'outline' }))}>
            Back to Reports
          </Link>
        </>
      }
    />
  )
}
