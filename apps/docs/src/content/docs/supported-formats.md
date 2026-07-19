---
title: Supported formats
description: Which containers and RAW formats Luna handles, and how each gets a thumbnail.
---

Luna surfaces **every** file on a card. A file is a file: its name, full path, and size are
always recorded and counted in the report totals, whether or not Luna can paint a thumbnail for
it. Most clips also get metadata and evenly-spaced thumbnails.

## How each format is handled

Luna decides how to produce a thumbnail from the file's extension (and, for ProRes RAW, its
codec):

| Handling | Formats |
| --- | --- |
| **Hardware-decoded thumbnails** (WebCodecs) | `.mov` · `.mp4` · `.m4v` · `.mkv` · `.webm` · `.3gp` |
| **Software-decoded thumbnails** (FFmpeg WASM — slower) | `.mxf` · `.avi` · `.mts` · `.m2ts` · `.wmv` · `.flv` |
| **Embedded / sidecar preview frame** | `.crm` · `.r3d` · ProRes RAW `.mov` |
| **Metadata + placeholder thumbnail** | `.braw` · `.ari` |

ProRes clips decode through a dedicated high-performance path. **ProRes RAW** is detected by its
codec and uses its embedded preview frame rather than a full decode.

## RAW formats are first-class clips

Blackmagic RAW (`.braw`), REDCODE (`.r3d`), Canon Cinema RAW Light (`.crm`), and ARRIRAW
(`.ari`) are treated as full clips, not second-class citizens:

- **Metadata** is extracted wherever the format carries it.
- **`.crm` and `.r3d`** show a real reference frame from the file's embedded preview or its
  `.rtn` sidecar when present.
- **`.braw` and `.ari`** show an honest placeholder thumbnail — the browser can't paint a frame
  from them without the vendor SDK — while still appearing in their reel with full name, path,
  size, and any available metadata.

These files are never labelled "undecodable" or dropped from the report.

## Metadata enrichment

On top of the container's generic metadata, Luna applies vendor-aware enrichment for common
camera families — ARRI (QuickTime and MXF), Sony and Canon acquisition metadata, Blackmagic
RAW, and Panasonic. Camera fields (ISO, white balance, lens, and so on) appear only when the
file actually carries them.

:::note[Source of truth]
Format handling is defined in the app's core: the supported-extension allowlist in
`packages/core/src/media/extensions.ts`, the thumbnail routing in
`packages/core/src/thumbs/router.ts`, and the vendor enrichers in
`packages/core/src/metadata/vendors/registry.ts`.
:::

Read next: [Limitations](/docs/limitations/)
