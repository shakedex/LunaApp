import { createRootRoute, Link, Outlet } from '@tanstack/react-router'
import { AppShell } from '@/components/app-shell'
import { CapabilityGate } from '@/components/capability-gate'
import { buttonVariants } from '@/components/ui/button'
import { cn } from '@/lib/utils'

export const Route = createRootRoute({
  component: () => (
    <CapabilityGate>
      <AppShell>
        <Outlet />
      </AppShell>
    </CapabilityGate>
  ),
  notFoundComponent: () => (
    <div className="flex flex-col items-center gap-4 py-24 text-center">
      <p className="text-muted-foreground font-mono text-sm">404</p>
      <h1 className="text-2xl font-semibold tracking-tight">This page doesn't exist</h1>
      <Link to="/" className={cn(buttonVariants({ variant: 'outline' }))}>
        Back to Luna
      </Link>
    </div>
  ),
})
