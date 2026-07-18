// Structural mirrors of the File System Access API, so the walker stays
// DOM-free and testable. The real FileSystemDirectoryHandle satisfies this
// shape at runtime; the app casts once at the boundary.
export interface FileHandleLike {
  readonly kind: 'file'
  readonly name: string
  getFile(): Promise<{ readonly size: number }>
}

export interface DirectoryHandleLike {
  readonly kind: 'directory'
  readonly name: string
  entries(): AsyncIterable<readonly [string, FileSystemEntryLike]>
}

export type FileSystemEntryLike = FileHandleLike | DirectoryHandleLike
