import { describe, expect, test } from 'vite-plus/test'
import { extractSonyMxfCameraModel, sonyCameraDisplayName } from '../src/metadata/sony-klv'
import type { BlobLike } from '../src/scan/handles'

// ---------------------------------------------------------------------------
// KLV fixture builders — synthetic MXF-shaped byte streams. Real layout
// (FINDINGS.md 2026-07-20 X-OCN probes): header partition pack, header
// metadata local sets, then per-frame essence triplets where the picture
// element is tens of MB and the small per-frame RDD-18 metadata element
// carries the ASCII CameraAttributes string ("MPC-3626 0010119 Version3.00").
// ---------------------------------------------------------------------------

const UL_PREFIX = [0x06, 0x0e, 0x2b, 0x34]
// Header partition pack key (ClosedComplete), as at offset 0 of the corpus files.
const PARTITION_KEY = [
  ...UL_PREFIX,
  0x02,
  0x05,
  0x01,
  0x01,
  0x0d,
  0x01,
  0x02,
  0x01,
  0x01,
  0x02,
  0x04,
  0x00,
]
// Generic-container picture essence element key (frame-wrapped X-OCN).
const PICTURE_KEY = [
  ...UL_PREFIX,
  0x01,
  0x02,
  0x01,
  0x01,
  0x0d,
  0x01,
  0x03,
  0x01,
  0x15,
  0x01,
  0x05,
  0x01,
]
// Generic-container data element key (the RDD-18/ANC metadata element).
const DATA_KEY = [
  ...UL_PREFIX,
  0x01,
  0x02,
  0x01,
  0x01,
  0x0d,
  0x01,
  0x03,
  0x01,
  0x17,
  0x01,
  0x02,
  0x01,
]

function berLength(n: number): number[] {
  if (n < 0x80) return [n]
  const bytes: number[] = []
  let v = n
  while (v > 0) {
    bytes.unshift(v & 0xff)
    v = Math.floor(v / 256)
  }
  return [0x80 | bytes.length, ...bytes]
}

function klv(key: number[], value: number[]): number[] {
  if (key.length !== 16) throw new Error('KLV key must be 16 bytes')
  return [...key, ...berLength(value.length), ...value]
}

// A KLV header declaring a value length with NO value bytes materialized —
// proves the walker jumps by declared length instead of reading the body.
function klvHeaderOnly(key: number[], declaredLength: number): number[] {
  return [...key, ...berLength(declaredLength)]
}

function ascii(s: string): number[] {
  return [...s].map((c) => c.charCodeAt(0))
}

// RDD-18-ish metadata value: binary junk around the CameraAttributes string,
// mirroring the real files where the string sits mid-payload between KLV
// local-set tags.
function metadataValueWith(attrs: string): number[] {
  return [0x43, 0x00, 0x25, 0x00, 0x00, 0x1c, ...ascii(attrs), 0x00, 0x00, 0x43, 0x11, 0x00]
}

function fakeBlobOf(bytes: number[]): BlobLike {
  const arr = Uint8Array.from(bytes)
  function make(offset: number, length: number): BlobLike {
    return {
      size: length,
      slice(start = 0, end = length) {
        const from = offset + Math.max(0, start)
        const to = offset + Math.min(end, length)
        if (from > arr.length || to > arr.length) {
          throw new Error(`fakeBlobOf: slice(${start}, ${end}) reads beyond materialized bytes`)
        }
        return make(from, Math.max(0, to - from))
      },
      async arrayBuffer() {
        return arr.slice(offset, offset + length).buffer
      },
    }
  }
  return make(0, arr.length)
}

// A minimal Sony-shaped MXF: partition pack, one small header set, a picture
// element (small here; the huge-jump case gets its own test), audio, then the
// metadata element carrying the model string.
function sonyMxfWith(attrs: string, opts?: { pictureBytes?: number[] }): BlobLike {
  return fakeBlobOf([
    ...klv(PARTITION_KEY, new Array(88).fill(0)),
    ...klv(PICTURE_KEY, opts?.pictureBytes ?? new Array(200).fill(0xaa)),
    ...klv(DATA_KEY, metadataValueWith(attrs)),
  ])
}

describe('sonyCameraDisplayName', () => {
  test('maps corpus-verified MPC codes to model names', () => {
    expect(sonyCameraDisplayName('MPC-2610 1000389 Version1.10')).toBe('BURANO')
    expect(sonyCameraDisplayName('MPC-3610 0010658 Version0.60')).toBe('VENICE')
    expect(sonyCameraDisplayName('MPC-3626 0010119 Version3.00')).toBe('VENICE 2')
  })

  test('unknown MPC codes fall back to the honest raw code', () => {
    expect(sonyCameraDisplayName('MPC-9999 0000001 Version9.99')).toBe('Sony MPC-9999')
  })

  test('reads modelName XML attributes (FX6-style embedded XML)', () => {
    // Corpus FX6 XAVC MXF embeds NRT-style XML in the header:
    // manufacturer="Sony" modelName="ILME-FX6V" serialNo="4000254"
    expect(
      sonyCameraDisplayName('manufacturer="Sony" modelName="ILME-FX6V" serialNo="4000254"'),
    ).toBe('FX6')
  })

  test('unknown modelName values fall back to the honest raw code', () => {
    expect(sonyCameraDisplayName('modelName="ILME-FX99" serialNo="1"')).toBe('Sony ILME-FX99')
  })

  test('non-camera strings yield nothing', () => {
    expect(sonyCameraDisplayName('Version3.00 only')).toBeUndefined()
    expect(sonyCameraDisplayName('')).toBeUndefined()
    // "modelName" without a sane quoted value must not match.
    expect(sonyCameraDisplayName('modelName="" x')).toBeUndefined()
  })
})

