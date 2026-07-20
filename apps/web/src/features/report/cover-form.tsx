import { useForm } from '@tanstack/react-form'
import { useSelector } from '@tanstack/react-store'
import { ImageUp } from 'lucide-react'
import { useRef, useState } from 'react'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import { todayIso } from '@/lib/format'
import { useObjectUrl } from '@/lib/use-object-url'
import { cn } from '@/lib/utils'
import { coverStore, setCoverFields } from './cover-store'

const TEXT_FIELDS = [
  ['projectTitle', 'Project title'],
  ['productionCompany', 'Production company'],
  ['dit', 'DIT'],
  ['director', 'Director'],
  ['dp', 'Director of photography'],
  ['date', 'Date'],
] as const

type FieldName = (typeof TEXT_FIELDS)[number][0]

export function CoverForm() {
  const cover = coverStore.state

  const form = useForm({
    defaultValues: {
      projectTitle: cover.projectTitle ?? '',
      productionCompany: cover.productionCompany ?? '',
      dit: cover.dit ?? '',
      director: cover.director ?? '',
      dp: cover.dp ?? '',
      date: cover.date ?? todayIso(),
    },
  })

  const renderField = (name: FieldName, label: string) => (
    <form.Field key={name} name={name}>
      {(field) => (
        <div className="flex flex-col gap-1.5">
          <Label htmlFor={field.name} className="text-muted-foreground">
            {label}
          </Label>
          <Input
            id={field.name}
            name={field.name}
            type={name === 'date' ? 'date' : 'text'}
            value={field.state.value}
            onChange={(e) => field.handleChange(e.target.value)}
            onBlur={() => {
              field.handleBlur()
              setCoverFields({ [field.name]: field.state.value })
            }}
          />
        </div>
      )}
    </form.Field>
  )

  return (
    <Card className="w-full">
      <CardHeader>
        <CardTitle>Report details</CardTitle>
        <p className="text-muted-foreground text-sm">
          Shown on the report cover and in the exported PDF.
        </p>
      </CardHeader>
      <CardContent>
        <div className="grid gap-6 md:grid-cols-[1fr_11rem]">
          <div className="space-y-4">
            {renderField('projectTitle', 'Project title')}
            <div className="grid gap-4 sm:grid-cols-2">
              {TEXT_FIELDS.slice(1).map(([name, label]) => renderField(name, label))}
            </div>
          </div>
          <LogoDropWell />
        </div>
      </CardContent>
    </Card>
  )
}

function LogoDropWell() {
  const logo = useSelector(coverStore, (s) => s.logo)
  const previewUrl = useObjectUrl(logo)
  const [dragging, setDragging] = useState(false)
  const inputRef = useRef<HTMLInputElement>(null)

  const accept = (file: File | undefined) => {
    if (file?.type.startsWith('image/')) setCoverFields({ logo: file })
  }

  return (
    <div className="flex flex-col gap-1.5">
      <Label htmlFor="cover-logo" className="text-muted-foreground">
        Logo
      </Label>
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
          onClick={() => setCoverFields({ logo: undefined })}
          className="text-muted-foreground hover:text-foreground text-xs underline-offset-2 hover:underline"
        >
          Remove logo
        </button>
      )}
      <input
        ref={inputRef}
        id="cover-logo"
        type="file"
        accept="image/*"
        className="hidden"
        onChange={(e) => accept(e.target.files?.[0])}
      />
    </div>
  )
}
