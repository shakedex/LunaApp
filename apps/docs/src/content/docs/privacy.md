---
title: Privacy
description: Why your footage never leaves your device.
---

**Your footage never leaves your device.** Luna reads your media locally through the File
System Access API and processes everything client-side. It does not upload footage, metadata,
or reports to any server.

## What Luna sends about your media: nothing

Luna has no analytics, no telemetry, and makes no network requests carrying your data. The
only network traffic is fetching the program itself:

- The **app bundle** (HTML, JavaScript, CSS), served like any website.
- The **WebAssembly engines** that decode media and read metadata, fetched from a public CDN
  (jsDelivr) the first time they're needed.

Neither carries anything about your footage — they are the same files for every visitor.

## First visit, then offline

On your first visit, Luna downloads its WebAssembly engines — the FFmpeg core (~31 MB) and the
MediaInfo module. The browser caches them, so later visits are instant and Luna keeps working
**offline**. Your media is read straight from disk on every run and held only in your browser's
memory while a report is being built.

:::note
Reports are generated in your browser and saved by you to a location you choose. Luna never
sees them.
:::

Read next: [Supported formats](/docs/supported-formats/) · [Limitations](/docs/limitations/)
