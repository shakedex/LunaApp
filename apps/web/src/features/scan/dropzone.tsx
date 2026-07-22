import { FolderOpen, Upload } from 'lucide-react'
import { useState } from 'react'
import { Logo } from '@/components/logo'
import { cn } from '@/lib/utils'
import { RecentList } from './recent-list'
import { pickAndScan, scanFrom } from './run-scan'

type FsItem = DataTransferItem & {
  getAsFileSystemHandle?: () => Promise<FileSystemHandle | null>
}

const FORMATS = ['MOV', 'MP4', 'MXF', 'MKV', 'M4V', 'AVI', 'MTS', 'M2TS', '3GP', 'WEBM']

// The cameras Luna reads vendor-specific metadata for — one per enricher in
// packages/core/src/metadata/vendors/registry.ts. Update both together.
const VENDORS = ['ARRI', 'SONY', 'CANON', 'BLACKMAGIC', 'PANASONIC']

export function Dropzone() {
  const [dragging, setDragging] = useState(false)

  async function handleDrop(e: React.DragEvent) {
    e.preventDefault()
    setDragging(false)
    const item = e.dataTransfer.items?.[0] as FsItem | undefined
    const handle = await item?.getAsFileSystemHandle?.()
    if (handle && handle.kind === 'directory') {
      await scanFrom(handle as FileSystemDirectoryHandle)
    }
  }

  return (
    <>
      <div
        aria-hidden="true"
        className="landing-atmosphere pointer-events-none fixed inset-0 z-0"
      />

      {/* One measure for the whole composition — the plate, the recent list and
          the reference chips all share max-w-xl so their edges line up. */}
      <div className="relative z-10 mx-auto flex min-h-[calc(100dvh-9rem)] w-full max-w-xl flex-col items-center justify-center gap-7 py-4 text-center">
        {/* Deliberate rhythm rather than one uniform gap: the mark gets the most
            air, the headline and its supporting line sit close as one unit. */}
        <div>
          <Logo className="drop-shadow-glow-lg mx-auto mb-5 h-20 w-auto" />
          <h1 className="text-3xl font-semibold tracking-[-0.03em]">
            Camera reports, in your browser
          </h1>
          <p className="text-muted-foreground mt-2.5 text-sm leading-relaxed">
            Per-clip metadata, thumbnails, and totals — exported as PDF or CSV.
          </p>
        </div>

        <button
          type="button"
          aria-label="Pick or drop a footage folder to scan"
          onClick={() => void pickAndScan()}
          onDragOver={(e) => {
            e.preventDefault()
            setDragging(true)
          }}
          onDragLeave={() => setDragging(false)}
          onDrop={(e) => void handleDrop(e)}
          className={cn(
            // 1px edge, not 2px: every other border on this page is a hairline,
            // and the heavy dash was the least refined edge in the composition.
            'group focus-visible:ring-ring/50 flex w-full cursor-pointer flex-col items-center gap-4 rounded-xl border border-dashed px-8 py-9 outline-none transition-[background-color,border-color,box-shadow] duration-200 focus-visible:ring-3',
            dragging
              ? 'border-primary bg-primary/10 shadow-panel-drag'
              : 'bg-card/50 border-border shadow-panel hover:border-input hover:bg-card/80 hover:shadow-panel-hover',
          )}
        >
          <span
            className={cn(
              'flex size-14 items-center justify-center rounded-full border transition-colors duration-200',
              dragging
                ? 'border-primary bg-primary/10 text-primary'
                : 'border-border text-muted-foreground group-hover:border-input group-hover:text-foreground',
            )}
          >
            <Upload className="size-6" />
          </span>
          <span className="block space-y-1.5">
            <span className="block font-medium">
              {dragging ? 'Drop to scan this folder' : 'Drop a footage folder here'}
            </span>
            <span className="text-muted-foreground block text-sm">
              or click anywhere to choose one
            </span>
          </span>
          <span className="bg-primary text-primary-foreground shadow-glow mt-1 inline-flex items-center gap-1.5 rounded-lg px-3 py-1.5 text-sm font-medium">
            <FolderOpen className="size-4" /> Pick folder
          </span>
        </button>

        <RecentList />

        {/* Reference matter, so it sits further from the recent list than the
            list does from the plate — a footer, not another peer section. */}
        <div className="text-muted-foreground-dim mt-3 space-y-1.5">
          <div className="text-2xs flex flex-wrap items-center justify-center gap-1.5 font-mono">
            {FORMATS.map((f) => (
              <span key={f} className="border-border/50 rounded-sm border px-1.5 py-0.5">
                {f}
              </span>
            ))}
          </div>
          <div className="text-2xs flex flex-wrap items-center justify-center gap-1.5 font-mono">
            {VENDORS.map((v, i) => (
              // The marker rides along with the last chip so it can't wrap onto
              // a line of its own in a narrow window.
              <span key={v} className="flex items-center gap-1">
                <span className="border-border/50 rounded-sm border px-1.5 py-0.5">{v}</span>
                {i === VENDORS.length - 1 && (
                  <span className="text-primary/70" aria-hidden="true">
                    *
                  </span>
                )}
              </span>
            ))}
          </div>
          <p className="text-2xs mx-auto max-w-md leading-relaxed">
            * Raw formats — Sony X-OCN, ARRIRAW, BRAW — report full metadata, but can't be decoded
            to a thumbnail in the browser, so those clips get a placeholder.
          </p>
        </div>
      </div>
    </>
  )
}
