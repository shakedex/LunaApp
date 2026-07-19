import type {
  ClipMetadata,
  ClipRef,
  OtherFileRef,
  ScanSummary,
  ThumbnailFrame,
} from '@luna-web/core'
import { Store } from '@tanstack/store'

export type ScanPhase =
  | 'idle'
  | 'scanning'
  | 'summary'
  | 'processing'
  | 'thumbnailing'
  | 'processed'
  | 'error'

export type ClipProcessStatus = 'queued' | 'processing' | 'done' | 'failed'

export type ThumbStatus = 'queued' | 'decoding' | 'done' | 'failed'

export interface ScanState {
  phase: ScanPhase
  sourceName: string | null
  progress: { filesSeen: number; clipsFound: number } | null
  clips: ClipRef[]
  otherFiles: OtherFileRef[]
  summary: ScanSummary | null
  error: string | null
  clipStatus: Record<string, ClipProcessStatus>
  metadataById: Record<string, ClipMetadata>
  clipErrors: Record<string, string>
  processedCount: number
  thumbStatus: Record<string, ThumbStatus>
  thumbsById: Record<string, ThumbnailFrame<Blob>[]>
  thumbErrors: Record<string, string>
  thumbDoneCount: number
}

export const initialScanState: ScanState = {
  phase: 'idle',
  sourceName: null,
  progress: null,
  clips: [],
  otherFiles: [],
  summary: null,
  error: null,
  clipStatus: {},
  metadataById: {},
  clipErrors: {},
  processedCount: 0,
  thumbStatus: {},
  thumbsById: {},
  thumbErrors: {},
  thumbDoneCount: 0,
}

export const scanStore = new Store<ScanState>(initialScanState)
