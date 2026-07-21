import { describe, expect, test } from 'vite-plus/test'
import {
  type BoxHeader,
  findChildBox,
  readBoxHeaderAt,
  walkTopLevelBoxes,
} from '../src/preview/boxes'
import type { BlobLike } from '../src/scan/handles'

function u32(n: number): number[] {
  return [n >>> 24, (n >>> 16) & 0xff, (n >>> 8) & 0xff, n & 0xff]
}
function u64(n: bigint): number[] {
  const bytes: number[] = []
  for (let shift = 56n; shift >= 0n; shift -= 8n) bytes.push(Number((n >> shift) & 0xffn))
  return bytes
}
function box(type: string, ...payload: number[][]): number[] {
  const body = payload.flat()
  return [...u32(8 + body.length), ...[...type].map((c) => c.charCodeAt(0)), ...body]
}
// A box header declaring size===1 (64-bit largesize) with NO body materialized
// after it — used to prove the walker jumps by declared size instead of
// reading the (potentially multi-gigabyte) body.
function largesizeHeaderOnly(type: string, largeSize: bigint): number[] {
  return [...u32(1), ...[...type].map((c) => c.charCodeAt(0)), ...u64(largeSize)]
}
// A box header declaring size===0 ("runs to end of file").
function sizeZeroHeader(type: string, payload: number[]): number[] {
  return [0, 0, 0, 0, ...[...type].map((c) => c.charCodeAt(0)), ...payload]
}
function fakeBlobOf(bytes: number[]): BlobLike {
  const arr = Uint8Array.from(bytes)
  function make(offset: number, length: number): BlobLike {
    return {
      size: length,
      slice(start = 0, end = length) {
        const from = offset + Math.max(0, start)
        const to = offset + end
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

describe('readBoxHeaderAt', () => {
  test('reads a 32-bit header', async () => {
    const ftyp = box('ftyp', [0x69, 0x73, 0x6f, 0x6d, 0, 0, 2, 0, 0x69, 0x73, 0x6f, 0x6d])
    const blob = fakeBlobOf(ftyp)

    const header = await readBoxHeaderAt(blob, 0)

    expect(header).toEqual({ type: 'ftyp', start: 0, size: ftyp.length, headerSize: 8 })
  })

  test('expands size===1 into a 64-bit largesize', async () => {
    const largeSize = 5_625_818_281n // FINDINGS.md's real DJI Ronin-4D mdat size
    const header = await readBoxHeaderAt(fakeBlobOf(largesizeHeaderOnly('mdat', largeSize)), 0)

    expect(header).toEqual({ type: 'mdat', start: 0, size: Number(largeSize), headerSize: 16 })
  })

  test('treats size===0 as running to end-of-file', async () => {
    const payload = [9, 9, 9, 9, 9]
    const header = await readBoxHeaderAt(fakeBlobOf(sizeZeroHeader('mdat', payload)), 0)

    expect(header).toEqual({ type: 'mdat', start: 0, size: 8 + payload.length, headerSize: 8 })
  })

  test('exposes the 16-byte extended id for a uuid box', async () => {
    const uuidId = Array.from({ length: 16 }, (_, i) => i + 1)
    const payload = [0xaa, 0xbb]
    const blob = fakeBlobOf(box('uuid', uuidId, payload))

    const header = await readBoxHeaderAt(blob, 0)

    expect(header?.type).toBe('uuid')
    expect(header?.uuid).toEqual(Uint8Array.from(uuidId))
    expect(header?.headerSize).toBe(8 + 16)
    expect(header?.size).toBe(8 + uuidId.length + payload.length)
  })

  test('returns null when the header would read past the blob', async () => {
    const blob = fakeBlobOf([0, 0, 0, 20, 0x66, 0x74])
    expect(await readBoxHeaderAt(blob, 0)).toBeNull()
  })
})

describe('walkTopLevelBoxes', () => {
  test('walks 32-bit boxes by jumping over each declared size', async () => {
    const ftyp = box('ftyp', [0x69, 0x73, 0x6f, 0x6d, 0, 0, 2, 0, 0x69, 0x73, 0x6f, 0x6d])
    const free = box('free', [1, 2, 3, 4])
    const blob = fakeBlobOf([...ftyp, ...free])

    const visited: { type: string; start: number; size: number }[] = []
    await walkTopLevelBoxes(blob, (b) => {
      visited.push({ type: b.type, start: b.start, size: b.size })
      return 'continue'
    })

    expect(visited).toEqual([
      { type: 'ftyp', start: 0, size: ftyp.length },
      { type: 'free', start: ftyp.length, size: free.length },
    ])
  })

  test('stops early when visit returns stop', async () => {
    const ftyp = box('ftyp', [1, 2, 3, 4])
    const free = box('free', [5, 6])
    const blob = fakeBlobOf([...ftyp, ...free])

    const visited: string[] = []
    await walkTopLevelBoxes(blob, (b) => {
      visited.push(b.type)
      return 'stop'
    })

    expect(visited).toEqual(['ftyp'])
  })

  test('jumps over a 64-bit largesize mdat without ever reading its body', async () => {
    const ftyp = box('ftyp', [0x69, 0x73, 0x6f, 0x6d, 0, 0, 2, 0, 0x69, 0x73, 0x6f, 0x6d])
    const largeSize = 5_625_818_281n
    const mdatHeader = largesizeHeaderOnly('mdat', largeSize)
    // Only the header is materialized — the blob "ends" right after it, well
    // short of the declared largesize. Any attempt to slice the body throws.
    const blob = fakeBlobOf([...ftyp, ...mdatHeader])

    const visited: BoxHeader[] = []
    await walkTopLevelBoxes(blob, (b) => {
      visited.push(b)
      return 'continue'
    })

    expect(visited.map((b) => b.type)).toEqual(['ftyp', 'mdat'])
    const mdat = visited[1]
    expect(mdat?.size).toBe(Number(largeSize))
    expect(mdat?.headerSize).toBe(16)
    // Confirms the trap actually guards something: reading the declared body
    // really would throw against this fixture.
    expect(() => blob.slice(ftyp.length, ftyp.length + Number(largeSize))).toThrow()
  })
})

describe('findChildBox', () => {
  test('locates udta inside moov', async () => {
    const trak = box('trak', [1, 2, 3, 4])
    const udta = box('udta', [5, 6, 7, 8, 9])
    const blob = fakeBlobOf(box('moov', trak, udta))

    const moov = await readBoxHeaderAt(blob, 0)
    expect(moov).not.toBeNull()

    const found = await findChildBox(blob, moov as BoxHeader, 'udta')
    expect(found?.type).toBe('udta')
    expect(found?.start).toBe(8 + trak.length)

    const missing = await findChildBox(blob, moov as BoxHeader, 'stbl')
    expect(missing).toBeNull()
  })
})
