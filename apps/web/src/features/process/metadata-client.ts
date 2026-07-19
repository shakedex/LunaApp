import type { MediaInfoObjectResult } from '@luna-web/core'
import * as Comlink from 'comlink'

export interface MetadataWorkerApi {
  // Typed structured-clone boundary: the worker returns mediainfo.js's raw
  // object-format result, which is this shape by construction.
  analyze(file: File): Promise<MediaInfoObjectResult>
}

export interface MetadataWorkerHandle {
  api: Comlink.Remote<MetadataWorkerApi>
  worker: Worker
}

export function createMetadataWorker(): MetadataWorkerHandle {
  const worker = new Worker(new URL('../../workers/metadata.worker.ts', import.meta.url), {
    type: 'module',
  })
  return { api: Comlink.wrap<MetadataWorkerApi>(worker), worker }
}
