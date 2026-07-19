---
title: Supported formats
description: Formats Luna reads and what you get for each.
---

Luna lists every file in the folder in the report — name, path, and size — and adds metadata
and a thumbnail where it can. What you get depends on the format:

| Format | Thumbnail | Metadata |
| --- | --- | --- |
| MOV, MP4, M4V, MKV, WebM, 3GP — H.264, HEVC, ProRes, VP9, AV1 | Decoded | Yes |
| MXF, AVI, MTS, M2TS, WMV, FLV | Decoded (software — slower) | Yes |
| Canon Cinema RAW Light (CRM), REDCODE (R3D), ProRes RAW | Embedded preview frame | Yes |
| Blackmagic RAW (BRAW), ARRIRAW (ARI) | Placeholder | Yes |

Thumbnails come from the browser's hardware decoders where possible, and from a software decoder
(FFmpeg) for the older containers, which is slower. CRM and R3D use the preview frame the file —
or its `.rtn` sidecar — already carries, and ProRes RAW does the same. BRAW and ARI can't be
decoded to a frame in the browser without the manufacturer's SDK, so they get a placeholder.

Metadata is read from the container, and enriched with vendor-specific fields for ARRI, Sony,
Canon, Blackmagic, and Panasonic files. A field only appears when the file actually stores it.
