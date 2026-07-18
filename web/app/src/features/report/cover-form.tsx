import { useForm } from '@tanstack/react-form'
import { useStore } from '@tanstack/react-store'
import { useEffect, useState } from 'react'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import { coverStore, setCoverFields } from './cover-store'

const TEXT_FIELDS = [
  ['projectTitle', 'Project title'],
  ['productionCompany', 'Production company'],
  ['dit', 'DIT'],
  ['director', 'Director'],
  ['dp', 'Director of photography'],
  ['date', 'Date'],
] as const

export function CoverForm() {
  const cover = coverStore.state

  const form = useForm({
    defaultValues: {
      projectTitle: cover.projectTitle ?? '',
      productionCompany: cover.productionCompany ?? '',
      dit: cover.dit ?? '',
      director: cover.director ?? '',
      dp: cover.dp ?? '',
      date: cover.date ?? new Date().toISOString().slice(0, 10),
    },
  })

  return (
    <Card className="w-full">
      <CardHeader>
        <CardTitle>Report details</CardTitle>
      </CardHeader>
      <CardContent>
        <div className="grid grid-cols-2 gap-4">
          {TEXT_FIELDS.map(([name, label]) => (
            <form.Field key={name} name={name}>
              {(field) => (
                <div className="flex flex-col gap-1.5">
                  <Label htmlFor={field.name} className="text-muted-foreground">
                    {label}
                  </Label>
                  <Input
                    id={field.name}
                    name={field.name}
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
          ))}
          <LogoPicker />
        </div>
      </CardContent>
    </Card>
  )
}

function LogoPicker() {
  const logo = useStore(coverStore, (s) => s.logo)
  const [previewUrl, setPreviewUrl] = useState<string | null>(null)

  useEffect(() => {
    if (!logo) {
      setPreviewUrl(null)
      return
    }
    const url = URL.createObjectURL(logo)
    setPreviewUrl(url)
    return () => URL.revokeObjectURL(url)
  }, [logo])

  return (
    <div className="flex flex-col gap-1.5">
      <Label htmlFor="cover-logo" className="text-muted-foreground">
        Logo
      </Label>
      <input
        id="cover-logo"
        type="file"
        accept="image/*"
        className="text-muted-foreground file:bg-secondary file:text-secondary-foreground hover:file:bg-secondary/80 text-sm file:mr-3 file:rounded-md file:border-0 file:px-3 file:py-1.5 file:text-sm"
        onChange={(e) => {
          const file = e.target.files?.[0]
          if (file) setCoverFields({ logo: file })
        }}
      />
      {previewUrl && (
        <img
          src={previewUrl}
          alt="Report logo preview"
          className="mt-1 h-10 w-auto object-contain"
        />
      )}
    </div>
  )
}
