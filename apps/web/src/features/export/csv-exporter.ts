import { generateReportCsv } from '@luna-web/core'
import { type Exporter, exporters } from './exporter'

// Excel on Windows ignores the MIME charset when opening a local file and falls
// back to the system codepage, mangling non-ASCII filenames and lens names. The
// BOM is the only signal it honours. It belongs here, at the file boundary —
// generateReportCsv stays a pure string function.
const UTF8_BOM = '\uFEFF'

export const csvExporter: Exporter = {
  id: 'csv',
  label: 'CSV',
  extension: 'csv',
  mime: 'text/csv',
  generate: async (report) =>
    new Blob([UTF8_BOM, generateReportCsv(report)], { type: 'text/csv;charset=utf-8' }),
}

exporters.push(csvExporter)
