import type { ReportClip } from '@luna-web/core'
import { Card } from '@/components/ui/card'
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from '@/components/ui/table'
import { formatBytes, formatDuration } from '@/lib/format'

export function ClipTable({ clips }: { clips: ReportClip<Blob>[] }) {
  return (
    <Card className="overflow-hidden py-0">
      <Table>
        <TableHeader>
          <TableRow>
            <TableHead className="px-4">File</TableHead>
            <TableHead>TC</TableHead>
            <TableHead>Resolution</TableHead>
            <TableHead>Codec</TableHead>
            <TableHead className="text-right">FPS</TableHead>
            <TableHead className="text-right">Duration</TableHead>
            <TableHead className="text-right">ISO</TableHead>
            <TableHead>WB</TableHead>
            <TableHead>Lens</TableHead>
            <TableHead className="px-4 text-right">Size</TableHead>
          </TableRow>
        </TableHeader>
        <TableBody>
          {clips.map((clip) => {
            const m = clip.metadata
            return (
              <TableRow key={clip.id}>
                <TableCell
                  className="max-w-64 truncate px-4 font-mono font-medium"
                  title={clip.fileName}
                >
                  {clip.fileName}
                </TableCell>
                <TableCell className="font-mono tabular-nums">{m.startTimecode ?? '—'}</TableCell>
                <TableCell className="font-mono tabular-nums">
                  {m.width && m.height ? `${m.width}×${m.height}` : '—'}
                </TableCell>
                <TableCell>{m.codec ?? '—'}</TableCell>
                <TableCell className="text-right font-mono tabular-nums">
                  {m.frameRate ?? '—'}
                </TableCell>
                <TableCell className="text-right font-mono tabular-nums">
                  {m.durationSeconds !== undefined ? formatDuration(m.durationSeconds) : '—'}
                </TableCell>
                <TableCell className="text-right font-mono tabular-nums">{m.iso ?? '—'}</TableCell>
                <TableCell className="font-mono tabular-nums">{m.whiteBalance ?? '—'}</TableCell>
                <TableCell className="max-w-40 truncate" title={m.lens}>
                  {m.lens ?? '—'}
                </TableCell>
                <TableCell className="px-4 text-right font-mono tabular-nums">
                  {formatBytes(clip.sizeBytes)}
                </TableCell>
              </TableRow>
            )
          })}
        </TableBody>
      </Table>
    </Card>
  )
}
