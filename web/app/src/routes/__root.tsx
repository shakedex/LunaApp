import { createRootRoute, Outlet } from '@tanstack/react-router'
import { AppShell } from '@/components/app-shell'
import { CapabilityGate } from '@/components/capability-gate'

export const Route = createRootRoute({
  component: () => (
    <CapabilityGate>
      <AppShell>
        <Outlet />
      </AppShell>
    </CapabilityGate>
  ),
})
