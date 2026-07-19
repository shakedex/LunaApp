---
title: Limitations
description: Honest constraints — browser support, decode speed, RAW thumbnails, metadata.
---

Luna is built around an honest report. These are its real constraints.

## Chromium-only

Luna relies on the File System Access API and WebCodecs, which today are available in
Chromium-based browsers — Chrome, Edge, Brave, and Arc. Firefox and Safari don't yet expose both
APIs, so Luna doesn't run there.

## Software decode is slower

Clips that go through the FFmpeg WASM path (`.mxf`, DNxHD, and other legacy formats) are
decoded purely in software. Large MXF cards on a slower machine will feel it. Clips on the
hardware path (H.264, HEVC, VP9, AV1, and ProRes) are much faster.

## Some RAW clips show placeholder thumbnails

`.braw` and `.ari` show a placeholder thumbnail — the browser can't decode a frame from them
without the vendor SDK. `.crm` and `.r3d` show an embedded or sidecar preview frame when one is
present. In every case the clip is still fully catalogued: name, path, size, and any metadata
the file carries.

## Metadata depends on the format

Camera fields — ISO, white balance, lens, color space, and the rest — appear only when the
container actually records them. A format that doesn't store a field simply leaves it blank;
Luna never invents values.

## First visit downloads the engines

The first time you use Luna, it downloads its WebAssembly engines (~31 MB). After that they're
cached and Luna runs offline.

:::caution
A "limitation" here never means a file is hidden or dropped. Every file on the card is surfaced
and counted, even when Luna can't paint a thumbnail for it.
:::

Read next: [FAQ](/docs/faq/)
