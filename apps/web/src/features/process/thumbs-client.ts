import type { ThumbnailFrame } from '@luna-web/core'
import * as Comlink from 'comlink'

export interface ThumbsWorkerApi {
  // Returns one frame per requested timestamp. Never rejects for per-frame
  // problems — outcomes carry them. Rejects only on container-open failure
  // (mapped by the caller to ContainerOpenFailed) or undecodable track
  // (thrown as Error with message 'NO_DECODER', mapped to NoDecoder).
  thumbnails(file: File, timestamps: number[], width: number): Promise<ThumbnailFrame<Blob>[]>
}

export interface ThumbsWorkerHandle {
  api: Comlink.Remote<ThumbsWorkerApi>
  worker: Worker
}

export function createThumbsWorker(): ThumbsWorkerHandle {
  const worker = new Worker(new URL('../../workers/thumbs.worker.ts', import.meta.url), {
    type: 'module',
  })
  return { api: Comlink.wrap<ThumbsWorkerApi>(worker), worker }
}
