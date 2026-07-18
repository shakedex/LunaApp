# Payload findings — how to build Luna Web's metadata parser in detail

_Generated from `tools/analyze-clips.mjs` over the on-set test corpus
(`D:/LUNA_TEST/TEST_PROJECT_LUNA/CAMERA` + `docs/`), first run 2026-07-18. This
file records what we learned so the parser work is data-driven, not guesswork.
Re-run the tool against new clips as we add camera/model support and update the
tables below — it's a living reference, not a one-off._

## TL;DR

The camera block we declared in [`ClipMetadata`](../packages/core/src/metadata/model.ts)
(`camera, iso, whiteBalance, lens, focalLength, aperture, shutter, gamma`) is
**already present in the mediainfo.js payload for ARRI, Sony X-OCN, and
Blackmagic** — we just don't read it. [`mapMediaInfoToClipMetadata`](../packages/core/src/metadata/mediainfo.ts)
only touches the flat top-level track fields and ignores each track's `extra`
object, which is exactly where MediaInfo parks proprietary camera metadata.

**No new library or vendor tool is needed to fill most of the camera block —
just parse `track.extra`.** Sidecars (Avid `.ale`, Sony `.xml`) matter only for
specific gaps, and some file types genuinely carry nothing.

## Where each field actually lives (three `extra` shapes)

MediaInfo emits proprietary metadata into `extra` differently per vendor:

| Vendor / file | `extra` location | Example keys |
|---|---|---|
| **ARRI ProRes `.mov`** | `General.extra` | `com_arri_camera_CameraModel`, `com_arri_camera_ExposureIndexAsa`, `com_arri_camera_WhiteBalanceKelvin`, `com_arri_camera_ShutterAngle`, `com_arri_camera_ColorGammaSxS`, `com_arri_camera_LensType`, `com_arri_camera_ReelName`, full `ProductionInfo*` slate |
| **Sony X-OCN / Venice `.mxf`** | `Other.extra` (acquisition track) | `ISOSensitivity_FirstFrame`, `WhiteBalance_FirstFrame_String`, `ShutterSpeed_Angle_FirstFrame`, `ShutterSpeed_Time_FirstFrame`, `IrisFNumber_FirstFrame`, `IrisTNumber_FirstFrame`, `LensZoomActualFocalLength_FirstFrame`, `LensAttributes_FirstFrame`, `TransferCharacteristics_FirstFrame` |
| **Blackmagic `.braw`** | `General.extra` | `manufacturer`, `camera_type`, `lens_type`, `reel_name`, `scene`, `take`, `viewing_gamma`, `viewing_gamut`, `shutter_type`, `analog_gain` |

Sony's acquisition fields are per-frame time series exposed as a triple —
`<Field>_FirstFrame`, `<Field>_Values`, `<Field>_FrameCounts`. Take
`_FirstFrame` (or the `_String` variant for display); the `_Values` list only
matters if we ever want "ISO ramped 800→1600 mid-clip".

## Per-format coverage matrix (what the corpus proved)

| File type in corpus | Camera model | ISO / WB / Shutter / Lens | Gamma | Source |
|---|---|---|---|---|
| ARRI ALEXA Mini **`.mov`** (M_CAM) | ✅ `ALEXA Mini` | ✅ all four | ✅ `LOG-C` | mediainfo `General.extra.com_arri_camera_*` |
| ARRI ALEXA 35 / Mini LF **`.mxf`** (A_CAM, F_CAM) | ✅ via `General.Encoded_Application` + `…CompanyName=ARRI` | ❌ not embedded | ~ via `Video.transfer_characteristics` SMPTE UL (`0E17…`) | mediainfo standard fields; ISO/WB need `.ale` or ART CLI |
| Sony BURANO/Venice X-OCN **`.mxf`** (BURANO, B_CAM) | ~ `Encoded_Library_CompanyName=Sony` (model via `.xml`) | ✅ all four | ~ `TransferCharacteristics` + sidecar `CaptureGammaEquation` | mediainfo `Other.extra.*` (sidecar `.xml` additive) |
| Blackmagic **`.braw`** (D_CAM) | ✅ `camera_type` + `manufacturer` | lens ✅, shutter_type ✅, **ISO ❌** (only `analog_gain`) | ✅ `viewing_gamma` | mediainfo `General.extra.*` |
| DJI Ronin-4D ProRes **`.mov`** (RONIN-4D) | ❌ Apple mux | ❌ | BT.709 tag only | **nothing embedded — filename only** |
| Sony S-Log3 ProRes **`.mov`** transcode (C_CAM) | ❌ Apple mux | ❌ | mislabeled BT.709 | standard only — transcoding stripped acquisition data |

Takeaways:
- **`.mxf` vs `.mov` splits ARRI in two.** The `com_arri_camera_*` atoms are a
  QuickTime construct — they exist in ARRI `.mov` only. ARRI `.mxf` (ALEXA 35 /
  Mini LF) gives model via `Encoded_Application` and gamma via the transfer UL,
  but **not** ISO/WB/shutter. Dispatch by extension alone would be wrong; read
  the content.
