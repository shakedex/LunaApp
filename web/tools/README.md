# web/tools — metadata diagnostics

Dev/debug tooling for Luna Web's metadata parser. **Not shipped** in the app
bundle — kept in-repo so we can inspect real camera files, keep compatibility,
and extend support for more cameras/models as we encounter them.

## `analyze-clips.mjs`

Drives the **same** `mediainfo.js` the app's metadata worker uses and the
**same** [`mapMediaInfoToClipMetadata`](../packages/core/src/metadata/mediainfo.ts)
mapper from `@luna-web/core`, so "what mediainfo reads" and "what Luna currently
keeps" sit side by side. Also surfaces sidecars (Sony `.xml`, Avid `.ale`/`.bin`)
sitting next to each clip.

```sh
cd web
bun tools/analyze-clips.mjs                        # default corpora
bun tools/analyze-clips.mjs "D:/path/to/CAMERA"     # a folder (walked recursively)
bun tools/analyze-clips.mjs clipA.mxf clipB.mov     # explicit files
bun tools/analyze-clips.mjs --out some/dir ...       # override output dir
```

With no arguments it scans the default on-set corpora (`DEFAULT_ROOTS` at the
top of the script) — edit those paths for your machine.

### Outputs (`tools/out/`, git-ignored)

- `<clip>.json` — full mediainfo payload + current `ClipMetadata` mapping +
  sidecar text, per clip.
- `_schema.json` — machine-readable union of every tag key seen, per track type.
- `_schema.md` — human-readable schema + coverage tables.

`out/` is regenerable, so it's git-ignored; commit distilled conclusions to
`FINDINGS.md` instead.

## `probe-exiftool.mjs`

Companion for formats mediainfo can't fully read — notably Canon Cinema RAW
Light `.crm`, whose `CRAW` frames aren't ffmpeg-decodable and whose camera
metadata mediainfo doesn't surface. Uses `exiftool-vendored` to read vendor
maker-note metadata (ISO/WB/lens/model/shutter, normalized across vendors) and
to extract the **embedded preview/thumbnail JPEG** from a RAW container without
debayering.

```sh
cd web
bun tools/probe-exiftool.mjs                     # default: the S001 .crm
bun tools/probe-exiftool.mjs path/to/clip.crm    # explicit files
bun tools/probe-exiftool.mjs --out some/dir ...    # override output dir
```

Outputs to `tools/out/exiftool/` (git-ignored): `<clip>.exiftool.json` (every
tag) and `<clip>.<Tag>.jpg` (each extractable embedded image). Verified on the
Canon `.crm`: full camera block + a 2048×1080 preview — see `FINDINGS.md`.

Note: exiftool is a native/Perl binary, so this is a Node/desktop path — it does
not run in the browser.

## `box-offsets.mjs`

ISO-BMFF / QuickTime box walker. Maps a container's box tree (seek-only — never
loads `mdat` or the multi-GB essence) and pinpoints every embedded preview /
thumbnail JPEG with its absolute file offset, length, and pixel dimensions.
Used to derive the exact byte offsets a browser box-reader targets — no
dependency, no decode.

```sh
cd web
bun tools/box-offsets.mjs clip.crm clip.mov
```

Verified offsets for the Canon `.crm` and DJI ProRes RAW previews are in
`FINDINGS.md`.

## `FINDINGS.md`

Living reference of what the payloads actually contain per camera/format and how
to map them — the headline being that mediainfo already exposes most of the
camera block inside each track's `extra` object. Update it whenever we analyze a
new camera or model so the parser work stays data-driven.

## When you add a new camera/format

1. Drop sample clips somewhere and run `analyze-clips.mjs` against them.
2. Read the new `out/<clip>.json` — look inside `mediainfo.media.track[].extra`.
3. Record the vendor's `extra` shape, field names, and value quirks in
   `FINDINGS.md`.
4. Fold the mapping into `mapMediaInfoToClipMetadata` in `@luna-web/core`.
