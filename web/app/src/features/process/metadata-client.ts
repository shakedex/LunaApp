import * as Comlink from 'comlink'

export interface MetadataWorkerApi {
  analyze(file: File): Promise<unknown>
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
