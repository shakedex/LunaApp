import type { MediaInfoObjectResult } from '../mediainfo'
import type { ClipMetadata } from '../model'
import { acquisitionEnricher } from './acquisition'
import { arriMovEnricher } from './arri-mov'
import { arriMxfEnricher } from './arri-mxf'
import { brawEnricher } from './braw'
import { panasonicEnricher } from './panasonic'
import type { VendorEnricher } from './types'

export const vendorEnrichers: readonly VendorEnricher[] = [
  arriMovEnricher,
  arriMxfEnricher,
  acquisitionEnricher,
  brawEnricher,
  panasonicEnricher,
]

export function applyVendorEnrichment(
  result: MediaInfoObjectResult,
  base: ClipMetadata,
): ClipMetadata {
  for (const enricher of vendorEnrichers) {
    if (enricher.detect(result)) {
      return enricher.enrich(result, base)
    }
  }
  return base
}
