import { generateReportCsv } from '@luna-web/core'
import { type Exporter, exporters } from './exporter'

export const csvExporter: Exporter = {
  id: 'csv',
  label: 'CSV',
  extension: 'csv',
  mime: 'text/csv',
  generate: async (report) =>
    new Blob([generateReportCsv(report)], { type: 'text/csv;charset=utf-8' }),
}

exporters.push(csvExporter)
