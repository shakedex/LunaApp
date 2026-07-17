import { type ReactNode, useMemo } from 'react'
import { detectCapabilities, isBrowserSupported } from '@/lib/capabilities'

export function CapabilityGate({ children }: { children: ReactNode }) {
  const supported = useMemo(() => isBrowserSupported(detectCapabilities()), [])

  if (supported) return <>{children}</>

  return (
    <main className="flex min-h-screen flex-col items-center justify-center gap-4 bg-zinc-950 px-6 text-center text-zinc-100">
      <h1 className="text-2xl font-semibold">Luna Web needs a Chromium browser</h1>
      <p className="max-w-md text-zinc-400">
        Luna Web reads your footage folder directly on your device using the File System Access API,
        available in Chrome, Edge, and other Chromium browsers. Please open this page in one of
        those to continue. Nothing is ever uploaded.
      </p>
    </main>
  )
}
