import { createFileRoute } from '@tanstack/react-router'
import { ReportLibraryScreen } from '@/features/reports/report-library-screen'

export const Route = createFileRoute('/reports/')({
  component: ReportLibraryScreen,
})
