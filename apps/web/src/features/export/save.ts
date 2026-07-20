import { fileExtensionOf } from '@luna-web/core'
import { todayIso } from '@/lib/format'

export function reportFileName(
  projectTitle: string | undefined,
  date: string | undefined,
  extension: string,
): string {
  const slug = (projectTitle ?? '')
    .toLowerCase()
    .replaceAll(/\s+/g, '-')
    .replaceAll(/[^a-z0-9-]/g, '')
  const day = date ?? todayIso()
  return `${slug || 'luna-report'}-${day}.${extension}`
}

// `accept`'s key type is a MIME-shaped template literal (`${string}/${string}`)
// and its value is a `.ext`-shaped template literal, neither of which a plain
// `string` parameter satisfies as a computed property. Build the record
// separately and assert it to the narrow shape `FilePickerAcceptType` expects.
type AcceptRecord = Record<`${string}/${string}`, `.${string}`[]>

export async function saveBlob(blob: Blob, fileName: string, mime: string): Promise<void> {
  if ('showSaveFilePicker' in window) {
    try {
      const ext = fileExtensionOf(fileName).slice(1)
      const accept = { [mime]: [`.${ext}`] } as AcceptRecord
      const handle = await window.showSaveFilePicker({
        suggestedName: fileName,
        types: [{ description: ext.toUpperCase() || 'File', accept }],
      })
      const writable = await handle.createWritable()
      await writable.write(blob)
      await writable.close()
      return
    } catch (err) {
      if (err instanceof DOMException && err.name === 'AbortError') return // user cancelled
      throw err
    }
  }
  const url = URL.createObjectURL(blob)
  const anchor = document.createElement('a')
  anchor.href = url
  anchor.download = fileName
  anchor.click()
  // Defer the revoke: revoking synchronously can cancel a large download
  // before the browser finishes reading the blob (final-review finding).
  setTimeout(() => URL.revokeObjectURL(url), 60_000)
}
