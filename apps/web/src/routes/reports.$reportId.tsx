import { createFileRoute } from '@tanstack/react-router'

export const Route = createFileRoute('/reports/$reportId')({
  component: () => null, // Task 5 replaces this with SavedReportScreen
})
