import type { ReportModel } from '@luna-web/core'
import { useEffect, useState } from 'react'
import { Check } from 'lucide-react'
import { Button } from '@/components/ui/button'
import { logger } from '@/lib/logger'
import { saveReport } from '@/persistence/report-library'

export function SaveReportButton({ model }: { model: ReportModel<Blob> }) {
  const [state, setState] = useState<'idle' | 'saving' | 'saved'>('idle')
  const [error, setError] = useState<string | null>(null)

  // A new/changed model (fresh run, cover edit) re-arms the button.
  // biome-ignore lint/correctness/useExhaustiveDependencies: model identity change is the reset trigger, not its fields
  useEffect(() => {
    setState('idle')
    setError(null)
  }, [model])

  return (
    <div className="flex items-center gap-2">
      <Button
        variant="outline"
        disabled={state !== 'idle'}
        onClick={() => {
          setState('saving')
          setError(null)
          saveReport(model)
            .then((summary) => {
              setState('saved')
              logger.info('Report saved to library', summary.title)
            })
            .catch((err) => {
              setState('idle')
              const message =
                err instanceof DOMException && err.name === 'QuotaExceededError'
                  ? 'Not enough storage space — free disk space or delete saved reports, then try again.'
                  : err instanceof Error
                    ? err.message
                    : String(err)
              setError(message)
              logger.error('Saving report failed', message)
            })
        }}
      >
        {state === 'saving' ? (
          'Saving…'
        ) : state === 'saved' ? (
          <>
            <Check className="size-4" /> Saved
          </>
        ) : (
          'Save report'
        )}
      </Button>
      {error && <span className="text-destructive text-sm">{error}</span>}
    </div>
  )
}
