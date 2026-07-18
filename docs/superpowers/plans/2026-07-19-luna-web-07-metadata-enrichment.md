# Luna Web — Plan 07: Camera Metadata Enrichment Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** The camera block (`camera, iso, whiteBalance, lens, focalLength, aperture, shutter, gamma`) populates from mediainfo's `track.extra` for ARRI (.mov and .mxf), Sony/Canon acquisition-track MXF, Blackmagic BRAW, and Panasonic Lumix — and the two data-confirmed mapper bugs are fixed (camera gamma wins; vendor reel-name fields feed reel detection). Exports show it all with **zero export-code changes** (the columns are pre-wired).

**Architecture:** Entirely pure core work, driven by the maintainer's real-corpus findings (`web/tools/FINDINGS.md` — treat it as the requirements source for key names, shapes, and scale factors). A small **vendor-enricher pipeline** mirrors the desktop's content-aware-enricher philosophy: the base mapper runs first, then an ordered registry of enrichers (`detect(result)` → `enrich(result, base)`) fills the camera block. Detection is **by content signals, never extension** (ARRI .mov and ARRI .mxf are different animals; Canon C50 shares Sony's acquisition-track shape). Values are normalized per the findings' scale table (ARRI ints ÷10/÷1000; Sony/Canon `_String` variants preferred). Transcoded/DJI ProRes stays honestly blank.

**Tech Stack:** no new dependencies. Panasonic's embedded P2-style XML is parsed with targeted string extraction (core has no DOMParser and doesn't need one for a known vendor blob).

**Spec:** §8.6/§13 (fidelity, precedence), §8.7 (reel detection gains real vendor reel names). **Requirements source:** `web/tools/FINDINGS.md` (both corpus sections).

## Global Constraints

