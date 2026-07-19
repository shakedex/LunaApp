import { FolderOpen, Upload } from 'lucide-react'
import { useState } from 'react'
import { Logo } from '@/components/logo'
import { cn } from '@/lib/utils'
import { RecentList } from './recent-list'
import { pickAndScan, scanFrom } from './run-scan'

const FORMATS = ['MOV', 'MP4', 'MXF', 'MKV', 'M4V', 'AVI', 'MTS', 'M2TS', '3GP', 'WEBM']

type FsItem = DataTransferItem & {
  getAsFileSystemHandle?: () => Promise<FileSystemHandle | null>
}

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
    <div className="flex flex-col items-center gap-8 py-10 text-center">
      <div className="space-y-3">
        <Logo className="mx-auto h-14 w-auto" />
        <h1 className="text-2xl font-semibold tracking-tight">Camera reports, in your browser</h1>
        <p className="text-muted-foreground mx-auto max-w-md text-sm leading-relaxed">
          Choose your footage folder and Luna will scan and generate a camera report with per-clip
          metadata, thumbnails, and totals. Export as PDF or CSV.
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
          'focus-visible:border-ring focus-visible:ring-ring/50 flex w-full max-w-xl cursor-pointer flex-col items-center gap-4 rounded-2xl border-2 border-dashed px-8 py-14 outline-none transition focus-visible:ring-3',
          dragging
            ? 'border-primary bg-primary/5'
            : 'border-border hover:border-input hover:bg-muted/30',
        )}
      >
        <span
          className={cn(
            'flex size-14 items-center justify-center rounded-full border transition',
            dragging ? 'border-primary text-primary' : 'border-border text-muted-foreground',
          )}
        >
          <Upload className="size-6" />
        </span>
        <span className="block space-y-1">
          <span className="block font-medium">
            {dragging ? 'Drop to scan this folder' : 'Drop a footage folder here'}
          </span>
          <span className="text-muted-foreground block text-sm">
            or click anywhere to choose one
          </span>
        </span>
        <span
          className="bg-primary text-primary-foreground mt-1 inline-flex items-center gap-1.5 rounded-lg px-3 py-1.5 text-sm font-medium"
          style={{ boxShadow: '0 0 24px oklch(0.72 0.14 245 / 0.25)' }}
        >
          <FolderOpen className="size-4" /> Pick folder
        </span>
      </button>

      <div className="text-muted-foreground flex flex-wrap items-center justify-center gap-1.5 font-mono text-xs">
        {FORMATS.map((f) => (
          <span key={f} className="border-border rounded border px-1.5 py-0.5">
            {f}
          </span>
        ))}
      </div>

      <RecentList />
    </div>
  )
}
