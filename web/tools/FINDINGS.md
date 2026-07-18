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
