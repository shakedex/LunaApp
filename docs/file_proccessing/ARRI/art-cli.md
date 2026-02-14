# ARRI ART CLI (Quick Explainer)

Use `art-cmd.exe` to export a short processed segment from a source clip.

## Main command

`art-cmd.exe --mode export --input <filename.ext> --output <export_dir/> --duration 1 --skip-audio`

## What each option does

- `--mode export` : runs the export operation.
- `--input <filename.ext>` : input source file (e.g., `.mxf`, `.mov`, etc.).
- `--output <export_dir/>` : destination folder for exported results.
- `--duration 1` : export duration in seconds (here: 1 second).
- `--skip-audio` : disables audio export.

## Example

`art-cmd.exe --mode export --input reel_A001.mxf --output ./exports/ --duration 1 --skip-audio`

## Runtime binaries

The `bin/` folder contains the required ART CLI binaries for:

- Windows
- macOS (Apple Silicon / arm64)

These binaries should be embedded and shipped with the app so CLI export works on both targets.

## Metadata output (what to read first)

From the exported `metadata.json`, prioritize:

- `cameraModel` (from **Camera Device**)
- `lensModel` (from **Lens Device**)

Also useful for reports:

- **Slate Info**: `clipName`, `reelName`, `scene`, `take`, `production`, etc.
- Frame timing: `frameBasedMetadata.frames[0].timecode`
- Exposure block (per frame): `exposureIndex`, `exposureTime`, `sensorSampleRate`
- **Lens State** (per frame): focal length, iris, focus distance, encoder values

## Lens State → human-readable values

Based on ARRI schema `camera/lens_state/v1-0-1`:

- `lensFocalLength`, `lensEffectiveFocalLength`: stored in **µm**
	- Convert to mm: `value / 1000`
- `lensFocusDistanceMetric`: stored in **mm**
	- Convert to meters: `value / 1000`
- `lensFocusDistanceImperial`: stored in **1/1000 inch**
	- Convert to inches: `value / 1000`
	- Convert to feet: `value / 12000`
- `lensIris`: stored as **1/1000 stop** (T1 = 1000)
	- Convert to T-stop: `T = value / 1000`

Sentinel values in schema:

- focus distance: `-1` = infinity, `0` = not available
- iris: `-1` = not available, `-2` = closed, `-3` = near close
- some focal-length fields use `0` = not available

### Example from your sample

- `lensFocalLength: 44978` → **44.978 mm**
- `lensIris: 5190` → **T5.19**
- `lensFocusDistanceMetric: 14471` → **14.471 m**
- `lensFocusDistanceImperial: 569724` → **569.724 in** (≈ **47.477 ft**)
