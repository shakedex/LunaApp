---
title: Limitations
description: Browser support, speed, and RAW thumbnails.
---

**Chromium on a desktop or laptop.** Chrome, Edge, Brave, and Arc — not Firefox or Safari, and
not phones or tablets. See [Requirements](/docs/requirements/).

**Older containers decode slowly.** MXF, DNxHD, and similar formats decode in software rather
than on your GPU, so large cards take longer.

**BRAW and ARI have no thumbnail.** They can't be decoded to a frame in the browser, so you get
a placeholder plus full metadata. CRM and R3D show their embedded preview.

**Metadata varies by format.** A field only appears when the file records it.

**First load downloads ~31 MB** of decoding engines, cached afterward for offline use.
