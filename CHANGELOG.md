# Changelog

## Unreleased

Decoders: luna-decoders `4095c4b`.

- **Changed:** MXF clips that are clip-wrapped or have an unusable index no longer need
  attention; the run log still notes it.
- **Changed:** "Camera named from the folder name" and the like print in the report only
  with Show problems in reports on.
- **Changed:** Credits list each component's name and licence.

## 2026.11.0 — 2026-11-06

Tag `v2026.11.0`. Decoders: luna-decoders `4095c4b`.

- **New:** Camera look for Sony and ARRI: the LUT a VENICE 2 (X-OCN) or BURANO (XAVC) stored
  in the clip, and the look of an ALEXA 35 (ARRIRAW, ARRICORE, HDE, ProRes) or ALEXA Mini LF
  (ARRIRAW, HDE, ProRes) clip as the camera's monitor output showed it. The report names
  the look ("5110 Nature.alf4", "SL3SG3CtoTealOR_709.cube").
- **New:** Texture for ALEXA 35 clips (ProRes, ARRICORE, ARRIRAW, HDE), e.g. "K445 Default",
  in the report and as a CSV column.
- **Fixed:** ALEXA 65 `.ari` thumbnail colour: red and blue were too strong and green too weak.
  Now matches ARRI Reference Tool.
- **Fixed:** `.ari` / `.arx` frame sequences with missing frames: TC out and duration come
  from the last frame's own timecode, and the clip needs attention with the missing frame
  ranges named.
- **Fixed:** PDF pages that only list Other files or Unreadable no longer carry the last
  camera and reel in the page header.
- **Fixed:** After `Luna report <folder>` and *Back*, *Create report* uses the fields and
  settings changed in the window since launch. Options given on the command line still win.

## 2026.10.3 — 2026-10-27

Tag `v2026.10.3`. Decoders: luna-decoders `8822d47`.

- **New:** ARRIRAW HDE from the ALEXA 35 and ALEXA Mini LF, in MXF and as
  `.arx` frame sequences.
- **New:** ALEXA Mini LF and ALEXA Mini ARRIRAW in MXF: thumbnails.
- **New:** Sony XAVC H Intra (HEVC in MXF) from the BURANO.
- **New:** Blackmagic RAW scene and take from the camera's slate, in the Slate field.
- **New:** `.ari` and `.arx` frame sequences are one clip each, with timecode, recording
  date, lens, T-stop, ND, white balance and EI from the camera header.
- **New:** Thumbnail colour in Settings: Rec.709, None (the camera's log) or a `.cube`
  LUT. The report names what was applied. `luna report --colour rec709|none|lut` and
  `--lut <file>` do the same for one run.
- **New:** "Camera look" in Settings: Blackmagic RAW clips recorded with "Apply LUT in
  File" use their LUT for the thumbnails (`--camera-look` on the
  command line).
- **New:** "Show problems in reports" in Settings, off by default. Off, reports leave out
  clip warnings, the "Need attention" count and unreadable files; the app and the logs
  still show them. `luna report --problems` prints them for one run.
- **Changed:** Luna checks for updates every time it opens, not only once a day.
- **Changed:** Sony VENICE lenses are named from their /i serial (for example "Cooke
  S7/i 85mm").
- **Fixed:** Switching "Also save on the source drive" or "Show problems in reports" and
  then saving other Settings edits switched it back.
- **Fixed:** Sony VENICE X-OCN XT clips had blown-out thumbnails.
- **Fixed:** Atomos Ninja V ProRes RAW failed on every frame. (Windows)
- **Fixed:** ARRIRAW HDE in MXF was read as plain ARRIRAW and failed.
- **Fixed:** ALEXA Mini LF clips showed no camera serial or ND.
- **Fixed:** Sony FX9 (and other clips without an `M01.xml`) showed "Sony" as the camera
  and a code as the lens; they now show the model, serial and lens
  name.
- **Fixed:** Avid OP-Atom MXF was reported as damaged, and its sound-track files showed
  up as clips; they now list under other files.
- **New:** Canon EOS C200 Cinema RAW Light: thumbnails.
- **Changed:** a CinemaDNG sequence is one clip that says "CinemaDNG isn't supported yet",
  instead of "No camera clips in this folder". DNG stills stay other files.
- **Changed:** a reel shared by two folders names the source folder by its own name
  instead of "(.)".

## 2026.10.2 — 2026-10-26

Tag `v2026.10.2`. Decoders: luna-decoders `d8d0c21`.

- **New:** every report run writes a log file; **Settings → Show logs** opens the folder.
- **New:** iPhone clips show lens, focal length (35 mm equivalent) and f-stop.
- **New:** Blackmagic Video Assist clips take camera, reel, scene, shot and take from the
  recorder.
- **Changed:** the CSV has Scene, Shot and Take columns after Reel.
- **Fixed:** iPhone clips with sub-millisecond frame timing jitter had no frame rate.
- **Fixed:** portrait phone clips had sideways thumbnails.
- **Fixed:** an MXF cut off before it finished had no frame count or thumbnails.

## 2026.10.1 — 2026-10-24

Tag `v2026.10.1`. Decoders: luna-decoders `6d7f665`.

- **Fixed:** two reports finishing in the same minute failed with "The directory is not
  empty".
- **Changed:** a `--field` given on the command line replaces a saved field of the same
  name.
- **New:** `automation/offshoot_luna.py` makes a report for each copy OffShoot finishes.

## 2026.10.0 — 2026-10-24

Tag `v2026.10.0`. Decoders: luna-decoders `6d7f665`. First release.
