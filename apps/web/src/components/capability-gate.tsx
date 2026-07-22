import { type ReactNode, useMemo } from 'react'
import { Logo } from '@/components/logo'
import { Badge } from '@/components/ui/badge'
import { buttonVariants } from '@/components/ui/button'
import { blockReason, detectCapabilities } from '@/lib/capabilities'
import { LUNA_URL } from '@/lib/site'
import { cn } from '@/lib/utils'

const CHROMIUM_BROWSERS = ['Chrome', 'Edge', 'Brave', 'Arc', 'Opera', 'Vivaldi']

export function CapabilityGate({ children }: { children: ReactNode }) {
  const reason = useMemo(() => blockReason(detectCapabilities()), [])

  if (reason === null) return <>{children}</>

  return reason === 'mobile' ? <MobileScreen /> : <BrowserScreen />
}

function MobileScreen() {
  return (
    <Screen
      eyebrow="Desktop only"
      title="Luna doesn't run on phones or tablets"
      body="Open Luna on a Mac or PC, in Chrome, Edge, Brave, or another Chromium browser."
      cta="Why not mobile?"
    >
      <div className="text-muted-foreground flex flex-col items-center gap-1.5">
        <span className="text-2xs tracking-widest uppercase">On your computer, open</span>
        {/* Selectable, not a link — tapping it here would just reload this same screen. */}
        <span className="border-border bg-card text-foreground rounded-md border px-3 py-1.5 font-mono text-sm select-all">
          {LUNA_URL.replace(/^https:\/\//, '')}
        </span>
      </div>
    </Screen>
  )
}

function BrowserScreen() {
  return (
    <Screen
      eyebrow="Unsupported browser"
      title="Luna needs a Chromium browser"
      body="Luna reads your card straight off the disk, which only Chromium browsers can do."
      cta="Why Chromium?"
    >
      <div className="flex flex-wrap justify-center gap-1.5">
        {CHROMIUM_BROWSERS.map((name) => (
          <Badge key={name} variant="secondary" className="px-2.5">
            {name}
          </Badge>
        ))}
      </div>
    </Screen>
  )
}

function Screen({
  eyebrow,
  title,
  body,
  cta,
  children,
}: {
  eyebrow: string
  title: string
  body: string
  cta: string
  children: ReactNode
}) {
  return (
    <main className="landing-atmosphere flex min-h-dvh flex-col items-center justify-center gap-7 px-6 py-16 text-center">
      <Logo className="drop-shadow-glow-lg h-16 w-auto" />
      <div className="max-w-md space-y-3">
        <p className="text-primary font-mono text-xs tracking-[0.3em] uppercase">{eyebrow}</p>
        <h1 className="text-3xl font-semibold tracking-tight text-balance">{title}</h1>
        <p className="text-muted-foreground text-sm leading-relaxed text-pretty">{body}</p>
      </div>
      {children}
      <a href="/docs/requirements/" className={cn(buttonVariants({ variant: 'outline' }), 'mt-1')}>
        {cta}
      </a>
    </main>
  )
}
