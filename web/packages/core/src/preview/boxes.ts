// ISO-BMFF / QuickTime top-level box walker, ported from the seek-only recipe
// verified in `tools/box-offsets.mjs` (FINDINGS.md "Browser box-reader
// recipe"). Reads only box headers (8/16/32 bytes) and jumps by declared
// size — it must never slice a box's body, since bodies (mdat) can be
// multi-gigabyte. Callers that need a payload (e.g. a preview JPEG inside a
// small leaf box) slice it themselves once they hold a validated header.

import type { BlobLike } from '../scan/handles'

export interface BoxHeader {
  type: string
  start: number
  size: number
  headerSize: number
  uuid?: Uint8Array
}

const BASE_HEADER_SIZE = 8
const LARGESIZE_HEADER_SIZE = 16
const UUID_ID_SIZE = 16

/**
 * Reads a single box header at `offset`: the 8-byte size+type, expanded to a
 * 64-bit `largesize` when size===1, to end-of-file when size===0, and to a
 * `uuid`-carrying header (extra 16-byte id) when type is 'uuid'. Returns null
 * for anything malformed or out of bounds — never throws.
 */
export async function readBoxHeaderAt(blob: BlobLike, offset: number): Promise<BoxHeader | null> {
  if (offset < 0 || offset + BASE_HEADER_SIZE > blob.size) return null

  const headerBytes = new Uint8Array(
    await blob.slice(offset, offset + BASE_HEADER_SIZE).arrayBuffer(),
  )
  if (headerBytes.length < BASE_HEADER_SIZE) return null
  const headerView = new DataView(
    headerBytes.buffer,
    headerBytes.byteOffset,
    headerBytes.byteLength,
  )

  let size = headerView.getUint32(0)
  const type = String.fromCharCode(headerBytes[4], headerBytes[5], headerBytes[6], headerBytes[7])
  let headerSize = BASE_HEADER_SIZE

  if (size === 1) {
    if (offset + LARGESIZE_HEADER_SIZE > blob.size) return null
    const largeBytes = new Uint8Array(
      await blob.slice(offset + BASE_HEADER_SIZE, offset + LARGESIZE_HEADER_SIZE).arrayBuffer(),
    )
    if (largeBytes.length < 8) return null
    const largeView = new DataView(largeBytes.buffer, largeBytes.byteOffset, largeBytes.byteLength)
    const largeSize = largeView.getBigUint64(0)
    if (largeSize > BigInt(Number.MAX_SAFE_INTEGER)) return null
    size = Number(largeSize)
    headerSize = LARGESIZE_HEADER_SIZE
  } else if (size === 0) {
    size = blob.size - offset
  }

  if (size < headerSize) return null

  const header: BoxHeader = { type, start: offset, size, headerSize }

  if (type === 'uuid') {
    const uuidEnd = offset + headerSize + UUID_ID_SIZE
    if (uuidEnd > blob.size || offset + size < uuidEnd) return null
    const uuidBytes = new Uint8Array(await blob.slice(offset + headerSize, uuidEnd).arrayBuffer())
    if (uuidBytes.length < UUID_ID_SIZE) return null
    header.uuid = uuidBytes
    header.headerSize = headerSize + UUID_ID_SIZE
  }

  return header
}

/**
 * Walks top-level boxes from the start of the file, jumping by each box's
 * declared size. Never slices a box body — mdat safety lives here. `visit`
 * controls whether the walk continues or stops early.
 */
export async function walkTopLevelBoxes(
  blob: BlobLike,
  visit: (box: BoxHeader) => 'continue' | 'stop' | Promise<'continue' | 'stop'>,
): Promise<void> {
  let offset = 0
  while (offset + BASE_HEADER_SIZE <= blob.size) {
    const header = await readBoxHeaderAt(blob, offset)
    if (!header) return
    const result = await visit(header)
    if (result === 'stop') return
    offset += header.size
  }
}

/**
 * Walks a container box's children (its payload runs from
 * `container.start + container.headerSize` to `container.start +
 * container.size`) looking for the first child of `childType`.
 */
export async function findChildBox(
  blob: BlobLike,
  container: BoxHeader,
  childType: string,
): Promise<BoxHeader | null> {
  const end = container.start + container.size
  let offset = container.start + container.headerSize
  while (offset + BASE_HEADER_SIZE <= end) {
    const header = await readBoxHeaderAt(blob, offset)
    if (!header) return null
    if (header.type === childType) return header
    offset += header.size
  }
  return null
}
