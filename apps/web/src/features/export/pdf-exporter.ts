import { pdf } from '@react-pdf/renderer'
import { createElement } from 'react'
import { scanStore } from '@/features/scan/store'
import { type Exporter, exporters } from './exporter'
import { ReportDocument } from './pdf-document'
import { prepareReportForPdf } from './pdf-prepare'

export const pdfExporter: Exporter = {
  id: 'pdf',
  label: 'PDF',
  extension: 'pdf',
  mime: 'application/pdf',
  generate: async (report) => {
    const prepared = await prepareReportForPdf(report, scanStore.state.sourceName ?? '')
    // react-pdf's `pdf()` types its argument as ReactElement<DocumentProps>, which only
    // matches a literal <Document> element. Wrapping it in our own component (so we can
    // pass a typed `report` prop) is a well-known TS "weak type" mismatch — the runtime
    // tree is a valid <Document> root regardless, so the cast is safe.
    const element = createElement(ReportDocument, { report: prepared }) as Parameters<typeof pdf>[0]
    return pdf(element).toBlob()
  },
}

exporters.unshift(pdfExporter) // PDF leads — it is the product's deliverable
