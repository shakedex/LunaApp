---
title: Overview
description: What Luna is, who it's for, and how it turns a camera card into a report.
---

Luna is a camera-report tool that runs entirely in your browser. Point it at a folder of
footage and it reads the card locally, groups clips into reels, extracts metadata and
thumbnails, and produces a clean, shareable report — with no upload, no install, and no
account.

## Who it's for

Luna is built for DITs (digital imaging technicians) and camera assistants who need to hand
off an accurate account of what's on a card: every file, its technical metadata, and a visual
reference frame where one can be produced.

## How a report comes together

1. **Pick a folder.** Luna asks for read access to a folder through your browser's File System
   Access API. Nothing is uploaded — the permission is local to your machine.
2. **Scan.** Luna walks the folder, lists every clip, and shows a pre-scan summary (clip count
   and total size) so you can confirm before any heavy work starts.
3. **Process.** For each clip, Luna extracts metadata and renders evenly-spaced thumbnails,
   running several clips in parallel. Progress and per-clip outcomes appear live.
4. **Reels.** Clips are grouped into reels by their recorded reel name, falling back to the
   top-level folder.
5. **Export.** Fill in the cover and branding fields, then export a **PDF** and/or **CSV**
   camera report. You save the file locally.

## What's in a report

A report lists each reel and the clips within it, with per-clip technical metadata (camera,
lens, ISO, shutter, color space, frame rate, duration, and more, wherever the file carries it),
thumbnails where Luna can produce them, and totals — clip counts and byte-accurate sizes for
every file on the card.

Read next: [Privacy](/docs/privacy/) · [Supported formats](/docs/supported-formats/)
