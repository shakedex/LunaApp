import { createRootRoute, Link, Outlet } from '@tanstack/react-router'
import { AppShell } from '@/components/app-shell'
import { CapabilityGate } from '@/components/capability-gate'
import { Logo } from '@/components/logo'
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
    <div className="flex min-h-[50dvh] flex-col items-center justify-center gap-6 text-center">
      <Logo className="drop-shadow-glow-lg h-14 w-auto opacity-80" />
      <div className="space-y-2">
        <p className="text-primary font-mono text-sm tracking-[0.3em] uppercase">404</p>
        <h1 className="text-3xl font-semibold tracking-tight">This reel isn't on the card</h1>
        <p className="text-muted-foreground mx-auto max-w-sm text-sm leading-relaxed">
          The page you're looking for doesn't exist — it may have been moved, renamed, or never shot
          in the first place.
        </p>
      </div>
      <div className="flex items-center gap-2">
        <Link to="/" className={cn(buttonVariants({ variant: 'default' }))}>
          Back to Luna
        </Link>
        <a href="/docs/" className={cn(buttonVariants({ variant: 'outline' }))}>
          Docs
        </a>
      </div>
    </div>
  ),
})
