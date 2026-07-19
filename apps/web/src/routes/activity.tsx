import { createFileRoute } from '@tanstack/react-router'
import { ActivityScreen } from '@/features/activity/activity-screen'

export const Route = createFileRoute('/activity')({
  component: ActivityScreen,
})
