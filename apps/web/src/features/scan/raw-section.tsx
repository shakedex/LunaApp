import type { RawNotice } from '@luna-web/core'
import { TriangleAlert } from 'lucide-react'
import { Card } from '@/components/ui/card'
import { formatBytes } from '@/lib/format'

/** Honest disclosure of files that could not be decoded in the browser. */
export function RawSection({ raw }: { raw: RawNotice[] }) {
  if (raw.length === 0) return null
  return (
    <section className="mt-6">
      <h3 className="text-muted-foreground mb-2 text-sm font-medium">
        Unsupported files (not decodable in browser)
      </h3>
      <Card className="overflow-hidden py-0">
        <ul className="divide-y">
          {raw.map((r) => (
            <li
              key={r.id}
              className="text-muted-foreground flex items-center justify-between px-4 py-2 text-sm"
            >
              <span className="flex min-w-0 items-center gap-2">
                <TriangleAlert className="size-4 shrink-0 text-amber-400" />
                <span className="truncate">{r.relativePath}</span>
              </span>
              <span className="ml-4 shrink-0 font-mono tabular-nums">
                {formatBytes(r.sizeBytes)}
              </span>
            </li>
          ))}
        </ul>
      </Card>
    </section>
  )
}
