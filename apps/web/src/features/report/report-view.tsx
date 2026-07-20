import type { ReportModel } from '@luna-web/core'
import { type ReactNode, useEffect, useState } from 'react'
import { StatTile } from '@/components/stat-tile'
import { Card, CardContent } from '@/components/ui/card'
import { Table, TableBody, TableCell, TableRow } from '@/components/ui/table'
import { formatBytes, formatDuration } from '@/lib/format'
import { useObjectUrl } from '@/lib/use-object-url'
import { cn } from '@/lib/utils'
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
  ].filter(Boolean)

  const multiReel = model.reels.length > 1

  const [activeReel, setActiveReel] = useState<string | null>(null)

  // A new/changed model (different reels) re-scans the DOM for sections to observe.
  // biome-ignore lint/correctness/useExhaustiveDependencies: model identity change is the re-scan trigger, not its fields
  useEffect(() => {
    const sections = document.querySelectorAll('[data-reel-section]')
    if (sections.length < 2) return
    const observer = new IntersectionObserver(
      (entries) => {
        for (const entry of entries) {
          if (entry.isIntersecting) setActiveReel(entry.target.id)
        }
      },
      // Band across the upper-middle viewport: the reel whose content crosses it is "current".
      { rootMargin: '-30% 0px -60% 0px' },
    )
    for (const s of sections) observer.observe(s)
    return () => observer.disconnect()
  }, [model])

  return (
    <section className="w-full">
      <header className="mb-6 flex flex-wrap items-start justify-between gap-4">
        <div className="min-w-0">
          <p className="text-muted-foreground text-xs font-medium tracking-wider uppercase">
            {eyebrow}
          </p>
          <h1 className="truncate text-3xl font-semibold tracking-tight">
            {model.cover.projectTitle || 'Camera report'}
          </h1>
          <p className="text-muted-foreground mt-1.5 font-mono text-sm tabular-nums">
            {model.cover.date} · {model.stats.cardCount}{' '}
            {model.stats.cardCount === 1 ? 'card' : 'cards'} · {model.stats.clipCount}{' '}
            {model.stats.clipCount === 1 ? 'clip' : 'clips'} ·{' '}
            {formatDuration(model.stats.totalDurationSeconds)} ·{' '}
            {formatBytes(model.stats.totalSizeBytes)}
          </p>
          {metaLine.length > 0 && (
            <p className="text-muted-foreground mt-0.5 text-sm">{metaLine.join('  ·  ')}</p>
          )}
        </div>
        <div className="flex shrink-0 items-center gap-3">
          <CoverLogo logo={model.cover.logo} />
          {actions}
        </div>
      </header>

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
        <nav
          aria-label="Reels"
          className="bg-background/80 sticky top-14 z-20 -mx-2 mb-6 flex flex-wrap gap-2 border-b px-2 py-2 backdrop-blur"
        >
          {model.reels.map((reel) => {
            const id = `reel-${slug(reel.name)}`
            const isActive = activeReel === id
            return (
              <a
                key={reel.name}
                href={`#${id}`}
                aria-current={isActive ? 'true' : undefined}
                className={cn(
                  'flex items-center gap-2 rounded-full border px-3 py-1 text-sm transition-colors outline-none focus-visible:ring-3 focus-visible:ring-ring/50',
                  isActive
                    ? 'border-primary/40 bg-primary/10 text-foreground'
                    : 'bg-card hover:border-input',
                )}
              >
                <span className="font-medium">{reel.name}</span>
                <span className="text-muted-foreground font-mono tabular-nums">
                  {reel.stats.clipCount}
                </span>
              </a>
            )
          })}
        </nav>
      )}

      {model.reels.map((reel) => (
        <section
          key={reel.name}
          id={`reel-${slug(reel.name)}`}
          data-reel-section
          className={cn('mb-8', multiReel ? 'scroll-mt-[7rem]' : 'scroll-mt-16')}
        >
          <div
            className={cn(
              'bg-background/80 sticky z-10 mb-3 flex flex-wrap items-baseline justify-between gap-x-4 gap-y-1 border-b py-2 backdrop-blur',
              multiReel ? 'top-[6.5rem]' : 'top-14',
            )}
          >
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
                <Table>
                  <TableBody>
                    {reel.otherFiles.map((f) => (
                      <TableRow key={f.relativePath}>
                        <TableCell className="px-4 font-medium">{f.fileName}</TableCell>
                        <TableCell className="text-muted-foreground w-full truncate px-4">
                          {f.relativePath}
                        </TableCell>
                        <TableCell className="text-muted-foreground px-4 text-right font-mono tabular-nums">
                          {formatBytes(f.sizeBytes)}
                        </TableCell>
                      </TableRow>
                    ))}
                  </TableBody>
                </Table>
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