- **`packages/core` stays DOM-free.** No DOMParser; Panasonic XML via string/regex extraction of known tags.
- **FINDINGS.md is the requirements doc** for key names/shapes/scales. Where it documents keys (ARRI mov, Sony, BRAW, scales), transcribe them. Where it doesn't (Panasonic tag names, some ARRI details), the task is **data-driven**: run the maintainer's tool (`cd web && bun tools/analyze-clips.mjs <corpus path>`) against the local corpus (`D:/LUNA_TEST/TEST_PROJECT_LUNA/CAMERA`, `.../CAMERA/S_cam`) and read the JSON dumps in `web/tools/out/`. If the corpus/tool is unavailable, return NEEDS_CONTEXT — never invent key names.
- **Fidelity (§13):** enrichers fill only what the payload carries; missing stays `undefined`. BRAW has no ISO (only `analog_gain`) — leave `iso` blank there. Sony MXF model is not embedded — `camera` falls back to the manufacturer string only when that's all there is.
- **Precedence:** base standard fields never overwritten by enrichers EXCEPT `gamma` and `reelName`, where camera-sourced values win (the two confirmed bugs). `colorSpace` keeps `colour_primaries`; `gamma` carries the camera pipeline truth.
- **Detection by content:** `com_arri_camera_*` in `General.extra` → ARRI mov; `Encoded_Library_CompanyName` contains `ARRI` + `Encoded_Application` → ARRI mxf; an `Other` track with `ISOSensitivity_FirstFrame` → Sony/Canon acquisition; `General.extra.manufacturer === 'Blackmagic Design'` → BRAW; `General.extra.com_panasonic_SemiPro_metadata_xml` → Panasonic. First match wins, ordered exactly like that.
- **Tests:** `bun test`, core only — every enricher gets fixture tests built from the corpus-documented keys (Panasonic fixture from the real dump's XML string).
- **Gates green after every task** (from `web/`): `bun run lint && bun run typecheck && bun test && bun run build`.
- **Never touch `web/tools/`** (the maintainer's; read-only + running it is fine). No app-side changes expected at all — the mapper is already wired. The .NET app at the repo root is never touched.

---

## File Structure

```
web/packages/core/src/metadata/
  mediainfo.ts                 MODIFY: after base mapping, run the enricher registry
  normalize.ts                 NEW: scaledNumber, kelvinDisplay, degreesDisplay, tNumberDisplay
  vendors/types.ts             NEW: VendorEnricher interface + findTrack helpers
  vendors/arri-mov.ts          NEW
  vendors/arri-mxf.ts          NEW (incl. transfer-UL → gamma lookup)
  vendors/acquisition.ts       NEW (Sony + Canon shared Other.extra track)
  vendors/braw.ts              NEW
  vendors/panasonic.ts         NEW (P2 XML string extraction — data-driven task)
  vendors/registry.ts          NEW: ordered enricher list + applyVendorEnrichment
web/packages/core/test/
  enrich-arri.test.ts          NEW (mov + mxf)
  enrich-acquisition.test.ts   NEW (Sony + Canon)
  enrich-braw.test.ts          NEW
  enrich-panasonic.test.ts     NEW
  mediainfo-map.test.ts        MODIFY: existing tests still pass (enrichers no-op on plain fixtures)
web/packages/core/src/index.ts MODIFY: export applyVendorEnrichment (for the tools script's use, optional)
```

---

## Task 1: Enricher framework + normalizers + ARRI .mov enricher (TDD)

**Files:**
- Create: `src/metadata/normalize.ts`, `src/metadata/vendors/types.ts`, `src/metadata/vendors/arri-mov.ts`, `src/metadata/vendors/registry.ts`
- Modify: `src/metadata/mediainfo.ts` (call the registry after base mapping), `src/index.ts`
- Test: `test/enrich-arri.test.ts` (mov half)

**Interfaces (exact, used by Tasks 2–4):**

```ts
// vendors/types.ts
export interface VendorEnricher {
  id: string
  detect(result: MediaInfoObjectResult): boolean
  enrich(result: MediaInfoObjectResult, base: ClipMetadata): ClipMetadata
}
export function generalTrack(result: MediaInfoObjectResult): MediaInfoTrack | undefined
export function videoTrack(result: MediaInfoObjectResult): MediaInfoTrack | undefined
export function otherTrackWith(result: MediaInfoObjectResult, key: string): MediaInfoTrack | undefined
export function extraOf(track: MediaInfoTrack | undefined): Record<string, unknown>
// extraOf: returns track.extra when it is a plain object, else {}

// normalize.ts
export function scaledNumber(v: unknown, divisor: number): number | undefined
// str/num-tolerant; NaN/empty → undefined; e.g. scaledNumber('1728', 10) → 172.8
export function degreesDisplay(v: unknown, divisor?: number): string | undefined  // '172.8°'
export function kelvinDisplay(v: unknown): string | undefined                     // '5600 K'
export function tNumberDisplay(v: unknown): string | undefined                    // 'T1.9' (1 decimal)

// registry.ts
export const vendorEnrichers: readonly VendorEnricher[] // ordered per Global Constraints
export function applyVendorEnrichment(result: MediaInfoObjectResult, base: ClipMetadata): ClipMetadata
// first enricher whose detect() is true runs; none → base returned unchanged
```

**ARRI .mov mapping (from FINDINGS `General.extra.com_arri_camera_*`):**
`camera ← CameraModel`; `iso ← ExposureIndexAsa` (as-is, String); `whiteBalance ← kelvinDisplay(WhiteBalanceKelvin)`; `shutter ← degreesDisplay(ShutterAngle, 10)`; `gamma ← ColorGammaSxS` (WINS over base); `lens ← LensType`; `reelName ← com_arri_camera_ReelName` (WINS over base). Enricher never touches width/height/codec/etc.

- [ ] **Step 1: Write the failing test** — `test/enrich-arri.test.ts`:

```ts
import { describe, expect, test } from 'bun:test'
import { mapMediaInfoToClipMetadata } from '../src/metadata/mediainfo'

const arriMov = {
  media: {
    track: [
      {
        '@type': 'General',
        extra: {
          com_arri_camera_CameraModel: 'ALEXA Mini',
          com_arri_camera_ExposureIndexAsa: '800',
          com_arri_camera_WhiteBalanceKelvin: '5600',
          com_arri_camera_ShutterAngle: '1728',
          com_arri_camera_ColorGammaSxS: 'LOG-C',
          com_arri_camera_LensType: 'Cooke S4 32mm',
          com_arri_camera_ReelName: 'A001R2B',
        },
      },
      { '@type': 'Video', Width: '2880', Format: 'ProRes', colour_primaries: 'BT.709' },
    ],
  },
}

describe('ARRI .mov enrichment', () => {
  test('fills the camera block from com_arri_camera_* with normalization', () => {
    const m = mapMediaInfoToClipMetadata(arriMov)
    expect(m.camera).toBe('ALEXA Mini')
    expect(m.iso).toBe('800')
    expect(m.whiteBalance).toBe('5600 K')
    expect(m.shutter).toBe('172.8°')
    expect(m.gamma).toBe('LOG-C')
    expect(m.lens).toBe('Cooke S4 32mm')
    expect(m.reelName).toBe('A001R2B') // vendor reel WINS (base had none)
    expect(m.colorSpace).toBe('BT.709') // primaries untouched; gamma carries the truth
    expect(m.width).toBe(2880) // base fields untouched
  })

  test('non-ARRI payloads are untouched (enricher no-op)', () => {
    const m = mapMediaInfoToClipMetadata({
      media: { track: [{ '@type': 'General' }, { '@type': 'Video', Format: 'AVC' }] },
    })
    expect(m.camera).toBeUndefined()
    expect(m.gamma).toBeUndefined()
  })
})
```

- [ ] **Step 2: red** — `cd web/packages/core && bun test enrich-arri` FAILS (mapper doesn't enrich).

- [ ] **Step 3: Implement** — `normalize.ts`, `vendors/types.ts` per the interfaces above; `vendors/arri-mov.ts`:

```ts
import type { ClipMetadata } from '../model'
import { degreesDisplay, kelvinDisplay } from '../normalize'
import { extraOf, generalTrack } from './types'
import type { MediaInfoObjectResult } from '../mediainfo'
import type { VendorEnricher } from './types'

function str(v: unknown): string | undefined {
  return typeof v === 'string' && v.trim() !== '' ? v : undefined
}

// ARRI records QuickTime atoms only in .mov (FINDINGS: "mxf vs mov splits ARRI in two").
export const arriMovEnricher: VendorEnricher = {
  id: 'arri-mov',
  detect: (result) =>
    Object.keys(extraOf(generalTrack(result))).some((k) => k.startsWith('com_arri_camera_')),
  enrich: (result, base) => {
    const extra = extraOf(generalTrack(result))
    return {
      ...base,
      camera: str(extra.com_arri_camera_CameraModel) ?? base.camera,
      iso: str(extra.com_arri_camera_ExposureIndexAsa) ?? base.iso,
      whiteBalance: kelvinDisplay(extra.com_arri_camera_WhiteBalanceKelvin) ?? base.whiteBalance,
      shutter: degreesDisplay(extra.com_arri_camera_ShutterAngle, 10) ?? base.shutter,
      lens: str(extra.com_arri_camera_LensType) ?? base.lens,
      // Camera truth WINS over container tags (confirmed mapper bugs, FINDINGS):
      gamma: str(extra.com_arri_camera_ColorGammaSxS) ?? base.gamma,
      reelName: str(extra.com_arri_camera_ReelName) ?? base.reelName,
    }
  },
}
```
`registry.ts` with `[arriMovEnricher]` for now (Tasks 2–4 append in the constrained order); `mediainfo.ts`: the return becomes `return applyVendorEnrichment(result, base)` where `base` is the current object.

- [ ] **Step 4: green** — `bun test` all green (existing mediainfo tests must still pass: plain fixtures trigger no detect).
- [ ] **Step 5: gates + commit** — `feat(core): vendor enricher pipeline with ARRI mov support`

---

## Task 2: ARRI .mxf + Sony/Canon acquisition enrichers (TDD)

**Files:**
- Create: `vendors/arri-mxf.ts`, `vendors/acquisition.ts`
- Modify: `vendors/registry.ts` (order: arri-mov, arri-mxf, acquisition, …)
- Test: extend `test/enrich-arri.test.ts` (mxf), create `test/enrich-acquisition.test.ts`

**ARRI .mxf (from FINDINGS):** detect `Encoded_Library_CompanyName` containing `ARRI` (check `General` track, tolerate the field living beside or inside extra — verify against the corpus dump) AND `Encoded_Application` present. Map: `camera ← Encoded_Application`; `gamma ←` transfer-UL lookup on `Video.transfer_characteristics` — seed the map with `{'0E17010204020000': 'LogC4'}` (ALEXA 35, corpus-confirmed) and structure it for additions; no ISO/WB/shutter (not embedded — stays blank per fidelity).

**Sony/Canon acquisition (from FINDINGS — identical shape for Venice/BURANO/FX6/C50):** detect an `Other` track whose extra has `ISOSensitivity_FirstFrame`. Map from that track's extra: `iso ← ISOSensitivity_FirstFrame`; `whiteBalance ← WhiteBalance_FirstFrame_String` (display-ready) else `kelvinDisplay(WhiteBalance_FirstFrame)`; `shutter ← ShutterSpeed_Angle_FirstFrame_String` else `ShutterSpeed_Time_FirstFrame` (as-is string); `aperture ← tNumberDisplay(IrisTNumber_FirstFrame)` else `IrisFNumber_FirstFrame` display `F…`; `focalLength ← LensZoomActualFocalLength_FirstFrame` (+' mm' if bare number); `lens ← LensAttributes_FirstFrame`; `gamma ← TransferCharacteristics_FirstFrame` (string, WINS); `camera ← General.Encoded_Application` (C50) else `General.Encoded_Library_CompanyName` (e.g. 'Sony') else untouched.

Fixtures for both from the FINDINGS-documented keys (Sony: ISO 800, WB '3600 K', shutter '180.0°', T'1.922025'→'T1.9'; Canon C50: same track + `Encoded_Application: 'EOS C50'`). **Data-driven check:** before finalizing exact key spellings, run the maintainer's tool against the corpus dumps if available and confirm; report what you confirmed vs transcribed.

Steps: red (both new describe blocks) → implement → green → gates → commit `feat(core): arri mxf and sony/canon acquisition enrichers`.

---

## Task 3: BRAW enricher + precedence integration tests (TDD)

**Files:**
- Create: `vendors/braw.ts`
- Modify: `vendors/registry.ts` (append braw)
- Test: `test/enrich-braw.test.ts`

**BRAW (from FINDINGS `General.extra`):** detect `manufacturer === 'Blackmagic Design'`. Map: `camera ← camera_type` (prefix manufacturer? No — camera_type alone, e.g. 'Blackmagic Pocket Cinema Camera 6K'); `lens ← lens_type`; `gamma ← viewing_gamma` (WINS); `shutter ← shutter_type`? — NO: `shutter_type` is a mode string; check the corpus dump for `shutter_angle`/`shutter_speed` keys and use those if present, else leave blank (report). `reelName ← reel_name` (WINS). **`iso` stays blank** (only `analog_gain` exists — fidelity rule; do NOT map gain to iso).

**Precedence integration tests (same file):**
1. A payload with BOTH `General.Reel_Name` and BRAW `reel_name` → vendor value wins.
2. A payload with `colour_primaries: 'BT.709'` + `viewing_gamma: 'Blackmagic Design Film'` → `gamma` is the camera value, `colorSpace` still 'BT.709'.
3. detectReels downstream: `metadata.reelName` from a vendor field flows into reel grouping with no further change (assert via `detectReels([{ relativePath: 'X/a.braw', reelName: m.reelName }])`).

Steps: red → implement → green → gates → commit `feat(core): braw enricher and vendor precedence guarantees`.

---

## Task 4: Panasonic P2-XML enricher (DATA-DRIVEN, TDD)

**Files:**
- Create: `vendors/panasonic.ts`
- Modify: `vendors/registry.ts` (append panasonic — last)
- Test: `test/enrich-panasonic.test.ts`

**This task is corpus-driven — FINDINGS documents the shape's existence but not its tag names.**

- [ ] **Step 1: Extract the real XML.** Run `cd web && bun tools/analyze-clips.mjs "D:/LUNA_TEST/TEST_PROJECT_LUNA/CAMERA/S_cam"` (or read an existing dump in `web/tools/out/` if present). Locate the S002 Lumix dump's `General.extra.com_panasonic_SemiPro_metadata_xml` string. **If the corpus, the D: drive, or the tool output is unavailable → STOP and return NEEDS_CONTEXT** (the controller will get the XML from the maintainer). Never invent tag names.
- [ ] **Step 2: Fixture from reality.** Copy the actual XML string (trim irrelevant sections, keep real tags) into the test fixture. Write assertions for whichever of the camera-block fields the XML genuinely carries (expected per P2 convention: ISO/gain, white balance, shutter, camera model; assert exactly what exists — report the coverage).
- [ ] **Step 3: red → implement.** `panasonic.ts`: detect the extra key; extract fields with targeted regex per tag (e.g. `/<TagName>([^<]*)<\/TagName>/` — exact names from the dump), tolerate absence, normalize displays consistently with the other enrichers (kelvin/degrees helpers where the units match). Camera gamma → `gamma` (WINS) if the XML carries it.
- [ ] **Step 4: green → gates → commit** `feat(core): panasonic p2-xml enricher`.
- [ ] **Step 5 (verification, non-blocking):** if the corpus ran, re-run the tool and note in your report which previously-blank fields now populate in the dumps' `currentMapping` for S002 (and spot-check an ARRI/Sony clip too — the tool exercises the real mapper).

---

## Definition of done (this plan)

- `bun test` green (49 prior + ~10 new enricher tests).
- All gates green from `web/`. Zero app-side diffs (exports/UI pick the fields up automatically).
- **Manual QA (maintainer, Chromium, with the corpus):**
  1. Scan the CAMERA corpus → ARRI mov clips show ISO/WB/shutter/gamma/lens; reels group by TRUE reel names (not folders).
  2. Sony/Canon MXF clips show the full acquisition block; C50 shows `EOS C50`.
  3. BRAW clips show camera/lens/gamma/reel; ISO blank (honest).
  4. Lumix clip shows its XML-derived fields.
  5. DJI/transcoded ProRes: camera block blank (honest), everything else normal.
  6. Export PDF + CSV → the camera columns are populated with zero export changes.

## Self-review notes

- **Requirements source:** FINDINGS.md both sections; every key/scale in Tasks 1–3 traces to its tables; Task 4 refuses to guess.
- **The two confirmed bugs fixed:** gamma precedence (camera wins into `gamma`; `colorSpace` untouched — if the maintainer wants LOG-C to replace the colorSpace column display too, that's a one-line follow-up) and vendor reelName feeding detectReels (integration-tested in Task 3).
- **Spec alignment:** content-aware enrichers that self-select mirror the desktop's documented design; fidelity §13 held throughout (BRAW iso, ARRI-mxf ISO/WB, DJI all-blank).
- **Placeholders:** none; Task 4's unknowns are explicit NEEDS_CONTEXT gates, not guesses.
- **Type consistency:** `VendorEnricher`/`applyVendorEnrichment`/helpers defined once in Task 1, consumed by 2–4; registry order fixed in Global Constraints.
- **Deliberately out:** sidecars (.ale/.xml) per FINDINGS ("fallback/enrichment, not primary"); `.crm`/ProRes RAW (user's separate research in flight — re-check FINDINGS before Plan 08); Canon `.mp4` `Metas` blob (opaque, noted in FINDINGS).
