import type { ClipMetadata, ClipRef, RawNotice, ScanSummary } from '@luna-web/core'
import { Store } from '@tanstack/store'

export type ScanPhase = 'idle' | 'scanning' | 'summary' | 'processing' | 'processed' | 'error'

export type ClipProcessStatus = 'queued' | 'processing' | 'done' | 'failed'

export interface ScanState {
  phase: ScanPhase
  sourceName: string | null
  progress: { filesSeen: number; clipsFound: number } | null
  clips: ClipRef[]
  raw: RawNotice[]
  summary: ScanSummary | null
  error: string | null
  clipStatus: Record<string, ClipProcessStatus>
  metadataById: Record<string, ClipMetadata>
  clipErrors: Record<string, string>
  processedCount: number
}

export const initialScanState: ScanState = {
  phase: 'idle',
  sourceName: null,
  progress: null,
  clips: [],
  raw: [],
  summary: null,
  error: null,
  clipStatus: {},
  metadataById: {},
  clipErrors: {},
  processedCount: 0,
}

export const scanStore = new Store<ScanState>(initialScanState)
