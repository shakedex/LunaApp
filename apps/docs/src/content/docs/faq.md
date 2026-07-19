---
title: FAQ
description: Quick answers to the most common questions about Luna.
---

## Which browsers work?

Chromium-based browsers — Chrome, Edge, Brave, and Arc. Luna needs the File System Access API
and WebCodecs, which Firefox and Safari don't fully support yet. See
[Limitations](/docs/limitations/).

## Does my footage get uploaded anywhere?

No. Everything runs in your browser and your media never leaves your device. The only network
requests fetch the program itself. See [Privacy](/docs/privacy/).

## Why do some RAW clips show a placeholder instead of a frame?

`.braw` and `.ari` can't be decoded to a frame in the browser without the camera vendor's SDK,
so they show a placeholder. `.crm` and `.r3d` show an embedded or sidecar preview when the file
has one. Either way the clip is fully listed with its metadata, path, and size. See
[Supported formats](/docs/supported-formats/).

## Why is a large MXF card slow?

MXF and other legacy containers decode in software (FFmpeg WASM) rather than on your machine's
hardware video decoders. It works, but it's slower than the hardware path used for H.264, HEVC,
and ProRes.

## Does Luna work offline?

Yes, after the first visit. Luna downloads its WebAssembly engines once (~31 MB), the browser
caches them, and later runs work without a connection.

## Is Luna free and open-source?

Yes. Luna is licensed under the Apache License 2.0. The source is on
[GitHub](https://github.com/shakedex/LunaApp).

## Is Luna affiliated with ARRI, Blackmagic, Sony, or RED?

No. Luna is an independent open-source project and is not affiliated with, endorsed by, or
sponsored by any camera manufacturer. All product names and trademarks belong to their
respective owners. See the full
[Disclaimer](https://github.com/shakedex/LunaApp/blob/master/DISCLAIMER.md).
