import { createFileRoute } from '@tanstack/react-router'
import { SavedReportScreen } from '@/features/reports/saved-report-screen'

export const Route = createFileRoute('/reports/$reportId')({
  component: RouteComponent,
})

function RouteComponent() {
  const { reportId } = Route.useParams()
  return <SavedReportScreen reportId={reportId} />
}
