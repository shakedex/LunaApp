import type { ClipRef, RawNotice, ScanSummary } from '@luna-web/core'
import { Store } from '@tanstack/store'

export type ScanPhase = 'idle' | 'scanning' | 'summary' | 'confirmed' | 'error'

export interface ScanState {
  phase: ScanPhase
  sourceName: string | null
  progress: { filesSeen: number; clipsFound: number } | null
  clips: ClipRef[]
  raw: RawNotice[]
  summary: ScanSummary | null
  error: string | null
}

export const initialScanState: ScanState = {
  phase: 'idle',
  sourceName: null,
  progress: null,
  clips: [],
  raw: [],
  summary: null,
  error: null,
}

export const scanStore = new Store<ScanState>(initialScanState)
