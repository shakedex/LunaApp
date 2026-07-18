import { useForm } from '@tanstack/react-form'
import { useStore } from '@tanstack/react-store'
import { useEffect, useState } from 'react'
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
    <section className="w-full rounded-lg border p-6">
      <h3 className="mb-4 text-lg font-medium">Report details</h3>
      <div className="grid grid-cols-2 gap-4">
        {TEXT_FIELDS.map(([name, label]) => (
          <form.Field key={name} name={name}>
            {(field) => (
              <label className="flex flex-col gap-1 text-sm">
                <span className="text-muted-foreground">{label}</span>
                <input
                  className="bg-background rounded-md border px-3 py-2"
                  name={field.name}
                  value={field.state.value}
                  onChange={(e) => field.handleChange(e.target.value)}
                  onBlur={() => {
                    field.handleBlur()
                    setCoverFields({ [field.name]: field.state.value })
                  }}
                />
              </label>
            )}
          </form.Field>
        ))}
        <LogoPicker />
      </div>
    </section>
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
    <label className="flex flex-col gap-1 text-sm">
      <span className="text-muted-foreground">Logo</span>
      <input
        type="file"
        accept="image/*"
        className="text-sm"
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
    </label>
  )
}
