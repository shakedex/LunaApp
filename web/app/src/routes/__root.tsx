import { createRootRoute, Outlet } from '@tanstack/react-router'
import { CapabilityGate } from '@/components/capability-gate'

export const Route = createRootRoute({
  component: () => (
    <CapabilityGate>
      <Outlet />
    </CapabilityGate>
  ),
})
