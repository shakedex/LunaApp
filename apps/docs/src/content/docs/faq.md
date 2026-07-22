---
title: FAQ
description: Common questions, short answers.
---

## Which browsers work?

Chrome, Edge, Brave, and Arc. Luna needs the File System Access API, which Firefox and Safari
don't support yet. See [Requirements](/docs/requirements/).

## Does it work on my phone or tablet?

No — desktop and laptop only. See [Requirements](/docs/requirements/).

## Does my footage get uploaded?

No. Everything runs in your browser; footage never leaves your device. See [Privacy](/docs/privacy/).

## Why do some RAW clips show a placeholder instead of a frame?

BRAW and ARI can't be decoded in the browser without the manufacturer's SDK, so they get a
placeholder. CRM and R3D show the preview frame the file already carries. All of them still get
full metadata. See [Supported formats](/docs/supported-formats/).

## Why is a large MXF card slow?

MXF and other older containers decode in software instead of on your GPU — it works, just slower
than H.264, HEVC, or ProRes.

## Does it work offline?

Yes, after the first visit. The decoding engines (~31 MB) download once and are then cached.

## Is it free and open source?

Yes, under the MIT License. Source is on [GitHub](https://github.com/shakedex/LunaApp).

## Is it affiliated with ARRI, Blackmagic, Sony, or RED?

No — Luna is an independent project, not affiliated with any camera manufacturer. See the
[Disclaimer](https://github.com/shakedex/LunaApp/blob/master/DISCLAIMER.md).
