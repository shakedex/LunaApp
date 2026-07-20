import type { ReportModel } from '@luna-web/core'
import { Link } from '@tanstack/react-router'
import { useEffect, useState } from 'react'
import { buttonVariants } from '@/components/ui/button'
import {
  Empty,
  EmptyContent,
  EmptyDescription,
  EmptyHeader,
  EmptyTitle,
} from '@/components/ui/empty'
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
      <Empty className="py-24">
        <EmptyHeader>
          <EmptyTitle className="text-2xl">Report not found</EmptyTitle>
          <EmptyDescription>
            This saved report doesn't exist anymore — it may have been deleted.
          </EmptyDescription>
        </EmptyHeader>
        <EmptyContent>
          <Link to="/reports/" className={cn(buttonVariants({ variant: 'outline' }))}>
            Back to Reports
          </Link>
        </EmptyContent>
      </Empty>
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
