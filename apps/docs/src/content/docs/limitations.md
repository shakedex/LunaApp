---
title: Limitations
description: Browser support, speed, and RAW thumbnails.
---

**Chromium only.** Luna needs the File System Access API and WebCodecs, so it runs in Chrome,
Edge, Brave, and Arc — not Firefox or Safari.

**Older containers decode slowly.** MXF, DNxHD, and similar formats decode in software rather
than on your GPU, so large cards take longer.

**BRAW and ARI have no thumbnail.** They can't be decoded to a frame in the browser, so you get
a placeholder plus full metadata. CRM and R3D show their embedded preview.

**Metadata varies by format.** A field only appears when the file records it.

**First load downloads ~31 MB** of decoding engines, cached afterward for offline use.
