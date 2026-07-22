import { createFileRoute } from '@tanstack/react-router'
import { Fragment } from 'react'
import { Logo } from '@/components/logo'
import { buttonVariants } from '@/components/ui/button'
import { AUTHOR_NAME, AUTHOR_URL, GITHUB_URL } from '@/lib/site'
import { cn } from '@/lib/utils'

type Project = readonly [name: string, href: string]

/** Each row credits one or more upstream projects, joined by " + ". */
const STACK: ReadonlyArray<readonly [ReadonlyArray<Project>, string]> = [
  [[['mediainfo.js', 'https://mediainfo.js.org/']], 'container + camera metadata (WASM)'],
  [
    [
      ['mediabunny', 'https://github.com/vanilagy/mediabunny'],
      ['@mediabunny/prores', 'https://www.npmjs.com/package/@mediabunny/prores'],
    ],
    'WebCodecs decode, ProRes via TurboRes',
  ],
  [[['ffmpeg.wasm', 'https://ffmpegwasm.netlify.app/']], 'fallback decode engine (CDN-cached)'],
  [
    [
      ['React 19', 'https://react.dev/'],
      ['TanStack Router / Store / Form', 'https://tanstack.com/'],
    ],
    'application framework',
  ],
  [
    [
      ['Tailwind CSS 4', 'https://tailwindcss.com/'],
      ['shadcn/ui', 'https://ui.shadcn.com/'],
      ['Base UI', 'https://base-ui.com/'],
    ],
    'interface',
  ],
  [[['react-pdf', 'https://react-pdf.org/']], 'PDF report rendering'],
  [[['idb', 'https://github.com/jakearchibald/idb']], 'IndexedDB persistence'],
  [[['comlink', 'https://github.com/GoogleChromeLabs/comlink']], 'worker RPC'],
  [
    [
      ['Lucide', 'https://lucide.dev/'],
      ['Geist', 'https://vercel.com/font'],
    ],
    'icons + type',
  ],
  [
    [
      ['Bun', 'https://bun.sh/'],
      ['Vite+', 'https://viteplus.dev/'],
    ],
    'package manager + build tooling',
  ],
]

const LINK = 'hover:text-primary underline-offset-4 transition-colors hover:underline'

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
      <p className="text-muted-foreground text-sm">
        Built by{' '}
        <a
          href={AUTHOR_URL}
          target="_blank"
          rel="noreferrer"
          className={cn('text-foreground', LINK)}
        >
          {AUTHOR_NAME}
        </a>
      </p>
      <dl className="grid w-full grid-cols-[auto_1fr] gap-x-6 gap-y-2 text-left text-sm">
        {STACK.map(([projects, role]) => (
          <div key={projects[0][0]} className="contents">
            <dt className="font-mono whitespace-nowrap">
              {projects.map(([name, href], i) => (
                <Fragment key={name}>
                  {i > 0 ? <span className="text-muted-foreground"> + </span> : null}
                  <a href={href} target="_blank" rel="noreferrer" className={LINK}>
                    {name}
                  </a>
                </Fragment>
              ))}
            </dt>
            <dd className="text-muted-foreground">{role}</dd>
          </div>
        ))}
      </dl>
      <nav className="flex items-center gap-2">
        <a
          href={GITHUB_URL}
          target="_blank"
          rel="noreferrer"
          className={cn(buttonVariants({ variant: 'ghost', size: 'sm' }))}
        >
          GitHub
        </a>
        <a href="/docs/" className={cn(buttonVariants({ variant: 'ghost', size: 'sm' }))}>
          Docs
        </a>
        <a
          href={`${GITHUB_URL}/blob/master/LICENSE`}
          target="_blank"
          rel="noreferrer"
          className={cn(buttonVariants({ variant: 'ghost', size: 'sm' }))}
        >
          License
        </a>
      </nav>
    </div>
  )
}

export const Route = createFileRoute('/credits')({
  component: CreditsScreen,
})
