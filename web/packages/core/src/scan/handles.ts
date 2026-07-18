// Structural mirrors of the File System Access API, so the walker stays
// DOM-free and testable. The real FileSystemDirectoryHandle satisfies this
// shape at runtime; the app casts once at the boundary.

// Structural subset of the DOM Blob/File the pipeline relies on. mediainfo.js
// reads chunks via slice().arrayBuffer(); later plans (ffmpeg WORKERFS mount,
// WebCodecs demux) consume the same surface. Core never names DOM types.
export interface BlobLike {
  readonly size: number
  slice(start?: number, end?: number): BlobLike
  arrayBuffer(): Promise<ArrayBuffer>
}

export interface FileHandleLike {
  readonly kind: 'file'
  readonly name: string
  getFile(): Promise<BlobLike>
}

export interface DirectoryHandleLike {
  readonly kind: 'directory'
  readonly name: string
  entries(): AsyncIterable<readonly [string, FileSystemEntryLike]>
}

export type FileSystemEntryLike = FileHandleLike | DirectoryHandleLike
