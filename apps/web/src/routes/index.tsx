import { createFileRoute } from '@tanstack/react-router'
import { ScanScreen } from '@/features/scan/scan-screen'

export const Route = createFileRoute('/')({
  component: ScanScreen,
})
