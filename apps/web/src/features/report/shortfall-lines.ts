import type { ReportStats, ScanSummary } from '@luna-web/core'

// One home for every sentence that says what a report could not read. Three
// renderers need these (the on-screen notice, the PDF, the scan summary row) and
// they must not drift apart — the screen and the printed page differ only in
// whether they can say "in the totals" or must say "above".
//
// No pronouns that break at n=1: "duration not included", never "their duration".
const plural = (n: number, one: string, many: string) => `${n} ${n === 1 ? one : many}`

/** Sentences for the on-screen notice. Empty when the report is complete. */
export function reportShortfalls(stats: ReportStats): string[] {
  const lines: string[] = []
  if (stats.clipsMissingMetadataCount > 0) {
    lines.push(
      `${plural(stats.clipsMissingMetadataCount, 'clip', 'clips')} missing metadata — duration not included in the totals.`,
    )
  }
  if (stats.unreadableFileCount > 0) {
    lines.push(
      `${plural(stats.unreadableFileCount, 'file', 'files')} could not be opened — not counted in the totals.`,
    )
  }
  if (stats.unreadableDirectoryCount > 0) {
    // A folder is the worse case: the count of what is missing behind it is itself unknown.
    lines.push(
      `${plural(stats.unreadableDirectoryCount, 'folder', 'folders')} could not be opened — nothing inside ${stats.unreadableDirectoryCount === 1 ? 'it' : 'them'} is counted in the totals.`,
    )
  }
  return lines
}

/** The same facts worded for a printed page, where "the totals" are literally above. */
export function pdfShortfalls(stats: ReportStats): string[] {
  const lines: string[] = []
  if (stats.clipsMissingMetadataCount > 0) {
    lines.push(
      `${plural(stats.clipsMissingMetadataCount, 'clip', 'clips')} missing metadata — duration not included above`,
    )
  }
  if (stats.unreadableFileCount > 0) {
    lines.push(
      `${plural(stats.unreadableFileCount, 'file', 'files')} could not be opened — not counted above`,
    )
  }
  if (stats.unreadableDirectoryCount > 0) {
    lines.push(
      `${plural(stats.unreadableDirectoryCount, 'folder', 'folders')} could not be opened — nothing inside ${stats.unreadableDirectoryCount === 1 ? 'it' : 'them'} is counted above`,
    )
  }
  return lines
}

/** The scan summary's one-line warning, or null when the card read cleanly. */
export function scanShortfall(summary: ScanSummary): string | null {
  const { unreadableFileCount: files, unreadableDirectoryCount: dirs } = summary
  if (files === 0 && dirs === 0) return null
  if (dirs === 0)
    return `${plural(files, 'file', 'files')} couldn't be opened and won't be in the report`
  if (files === 0) {
    return `${plural(dirs, 'folder', 'folders')} couldn't be opened and nothing inside ${dirs === 1 ? 'it' : 'them'} will be in the report`
  }
  return `${plural(files, 'file', 'files')} and ${plural(dirs, 'folder', 'folders')} couldn't be opened and won't be in the report`
}
