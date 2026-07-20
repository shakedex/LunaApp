# Backlog: 2026-07-20 audit follow-ups (Foolcat / industry cross-check)

**Origin:** 2026-07-20 product audit — Luna Web capability map crossed against Foolcat
(hedge.co) and the wider DIT tool landscape (Silverstack, OffShoot, YoYotta). Maintainer
triaged the findings; this file records what survived, what's deferred, and what's dead so
none of it gets re-litigated.

---

## Candidate — Report library export/import (portability & backup)

**Maintainer reaction: liked. Strongest candidate for next feature work; needs a design
session before implementation.**

**Problem:** Saved reports live only in IndexedDB (`reportSummaries` + `reportModels`).
A browser profile wipe, storage eviction, or machine migration destroys the report library.
Two real use cases: migrating between systems, and backing up the entire body of work before
wiping for a fresh project.

**Direction:** Export a saved report (or the whole library) to a self-contained file
(JSON, or a zip bundling the JSON model + thumbnail blobs), and import it back on any
machine. Round-trip must preserve everything needed to reopen the report in the workspace
and re-export PDF/CSV.

**Open questions for the design session:** per-report vs whole-library export; file format
(single JSON with base64 thumbs vs zip); schema versioning/migration on import (settings
already has a schemaVersion pattern to follow).

---

## Deferred — future consideration, not now

### Log→Rec.709 thumbnails (+ optional .cube LUT)

Thumbnails of Log footage (LogC3/LogC4, S-Log3) render flat/washed in reports. Foolcat Pro
converts RAW/Log to Rec.709 per vendor spec and accepts custom .cube 3D LUTs. Luna already
detects gamma via the metadata enrichers, so the transform could key off that. WebGL/WebGPU
makes in-browser 3D LUT application tractable. Revisit when report polish matters more than
format coverage.

### Scene/Take metadata fields

Foolcat reads Scene/Take from BRAW. Always a good addition to the camera field set
(`packages/core/src/metadata/fields.ts`) — deferred until a corpus pass makes it cheap to
verify which formats actually carry it.

---

## Decided against — do not re-propose

- **HTML report export.** Luna *is* a web app; print-to-PDF from the browser covers the
  "shareable page" case. PDF + CSV stay the export surface.
- **Checksums / ASC MHL / hash-manifest verification.** Overkill for Luna's scope. Luna
  reports what's on the card; offload verification is a different product.
- **Automation / URL schemes / event scripting** (Foolcat's `foolcat://` + AppleScript/
  Python). Exists to integrate with OffShoot's offload pipeline; meaningless for a browser
  app. Waste of time.

---

## Format coverage — bound by vendor SDKs, path is our own parsing

No vendor SDK is available to an indie browser app (BRAW/R3D SDKs are native-only; ARRI/
Sony gate theirs). The only path is Luna's existing one: empirical byte-level parsing of
real files (`tools/analysis/` → `FINDINGS.md` → enricher + tests). Metadata extraction is
winnable this way; codec reimplementation is not — for frames, the only viable hack is
embedded preview/proxy extraction (proven: `.crm` PRVW, `.r3d` `.rtn` sidecar, ProRes RAW
`moov/udta`), else the honest placeholder.

Pending corpus expansion (samples incoming): `.ari` / `.arx` / `.hde`; X-OCN worth probing
early — it's MXF-wrapped and may already yield metadata through the existing Sony RDD-18
enricher. RED R3D header metadata is the biggest coverage hole; community knowledge of the
header layout exists — probe when samples are in the corpus.
