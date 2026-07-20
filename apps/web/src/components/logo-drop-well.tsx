import { ImageUp } from 'lucide-react'
import { useRef, useState } from 'react'
import { useObjectUrl } from '@/lib/use-object-url'
import { cn } from '@/lib/utils'

export function LogoDropWell({
  id,
  value,
  onChange,
  className,
}: {
  id: string
  value: Blob | undefined
  onChange: (file: Blob | undefined) => void
  className?: string
}) {
  const previewUrl = useObjectUrl(value)
  const [dragging, setDragging] = useState(false)
  const inputRef = useRef<HTMLInputElement>(null)

  const accept = (file: File | undefined) => {
    if (file?.type.startsWith('image/')) onChange(file)
  }

  return (
    <div className="flex flex-col gap-1.5">
      <button
        type="button"
        onClick={() => inputRef.current?.click()}
        onDragOver={(e) => {
          e.preventDefault()
          setDragging(true)
        }}
        onDragLeave={() => setDragging(false)}
        onDrop={(e) => {
          e.preventDefault()
          setDragging(false)
          accept(e.dataTransfer.files?.[0])
        }}
        className={cn(
          'focus-visible:border-ring focus-visible:ring-ring/50 flex aspect-[4/3] w-full cursor-pointer flex-col items-center justify-center gap-1.5 rounded-lg border border-dashed p-3 text-center outline-none transition focus-visible:ring-3',
          dragging
            ? 'border-primary bg-primary/5'
            : 'border-border hover:border-input hover:bg-muted/30',
          className,
        )}
      >
        {previewUrl ? (
          <img
            src={previewUrl}
            alt="Report logo"
            className="max-h-full max-w-full object-contain"
          />
        ) : (
          <>
            <ImageUp className="text-muted-foreground size-5" />
            <span className="text-muted-foreground text-xs">Drop or click</span>
          </>
        )}
      </button>
      {previewUrl && (
        <button
          type="button"
          onClick={() => onChange(undefined)}
          className="text-muted-foreground hover:text-foreground text-xs underline-offset-2 hover:underline"
        >
          Remove logo
        </button>
      )}
      <input
        ref={inputRef}
        id={id}
        type="file"
        accept="image/*"
        className="hidden"
        onChange={(e) => accept(e.target.files?.[0])}
      />
    </div>
  )
}
