import type { ReportModel } from '@luna-web/core'
import { expect, test } from 'vite-plus/test'
import { csvExporter } from './csv-exporter'

const model: ReportModel<Blob> = {
  cover: {},
  sourceRoot: 'CARD_A',
  stats: {
    cardCount: 1,
    clipCount: 1,
    otherFileCount: 0,
    otherFileSizeBytes: 0,
    totalDurationSeconds: 10,
    totalSizeBytes: 100,
    unreadableFileCount: 0,
    unreadableDirectoryCount: 0,
    clipsMissingMetadataCount: 0,
  },
  reels: [
    {
      name: 'A001',
      otherFiles: [],
      stats: {
        clipCount: 1,
        otherFileCount: 0,
        otherFileSizeBytes: 0,
        totalSizeBytes: 100,
        totalDurationSeconds: 10,
      },
      clips: [
        {
          id: 'A001/clip.mov',
          fileName: 'clip.mov',
          relativePath: 'A001/clip.mov',
          extension: '.mov',
          sizeBytes: 100,
          metadata: { lens: 'Zeiss Süper Speed 50mm' },
          thumbnails: [],
        },
      ],
    },
  ],
  unreadableEntries: [],
}

test('the exported CSV starts with a UTF-8 BOM', async () => {
  const blob = await csvExporter.generate(model)
  const bytes = new Uint8Array(await blob.arrayBuffer())
  // Asserted on BYTES, not on blob.text(): the WHATWG "UTF-8 decode" algorithm
  // that Blob.text() runs strips a leading BOM, so a decoded string can never
  // show it. Excel reads bytes, so bytes are what has to be verified.
  expect([...bytes.slice(0, 3)]).toEqual([0xef, 0xbb, 0xbf])
})

test('the header row and non-ASCII text survive the BOM', async () => {
  const blob = await csvExporter.generate(model)
  // blob.text() drops the BOM (see above), so this is the header row itself.
  const text = await blob.text()
  expect(text.split('\r\n')[0]?.startsWith('reel,')).toBe(true)
  expect(text).toContain('Zeiss Süper Speed 50mm')
})
