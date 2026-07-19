---
title: Privacy
description: How local processing works.
---

Luna reads your footage directly from disk with the File System Access API and processes it in
your browser. It never uploads footage, metadata, or reports, and has no analytics or telemetry.

The only things it downloads are the app itself and its decoding engines — FFmpeg and MediaInfo,
about 31 MB, fetched once from a CDN and then cached. After that first load, Luna works offline.

Reports are saved to a location you choose. Luna keeps nothing.
