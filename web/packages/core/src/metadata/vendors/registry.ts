import type { MediaInfoObjectResult } from '../mediainfo'
import type { ClipMetadata } from '../model'
import { arriMovEnricher } from './arri-mov'
import type { VendorEnricher } from './types'

export const vendorEnrichers: readonly VendorEnricher[] = [arriMovEnricher]

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
