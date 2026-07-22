import { createFileRoute } from '@tanstack/react-router'
import { Logo } from '@/components/logo'

const STACK: ReadonlyArray<[string, string]> = [
  ['mediainfo.js', 'container + camera metadata (WASM)'],
  ['mediabunny + @mediabunny/prores', 'WebCodecs decode, ProRes via TurboRes'],
  ['ffmpeg.wasm', 'fallback decode engine (CDN-cached)'],
  ['React 19 + TanStack Router / Store / Form', 'application framework'],
  ['Tailwind CSS 4 + shadcn/ui (Base UI)', 'interface'],
  ['react-pdf', 'PDF report rendering'],
  ['idb', 'IndexedDB persistence'],
  ['comlink', 'worker RPC'],
  ['Lucide + Geist', 'icons + type'],
  ['Bun + Vite + Biome', 'build tooling'],
]

function CreditsScreen() {
  return (
    <div className="mx-auto flex max-w-2xl flex-col items-center gap-8 text-center">
      <Logo className="drop-shadow-glow-lg h-16 w-auto" />
      <div>
        <h1 className="text-2xl font-semibold tracking-tight">Luna Web</h1>
        <p className="text-muted-foreground mt-1 tabular-nums">v{__APP_VERSION__}</p>
      </div>
      <p className="text-muted-foreground max-w-prose text-sm leading-relaxed">
        A camera report tool for DITs. Your footage never leaves your device — Luna scans, reads
        metadata, and renders thumbnails entirely in your browser. No uploads, no analytics, no
        telemetry.
      </p>
      <dl className="grid w-full grid-cols-[auto_1fr] gap-x-6 gap-y-2 text-left text-sm">
        {STACK.map(([name, role]) => (
          <div key={name} className="contents">
            <dt className="font-mono whitespace-nowrap">{name}</dt>
            <dd className="text-muted-foreground">{role}</dd>
          </div>
        ))}
      </dl>
    </div>
  )
}

export const Route = createFileRoute('/credits')({
  component: CreditsScreen,
})
