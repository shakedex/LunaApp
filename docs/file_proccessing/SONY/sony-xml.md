# Sony Sidecar XML (How to Read)

Sony camera sidecar metadata is stored as XML next to the clip (same folder as MXF).

Root node:

- `NonRealTimeMeta` (with XML namespace `urn:schemas-professionalDisc:nonRealTimeMeta:ver.2.00`)

## Read order (recommended)

1. Basic clip info (`Duration`, `CreationDate`, `VideoFormat`, `AudioFormat`)
2. `AcquisitionRecord/Group[@name='LensUnitMetadataSet']`
3. `AcquisitionRecord/Group[@name='CameraUnitMetadataSet']`
4. Optional look/posture info:
	 - `Group[@name='SonyF65CameraMetadataSet']`
	 - `Group[@name='CameraPostureMetadataSet']`
	 - `ExtendedContents` (ASC CDL)

## Primary fields to extract

### Lens

From `LensUnitMetadataSet`:

- `LensZoomActualFocalLength` (e.g. `50.00mm`, `86.00mm`)
- `FocusPositionFromImagePlane` (e.g. `2.924m`)
- `IrisTNumber` / `IrisFNumber`

If needed, also keep raw ring/encoder-like fields:

- `IrisRingPosition`, `FocusRingPosition`, `ZoomRingPosition`

### Camera

From `CameraUnitMetadataSet`:

- `ISOSensitivity`
- `ExposureIndexOfPhotoMeter`
- `ShutterSpeed_Time` and/or `ShutterSpeed_Angle`
- `WhiteBalance`
- `CaptureFrameRate`
- `NeutralDensityFilterWheelSetting`

For camera identity (best available in sidecar):

- `CameraAttributes` (e.g. `MPC-2610 ...`, `MPC-3610 ...`)

> Note: unlike ARRI JSON, Sony sidecar samples do not expose a direct `cameraModel` field with friendly model text. You may map `CameraAttributes` / codec profile to a display model in app logic.

## Mapping to app-friendly output

- `lensModel` (Sony approximation):
	- Prefer a dedicated lens ID if available in future files.
	- In current samples, use `LensAttributes` as raw identifier and display focal/focus/T-stop from lens items.
- `cameraModel` (Sony approximation):
	- Derive from `CameraAttributes` and/or `VideoFrame/@videoCodec`.

## Normalization tips

- Values may include units in the string (e.g. `mm`, `m`, `deg`, `ms`, `%`).
- Store both:
	- raw string (`"50.00mm"`)
	- parsed numeric + unit (`50.00`, `mm`)
- Some fields can repeat (example: `ExposureIndexOfPhotoMeter` appears twice). Keep first non-empty value or deduplicate equal values.

## Useful XPath-style references

- Duration: `/NonRealTimeMeta/Duration/@value`
- Timecode start/end: `/NonRealTimeMeta/LtcChangeTable/LtcChange`
- Video codec: `/NonRealTimeMeta/VideoFormat/VideoFrame/@videoCodec`
- Lens items: `/NonRealTimeMeta/AcquisitionRecord/Group[@name='LensUnitMetadataSet']/Item`
- Camera items: `/NonRealTimeMeta/AcquisitionRecord/Group[@name='CameraUnitMetadataSet']/Item`

## Quick examples from provided files

- BURANO sample:
	- `LensZoomActualFocalLength = 50.00mm`
	- `FocusPositionFromImagePlane = 2.924m`
	- `ISOSensitivity = 800`
- VENICE sample:
	- `LensZoomActualFocalLength = 86.00mm`
	- `FocusPositionFromImagePlane = 3.958m`
	- `ISOSensitivity = 500`
