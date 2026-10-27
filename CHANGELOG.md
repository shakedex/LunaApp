# Changelog

Every change that ships, kept as it lands. **Unreleased** collects what's on
`master` since the last tag. At release time it becomes the next version's
heading, and `packaging/release-notes/<version>.md` (the text users see) is
written from it.

Each entry: **New**, **Changed** or **Fixed**, then the platform when it isn't
both. Each release names the `shakedex/luna-decoders` commit it pins.

## Unreleased

Decoders: luna-decoders `8822d47`.

- **New:** ARRIRAW HDE (Codex High Density Encoding, SMPTE RDD 51) from the ALEXA 35 and
  ALEXA Mini LF, in MXF and as `.arx` frame sequences. Bit-exact against the ARRIRAW of
  the same take.
- **New:** ALEXA Mini LF and ALEXA Mini ARRIRAW in MXF: thumbnails, with colour measured
  against ARRI's own software.
- **New:** Sony XAVC H Intra (HEVC in MXF) from the BURANO.
- **New:** `.ari` and `.arx` frame sequences are one clip each, with timecode, recording
  date, lens, T-stop, ND, white balance and EI from the camera header.
- **New:** "Show problems in reports" in Settings, off by default. Off, reports leave out
  clip warnings, the "Need attention" count and unreadable files; the app and the logs
  still show them. `luna report --problems` prints them for one run.
- **Changed:** Luna checks for updates every time it opens, not only once a day.
- **Changed:** Sony VENICE lenses are named from their /i serial (for example "Cooke
  S7/i 85mm") instead of showing the serial.
- **Fixed:** Sony VENICE X-OCN XT clips had blown-out thumbnails.
- **Fixed:** Atomos Ninja V ProRes RAW failed on every frame. (Windows)
- **Fixed:** ARRIRAW HDE in MXF was read as plain ARRIRAW and failed.
- **Fixed:** ALEXA Mini LF clips showed no camera serial or ND.
- **Fixed:** Sony FX9 (and other clips without an `M01.xml`) showed "Sony" as the camera
  and a code as the lens; the XML inside the MXF now gives the model, serial and lens
  name. Dates from it are the camera's local time.
- **Fixed:** Avid OP-Atom MXF was reported as damaged, and its sound-track files showed
  up as clips; they now list under other files.
- **New:** Canon EOS C200 Cinema RAW Light: thumbnails, with exposure and colour checked
  against Canon's own development.
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
