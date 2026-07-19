import { type ReactNode, useMemo } from 'react'
import { Logo } from '@/components/logo'
import { detectCapabilities, isBrowserSupported } from '@/lib/capabilities'

export function CapabilityGate({ children }: { children: ReactNode }) {
  const supported = useMemo(() => isBrowserSupported(detectCapabilities()), [])

  if (supported) return <>{children}</>

  return (
    <main className="bg-background text-foreground flex min-h-dvh flex-col items-center justify-center gap-5 px-6 text-center">
      <Logo className="h-12 w-auto" />
      <h1 className="text-2xl font-semibold">Luna needs a Chromium browser</h1>
      <p className="text-muted-foreground max-w-md text-sm leading-relaxed">
        Luna reads your footage folder directly on your device using the File System Access API,
        available in Chrome, Edge, and other Chromium browsers. Please open this page in one of
        those to continue. Nothing is ever uploaded.
      </p>
    </main>
  )
}
