import type { ReportModel } from '@luna-web/core'
import type { ReactNode } from 'react'
import { StatTile } from '@/components/stat-tile'
import { Card, CardContent } from '@/components/ui/card'
import { formatBytes, formatDuration } from '@/lib/format'
import { useObjectUrl } from '@/lib/use-object-url'
import { ClipCard } from './clip-card'

const slug = (s: string) => s.replace(/[^a-z0-9]+/gi, '-').toLowerCase()

export function ReportView({
  model,
  eyebrow = 'Camera Report',
  actions,
  children,
}: {
  model: ReportModel<Blob>
  eyebrow?: string
  actions: ReactNode
  children?: ReactNode
}) {
  const metaLine = [
    model.cover.productionCompany,
    model.cover.dit && `DIT · ${model.cover.dit}`,
    model.cover.director && `Director · ${model.cover.director}`,
    model.cover.dp && `DP · ${model.cover.dp}`,
    model.cover.date,
  ].filter(Boolean)

  const multiReel = model.reels.length > 1

  return (
    <section className="w-full">
      <div className="mb-6 flex flex-wrap items-start justify-between gap-4">
        <div className="min-w-0">
          <p className="text-muted-foreground text-xs font-medium tracking-wider uppercase">
            {eyebrow}
          </p>
          <h1 className="truncate text-2xl font-semibold tracking-tight">
            {model.cover.projectTitle || 'Camera report'}
          </h1>
          {metaLine.length > 0 && (
            <p className="text-muted-foreground mt-1 text-sm">{metaLine.join('  ·  ')}</p>
          )}
        </div>
        <div className="flex shrink-0 items-center gap-3">
          <CoverLogo logo={model.cover.logo} />
          {actions}
        </div>
      </div>

      <Card className="mb-6">
        <CardContent>
          <div className="grid grid-cols-2 gap-4 sm:grid-cols-5">
            <StatTile label="Cards" value={String(model.stats.cardCount)} />
            <StatTile label="Clips" value={String(model.stats.clipCount)} />
            <StatTile label="Other files" value={String(model.stats.otherFileCount)} />
            <StatTile label="Duration" value={formatDuration(model.stats.totalDurationSeconds)} />
            <StatTile label="Size" value={formatBytes(model.stats.totalSizeBytes)} />
          </div>
        </CardContent>
      </Card>

      {children}

      {multiReel && (
        <nav className="mb-6 flex flex-wrap gap-2">
          {model.reels.map((reel) => (
            <a
              key={reel.name}
              href={`#reel-${slug(reel.name)}`}
              className="bg-card hover:border-input flex items-center gap-2 rounded-full border px-3 py-1 text-sm transition-colors"
            >
              <span className="font-medium">{reel.name}</span>
              <span className="text-muted-foreground font-mono tabular-nums">
                {reel.stats.clipCount}
              </span>
            </a>
          ))}
        </nav>
      )}

      {model.reels.map((reel) => (
        <section key={reel.name} id={`reel-${slug(reel.name)}`} className="mb-8 scroll-mt-16">
          <div className="bg-background/80 sticky top-14 z-10 mb-3 flex flex-wrap items-baseline justify-between gap-x-4 gap-y-1 border-b py-2 backdrop-blur">
            <h2 className="text-lg font-semibold">{reel.name}</h2>
            <span className="text-muted-foreground font-mono text-sm tabular-nums">
              {reel.stats.clipCount} {reel.stats.clipCount === 1 ? 'clip' : 'clips'}
              {reel.stats.otherFileCount > 0 &&
                ` · ${reel.stats.otherFileCount} other ${
                  reel.stats.otherFileCount === 1 ? 'file' : 'files'
                }`}{' '}
              · {formatDuration(reel.stats.totalDurationSeconds)} ·{' '}
              {formatBytes(reel.stats.totalSizeBytes)}
            </span>
          </div>
          <div className="grid gap-4">
            {reel.clips.map((clip) => (
              <ClipCard key={clip.id} clip={clip} sourceRoot={model.sourceRoot} />
            ))}
          </div>
          {reel.otherFiles.length > 0 && (
            <div className="mt-4">
              <h3 className="text-muted-foreground mb-2 text-sm font-medium">Other files</h3>
              <Card className="overflow-hidden py-0">
                <table className="w-full text-sm">
                  <tbody className="divide-y">
                    {reel.otherFiles.map((f) => (
                      <tr key={f.relativePath}>
                        <td className="px-4 py-2 font-medium whitespace-nowrap">{f.fileName}</td>
                        <td className="text-muted-foreground w-full truncate px-4 py-2">
                          {f.relativePath}
                        </td>
                        <td className="text-muted-foreground px-4 py-2 text-right font-mono whitespace-nowrap tabular-nums">
                          {formatBytes(f.sizeBytes)}
                        </td>
                      </tr>
                    ))}
                  </tbody>
                </table>
              </Card>
            </div>
          )}
        </section>
      ))}
    </section>
  )
}

function CoverLogo({ logo }: { logo: Blob | undefined }) {
  const url = useObjectUrl(logo)

  if (!url) return null
  return <img src={url} alt="Production logo" className="h-10 w-auto object-contain" />
}