- **Container ≠ camera.** Detect vendors from real signals, not filename or
  extension: `com_arri_camera_*` present → ARRI mov; `…CompanyName=ARRI` +
  `Encoded_Application` starts with `ALEXA` → ARRI mxf; `Encoded_Library_CompanyName=Sony`
  + an `Other` track with `ISOSensitivity_*` → Sony; `General.extra.manufacturer=Blackmagic Design` → BRAW.
- **Transcodes are lossy for metadata.** Once footage is re-wrapped to a generic
  Apple ProRes `.mov` (DJI Ronin, the S-Log3 test), acquisition metadata is
  gone. Don't promise camera fields for those — fall back to filename parse or
  leave blank.

## Normalization gotchas (values are strings, and scaled)

Every `extra` value is a **string**, and several are integer-scaled:

| Field | Raw | Meaning | Transform |
|---|---|---|---|
| ARRI `ShutterAngle` | `"1728"` | 172.8° | ÷10 |
| ARRI `SensorFps` / `ProjectFps` | `"24000"` | 24.000 fps | ÷1000 |
| ARRI `ExposureIndexAsa` | `"800"` | ISO 800 | as-is |
| ARRI `WhiteBalanceKelvin` | `"5600"` | 5600 K | as-is |
| ARRI `NdFilterDensity` | `"1200"` | ND 1.2 | ÷1000 |
| Sony `ShutterSpeed_Angle_FirstFrame_String` | `"180.0°"` | 180° | already display-ready |
| Sony `WhiteBalance_FirstFrame_String` | `"3600 K"` | 3600 K | already display-ready |
| Sony `IrisTNumber_FirstFrame` | `"1.922025"` | T1.9 | round for display |

Prefer the Sony `_String` variants when present (units baked in); for ARRI
apply the scale factors. Cross-checked against the Avid `.ale` for the same
reel — `Shutter_angle=172.8`, `Exposure_index=800`, `White_balance=5600`,
`Gamma=LOG-C` — the ALE agrees, so it's a good fallback/confirmation source.

## Also worth mapping (already in the payload, not just camera block)

- **`Video.transfer_characteristics`** — real color-pipeline signal. For ARRI
  mxf it's a SMPTE UL (`0E17010204020000` = LogC4 on ALEXA 35). Worth a small
  UL→gamma lookup so `.mxf` ARRI still reports gamma.
- **Current bug**: the mapper sets `colorSpace` from `colour_primaries`, so the
  ARRI mov reports `BT.709` when it's actually `LOG-C`. `extra.com_arri_camera_ColorGammaSxS`
  is the truth — camera gamma should win over the container primaries tag.
- **`Other` track `TimeCode_FirstFrame`** — already mapped, good.
- **Reel name**: ARRI `com_arri_camera_ReelName` / `Encoded_Application`, Sony via
  sidecar, BRAW `reel_name`. Our `reelName` mapping currently reads only
  `General.Reel_Name` / `Video.Reel_Name`, which none of these populate — reel
  detection is falling back to folders for every one of these cameras.

## Concrete next steps for the parser (mediainfo-only, no new deps)

1. Extend `mapMediaInfoToClipMetadata` to read `extra`:
   - ARRI mov: pull `com_arri_camera_*` from `General.extra`.
   - Sony: pull `*_FirstFrame` from the `Other` track that has `ISOSensitivity_FirstFrame`.
   - BRAW: pull from `General.extra` (`camera_type`, `lens_type`, `viewing_gamma`…).
   - ARRI mxf: `camera` = `General.Encoded_Application`; gamma via transfer-UL map.
2. Add a tiny per-vendor value-normalizer (the scaling table above) and prefer
   `_String` variants for display.
3. Fix `colorSpace`/gamma precedence: camera gamma (`ColorGammaSxS` /
   `CaptureGammaEquation` / `viewing_gamma`) over container `colour_primaries`.
4. Fix reel detection to read the vendor reel fields before the folder fallback.
5. Treat sidecars as **fallback/enrichment**, not primary: `.ale` fills ISO/WB
   for ARRI mxf; Sony `.xml` adds gamma-equation/model detail. Neither is needed
   for the common case.
6. Leave DJI/transcoded ProRes honestly blank (or filename-derived).

## Running the tool

```
cd web
bun tools/analyze-clips.mjs                     # default corpora
bun tools/analyze-clips.mjs "D:/path/to/CAMERA"  # a folder (walked)
bun tools/analyze-clips.mjs clip.mxf clip2.mov   # explicit files
```

Outputs land in `tools/out/` (git-ignored): one `<clip>.json` per file with the
full mediainfo payload + current mapping + sidecar text, plus `_schema.json` /
`_schema.md` (union of every tag seen).

---

# 2026-07-18 update — second corpus (`CAMERA/S_cam`): Canon RAW, FX6, Lumix, ProRes RAW

