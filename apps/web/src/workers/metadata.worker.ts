/// <reference lib="webworker" />
import * as Comlink from 'comlink'
import mediaInfoFactory, { type MediaInfo } from 'mediainfo.js'
import wasmUrl from 'mediainfo.js/MediaInfoModule.wasm?url'

// One MediaInfo instance per worker, reused sequentially — the pool assigns
// one clip at a time per worker, so no concurrent analyzeData on an instance.
let miPromise: Promise<MediaInfo<'object'>> | null = null

function getMediaInfo(): Promise<MediaInfo<'object'>> {
  miPromise ??= mediaInfoFactory({
    format: 'object',
    locateFile: (path: string) => (path.endsWith('.wasm') ? wasmUrl : path),
  })
  return miPromise
}

const api = {
  async analyze(file: File): Promise<unknown> {
    const mediainfo = await getMediaInfo()
    const readChunk = async (chunkSize: number, offset: number) =>
      new Uint8Array(await file.slice(offset, offset + chunkSize).arrayBuffer())
    return await mediainfo.analyzeData(file.size, readChunk)
  },
}

Comlink.expose(api)