describe('extractSonyMxfCameraModel', () => {
  test('finds the model in the per-frame metadata element', async () => {
    const model = await extractSonyMxfCameraModel(sonyMxfWith('MPC-3626 0010119 Version3.00'))
    expect(model).toBe('VENICE 2')
  })

  test('finds BURANO and VENICE codes the same way', async () => {
    expect(await extractSonyMxfCameraModel(sonyMxfWith('MPC-2610 1000389 Version1.10'))).toBe(
      'BURANO',
    )
    expect(await extractSonyMxfCameraModel(sonyMxfWith('MPC-3610 0010658 Version0.60'))).toBe(
      'VENICE',
    )
  })

  test('long-form BER lengths are handled (values >127 bytes)', async () => {
    // metadataValueWith is small; force a long-form value by padding.
    const padded = [...metadataValueWith('MPC-3626 0010119 Version3.00'), ...new Array(300).fill(0)]
    const blob = fakeBlobOf([
      ...klv(PARTITION_KEY, new Array(88).fill(0)),
      ...klv(DATA_KEY, padded),
    ])
    expect(await extractSonyMxfCameraModel(blob)).toBe('VENICE 2')
  })

  test('jumps a huge picture element by declared length without reading it', async () => {
    // Picture element declares ~19 MB but NO body bytes exist — reading it
    // would throw in fakeBlobOf. The walker must skip by length; the walk then
    // runs past end-of-blob and finds nothing.
    const blob = fakeBlobOf([
      ...klv(PARTITION_KEY, new Array(88).fill(0)),
      ...klvHeaderOnly(PICTURE_KEY, 19_000_000),
    ])
    expect(await extractSonyMxfCameraModel(blob)).toBeUndefined()
  })

  test('huge declared values are skipped, later small values still scanned', async () => {
    // A large-but-materialized value over the read cap must be jumped, and the
    // metadata element after it still found.
    const bigValue = new Array(2_000_000).fill(0x55)
    const blob = fakeBlobOf([
      ...klv(PARTITION_KEY, new Array(88).fill(0)),
      ...klv(PICTURE_KEY, bigValue),
      ...klv(DATA_KEY, metadataValueWith('MPC-2610 1000389 Version1.10')),
    ])
    expect(await extractSonyMxfCameraModel(blob)).toBe('BURANO')
  })

  test('non-MXF bytes yield nothing', async () => {
    expect(
      await extractSonyMxfCameraModel(fakeBlobOf(ascii('RIFFnot-an-mxf-file'))),
    ).toBeUndefined()
    expect(await extractSonyMxfCameraModel(fakeBlobOf([]))).toBeUndefined()
  })

  test('an MXF without any camera string yields nothing', async () => {
    const blob = fakeBlobOf([
      ...klv(PARTITION_KEY, new Array(88).fill(0)),
      ...klv(DATA_KEY, [0x00, 0x01, 0x02, 0x03]),
    ])
    expect(await extractSonyMxfCameraModel(blob)).toBeUndefined()
  })

  test('finds the model in FX6-style embedded XML header values', async () => {
    const xml = '<Device manufacturer="Sony" modelName="ILME-FX6V" serialNo="4000254">'
    const blob = fakeBlobOf([
      ...klv(PARTITION_KEY, new Array(88).fill(0)),
      ...klv(DATA_KEY, ascii(xml)),
    ])
    expect(await extractSonyMxfCameraModel(blob)).toBe('FX6')
  })

  test('a string split across two scanned values is not required — first whole match wins', async () => {
    // Two metadata elements; the model string sits whole in the second.
    const blob = fakeBlobOf([
      ...klv(PARTITION_KEY, new Array(88).fill(0)),
      ...klv(DATA_KEY, [0x00, 0x01]),
      ...klv(DATA_KEY, metadataValueWith('MPC-3610 0010658 Version0.60')),
    ])
    expect(await extractSonyMxfCameraModel(blob)).toBe('VENICE')
  })

  test('malformed KLV (non-UL key) stops the walk safely', async () => {
    const blob = fakeBlobOf([
      ...klv(PARTITION_KEY, new Array(88).fill(0)),
      // Garbage where a key should be:
      ...ascii('XXXXGARBAGEXXXXX'),
      ...berLength(4),
      0,
      0,
      0,
      0,
    ])
    expect(await extractSonyMxfCameraModel(blob)).toBeUndefined()
  })
})