Ran `analyze-clips.mjs "…/CAMERA/S_cam"`. Six new clips. Two need a vendor
decoder (Canon Cinema RAW Light, ProRes RAW); the rest slot into patterns above.

| Clip | Container / codec | Camera model | ISO/WB/Shutter/Lens | Frames decodable? | Source |
|---|---|---|---|---|---|
| **Canon Cinema RAW Light `.crm`** (S001) | `crx` / `CRAW`, 6000×3164 50p | ❌ none in payload | ❌ none | ❌ CRAW proprietary | mediainfo gives **dims/fps/duration/TC only** |
| Canon EOS **C50 XF-AVC `.mxf`** (s004) | MXF / AVC, 3840×2160 25p | ✅ `EOS C50` (`Encoded_Application`) | ✅ full | ✅ AVC | mediainfo `Other.extra.*` — **same SMPTE acquisition track as Sony** |
| Canon **`.mp4`** XF-AVC/HEVC (S005) | `XFVC` / HEVC, 4096×2160 25p | ~ (needs `Video.extra.Metas` parse) | ❌ not surfaced | ✅ HEVC | mediainfo standard; Canon `Metas` blob opaque |
| Sony **FX6 XAVC `.mxf`** (S003) | MXF / AVC, 4096×2160 50p | ~ `CompanyName=Sony` | ✅ full (73-key acq. track) | ✅ AVC | mediainfo `Other.extra.*` — fits Sony MXF handling |
| Panasonic **Lumix S5 `.mov`** (S002) | MPEG-4 / AVC, 3840×2160 | via embedded XML | via embedded XML | ✅ AVC | **new shape**: `General.extra.com_panasonic_SemiPro_metadata_xml` (P2-style ClipMetadata XML string in a QT atom) |
| DJI **Ronin-4D ProRes RAW `.mov`** (S006) | MPEG-4 / ProRes RAW (`aprn`), 8192×4320 60p | ❌ (`App=DJI X2`) | ❌ | ❌ ProRes RAW not ffmpeg-decodable | mediainfo dims/fps only; `General.extra.snal` opaque |

Notes:
- **Canon XF-AVC MXF is great** — the C50 exposes the identical `Other.extra`
  acquisition set as Sony (ISO 800, WB 5600, Shutter 180°/(1/50), CaptureFrameRate,
  ImageSensor dims…) plus model via `Encoded_Application`. Same code path as Sony MXF.
- **`.crm` is the weak one** the request was about (see below).
- **Panasonic** needs a small XML-atom reader (parse `com_panasonic_SemiPro_metadata_xml`).
- The **`knownToCore`** flag in each dump shows which extensions Luna core doesn't
  list yet (`.crm` is the only `false` here).

## Canon Cinema RAW Light `.crm` — does it work? (the S001 question)

**Partly.** mediainfo reads the `crx` container and gives basic video params —
`6000×3164, 50fps, 26.88s, TC 00:25:14:20, codec CRAW` — so Luna *could* list
`.crm` clips with core info if we add `.crm` to the extensions. But:

- **No camera metadata** — no ISO/WB/lens/model, not even a Canon company tag.
  (Unlike Canon XF-AVC MXF, the RAW Light container doesn't expose acquisition data to mediainfo.)
- **No frames** — `CRAW` is Canon's proprietary RAW codec; neither ffmpeg.wasm
  nor mediabunny can decode it. There is **no native FFmpeg support** for CRM.

So `.crm` needs a vendor path for both a thumbnail and camera metadata. Options
found (GitHub / current, ordered by how usable they are for us to test):

1. **exiftool** (recommended first test) — reads Canon maker-note metadata
   (ISO/WB/lens/model) from the crx container AND extracts the embedded preview
   JPEG (`-PreviewImage` / `-JpgFromRaw`), so we get a thumbnail **without
   debayering**. crx embeds a `THMB` thumb + preview, same as CR3 stills. Node
   binding: `exiftool-vendored` (bundles the binary). Not installed here yet.
2. **LibRaw** — the CRX codec was reverse-engineered (A. Danilchenko, 2019) and
   is in LibRaw; `dcraw_emu` / LibRaw can debayer a real `.crm` frame. Open
   source, CLI-scriptable, and has WASM builds — the path if the embedded
   preview isn't good enough. Container reference: `lclevy/canon_cr3`.
3. **Canon Cinema RAW Development SDK** — official decoder (what Parallel Media
   Encoder uses). Gated/proprietary → desktop "detect-existing / link-out" pattern.
4. **DaVinci Resolve** decodes `.crm` natively → detect-existing on machines that have it.

**Recommended test path:** install exiftool (or `bun add -d exiftool-vendored`)
and run it on `S001/…CANONRAW.CRM` to confirm (a) Canon metadata fields and
(b) an embedded preview JPEG we can use as the thumbnail. If preview quality is
insufficient, fall back to LibRaw for a decoded frame. Same story applies to the
**ProRes RAW** Ronin-4D clip (S006) — no ffmpeg decode; needs a vendor route.
