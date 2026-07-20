import { useForm } from '@tanstack/react-form'
import { useSelector } from '@tanstack/react-store'
import { LogoDropWell } from '@/components/logo-drop-well'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import { todayIso } from '@/lib/format'
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
  const logo = useSelector(coverStore, (s) => s.logo)

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
          <div className="flex flex-col gap-1.5">
            <Label htmlFor="cover-logo" className="text-muted-foreground">
              Logo
            </Label>
            <LogoDropWell
              id="cover-logo"
              value={logo}
              onChange={(file) => setCoverFields({ logo: file ?? undefined })}
            />
          </div>
        </div>
      </CardContent>
    </Card>
  )
}
