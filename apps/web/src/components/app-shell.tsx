import { Link } from '@tanstack/react-router'
import type { ReactNode } from 'react'
import { Logo } from '@/components/logo'
import { buttonVariants } from '@/components/ui/button'
import { cn } from '@/lib/utils'

export function AppShell({ children }: { children: ReactNode }) {
  return (
    <div className="flex min-h-dvh flex-col">
      <header className="bg-background/80 sticky top-0 z-40 border-b backdrop-blur">
        <div className="mx-auto flex h-14 max-w-5xl items-center justify-between px-6">
          <a href="/" className="flex items-center gap-2">
            <Logo className="h-7 w-auto" />
            <span className="text-base font-semibold tracking-tight">Luna</span>
          </a>
          <nav className="text-muted-foreground flex items-center gap-2 text-sm">
            <Link to="/reports/" className={cn(buttonVariants({ variant: 'ghost', size: 'sm' }))}>
              Reports
            </Link>
            <Link to="/settings/" className={cn(buttonVariants({ variant: 'ghost', size: 'sm' }))}>
              Settings
            </Link>
            <Link to="/activity/" className={cn(buttonVariants({ variant: 'ghost', size: 'sm' }))}>
              Activity
            </Link>
            <a href="/docs/" className={cn(buttonVariants({ variant: 'ghost', size: 'sm' }))}>
              Docs
            </a>
            <Link to="/credits/" className="hover:text-foreground tabular-nums transition-colors">
              v{__APP_VERSION__}
            </Link>
          </nav>
        </div>
      </header>
      <main className="mx-auto w-full max-w-5xl flex-1 px-6 py-10">{children}</main>
    </div>
  )
}
