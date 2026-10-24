# macOS: everything still open

The one list of Mac work, across Luna and the private decoders. Written for a
Claude Code session on Shaked's Mac mini (Apple Silicon, full Xcode). Work
top to bottom; each item says where its details are and what "done" means.
Windows is the reference: every item there is built and verified.

## Ground rules

- **Leave the machine as you found it.** The Mac is used for other projects.
  Never uninstall, downgrade, reconfigure or clear Xcode, the Command Line
  Tools, Homebrew, rustup toolchains or global caches (`~/.cargo`,
  `~/.rustup`, DerivedData). Use what's installed; ask Shaked before
  installing anything. Delete only what your own work created.
- **Apple Silicon only** (`aarch64-apple-darwin`), **macOS 26 or later**. No Intel Mac support.
- **No reverse engineering** of vendor SDKs (Blackmagic, RED) or formats
  whose licence forbids it. Each decoder's `docs/spec.md` has its rules.
- **Never commit** vendor SDKs, camera files, or the update signing key.
- **Don't push to `master`** of either repo without Shaked's go-ahead. Push
  your work as `mac/<topic>` branches.
- **GUI apps are fine.** Shaked cleared Luna's window for work on this Mac (2026-10-24).

## Set up

```bash
mkdir -p ~/Code && cd ~/Code
gh auth login && gh auth setup-git   # once: the decoders repo is private
git clone https://github.com/shakedex/LunaApp.git
git clone https://github.com/shakedex/luna-decoders.git decoders
```

Luna's `Cargo.toml` fetches the decoders from GitHub by commit, so
`~/Code/decoders` is only for working on them. The xtask finds the R3D
add-on at `../decoders/r3d-sdk`, which is why the clone is named `decoders`.

**Test clips stay on the external drive** (`MASTER_01`, exFAT): read them in place from
`/Volumes/MASTER_01/TEST_PROJECT_LUNA/CAMERA`. Never copy clips onto the Mac.

**What Shaked copies over by hand** (never via git):

| What | From | Put it at |
|---|---|---|
| RED SDK subset | `Include/`, `Lib/mac64/`, `Redistributable/mac/REDR3D.dylib` from the R3D SDK | `~/Code/decoders/r3d-sdk/reference/` |
| Update signing key | `~/.luna/update-key` (only for item 6) | `~/.luna/update-key` |

Blackmagic RAW SDK: install it on the Mac from Blackmagic's site (item 3).
If files arrive quarantined (browser, AirDrop), clear the flag on those copies only:
`xattr -dr com.apple.quarantine <folder>`.

**Environment**, once per shell. `.cargo/config.toml` holds the Windows paths and
doesn't force them, so these win:

```bash
export LUNA_CORPUS=/Volumes/MASTER_01/TEST_PROJECT_LUNA/CAMERA
export BRAW_SDK_DIR="/Applications/Blackmagic RAW/Blackmagic RAW SDK"
export BRAW_TEST_CLIPS=$LUNA_CORPUS/D_CAM   # braw-sdk's tests
export R3D_SDK_DIR=~/Code/decoders/r3d-sdk/reference
export R3D_TEST_CLIPS=$LUNA_CORPUS          # r3d-sdk's tests
export MACOSX_DEPLOYMENT_TARGET=26.0        # outside LunaApp, where .cargo/config.toml doesn't apply
export LUNA_R3D_ADDON=~/Code/decoders/r3d-sdk/target/release/libr3d.dylib
export LUNA_R3D_SDK_DIR=~/Code/decoders/r3d-sdk/reference/Redistributable/mac
```

## 1. Luna builds and tests on macOS

```bash
cd ~/Code/LunaApp
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

None of this has run on a Mac. Fix what breaks. Corpus tests skip clips that
are missing, so note which ran.

**Done when:** all three pass, and you've listed the tests that skipped.

**Status 2026-10-24: done** (branch `mac/build`). All three pass, including all 44 corpus
tests against the real clips. The clips were read in place from the external drive
(`LUNA_CORPUS=/Volumes/MASTER_01/TEST_PROJECT_LUNA/CAMERA`); don't copy them onto the Mac.

## 2. H.264 / HEVC thumbnails through AVFoundation

`crates/luna-decode/src/platform_macos.rs` is a stub that returns "isn't
supported on macOS yet". Write it with AVFoundation
(`AVAssetImageGenerator`, or `AVAssetReader` + VideoToolbox), keeping the
`decode(clip, probe, positions, min_width)` contract every backend in
`luna-decode` follows (see `mf.rs` for the Windows version). Platform APIs
stay inside `luna-decode`. `luna-update` already compiles an Objective-C shim
with `cc` (`crates/luna-update/build.rs`); follow that pattern for
frameworks.

Clips: `S_cam/S002/lumix_s5.mov` and the H.264/HEVC clips in
`crates/luna-decode/tests/corpus.rs`.

**Done when:** the H.264 and HEVC corpus tests pass on the Mac with real thumbnails.

**Status 2026-10-24: done** (branch `mac/build`). AVFoundation decodes both clips natively,
with no luna-h264 fallback: the Lumix H.264 4:2:2 10-bit and the Canon HEVC 4:2:2 10-bit,
about 1 s for 3 frames each. The synthetic platform tests pass too.

## 3. Blackmagic RAW on the Mac

The checklist is `~/Code/decoders/braw-sdk/docs/macos.md`. Its Mac code is
written but has never been compiled. The biggest risk is CFString ownership
in the shim; the checklist says how to measure it.

**Done when:** that checklist is ticked, BRAW tests pass in
`~/Code/decoders/braw-sdk`, and Luna's `braw_cinema_camera_6k` corpus test
passes. Clip: `D_CAM/D001/D001_09150935_C013.braw`.

**Status 2026-10-24: done** (luna-decoders branch `mac/braw`). Every box is ticked:
- The frame oracle matches byte for byte.
- CFString ownership is measured.
- 6K timings: open 1 ms, full 51–57 ms.
- Luna's `braw_cinema_camera_6k` passes.

## 4. R3D and N-RAW add-on on the Mac

Steps and the expected failure modes are in
`~/Code/decoders/r3d-sdk/docs/mac-test-handoff.md`. Ignore its bundle and
`~/r3d-test` layout: use `~/Code/decoders/r3d-sdk` directly, with the SDK
subset in `reference/` as above. Don't re-pin decode checksums over the
Windows values; report Mac values instead.

**Done when:** `cargo test` passes in `r3d-sdk` (one Windows-only test
fewer), and Luna's `r3d_through_the_addon` and `nikon_nraw_through_the_addon`
corpus tests pass. Clips: `R3D/A005_W055_0819CM_001.R3D` and the Nikon `.NEV`.

**Status 2026-10-24: done, with one finding** (luna-decoders branch `mac/r3d`).
- **The fix:** RED's Mac library was built without RTTI, so the shim is now built with
  `-fno-rtti` on macOS.
- **Results:** 28 of 29 tests pass, and Luna's `r3d_through_the_addon` and
  `nikon_nraw_through_the_addon` pass.
- **Per-platform pins:** RED's Mac library gives slightly different R3D pixels from
  Windows, deterministic on each, and Shaked checked the output by eye. The Mac checksums
  are pinned next to the Windows ones. The NEV pin is the same on both.
- **Timings:** ⅛ in 45 ms, full 8K in 2.6 s.

## 5. ProRes RAW through VideoToolbox

Windows decodes ProRes RAW through FFmpeg, but its **colour is wrong**
(magenta). Apple's decoder is the reference that settles it. Run the check in
`~/Code/decoders/proresraw-mac-check` (its README). It needs the clip
`S_cam/S006/A003C0011_210922_0000_ronin4d_proresraw.mov`. `cut.py` there
made the 30-frame test clip on Windows; the full clip works too.

Fill in its `FEEDBACK.md`, then commit the filled-in form and `out/` on a
branch `mac/proresraw-check` in luna-decoders (force-add `out/`; it holds
renders of one test frame, not camera files).

**Done when:** FEEDBACK.md is answered and pushed. Wiring VideoToolbox into
Luna is a later step; the hook is `proresraw_decode` in
`crates/luna-decode/src/lib.rs`.

**Status 2026-10-24: done** (luna-decoders branch `mac/proresraw-check`). FEEDBACK.md is
answered.
- **Speed:** VideoToolbox decodes the 8K clip at 4–6 ms/frame, with the mosaic as `bp16`.
- **Metadata:** Apple's matches the frame header. Black is 256, and header 0x48 is
  GainFactor.
- **The likely cause of the magenta:** FFmpeg's samples follow a curve where Apple's are
  linear, and Luna takes black as 0.
- **Separate damage:** FFmpeg's decode also has block artifacts that Apple's doesn't.

**Wired into Luna 2026-10-24** (LunaApp branch `mac/proresraw`):
- **How it works:** `proresraw_decode` on macOS reads the `bp16` mosaic through the AVFoundation
  shim and bins it. It maps Apple's metadata onto the mosaic: black/white levels, WB factors,
  the colour matrix as `camera_to_xyz`, and `GainFactor / (white − black)` as `scene_scale`.
- **Result:** the Ronin 4D thumbnails match Apple's own rendering, neutral and with no block
  artifacts.
- **Test:** `proresraw_ronin4d_through_videotoolbox` covers it. The Windows FFmpeg path is
  unchanged.

## 6. Package, sign and update

`cargo xtask package` / `installer` / `publish` have macOS branches in
`xtask/src/main.rs` (`assemble_macos`, DMG, Sparkle appcast) that have never
run. The release flow is in the README ("On the Mac: ..."). Sparkle needs the
same ed25519 key as Windows (`~/.luna/update-key`); never generate a new one.

**Done when:** `cargo xtask package` builds `dist/Luna.app` and it runs a
report headlessly with `Contents/MacOS/luna-cli report ...`.
`installer` and `publish` wait until Luna is a working product; there are no releases
on either platform yet.

**Status 2026-10-24: done** (ad-hoc signed).
- **What it built:** `cargo xtask package` builds `dist/Luna.app` (174 MB), using the
  decoders checkout on `mac/r3d`.
- **The headless report:** `Contents/MacOS/luna-cli report` over the whole corpus wrote the
  PDF, HTML and CSV for 27 clips in 3.2 s, using only the bundled libraries. It exits 1:
  - 2 clips need attention, as expected: ProRes RAW isn't wired up on macOS yet, and the
    VENICE 2 MXF is truncated on purpose.
- **Minimum macOS 26:** `.cargo/config.toml` sets `MACOSX_DEPLOYMENT_TARGET = 26.0`, so
  rustc, every `cc` shim and the R3D add-on the xtask builds all target it, and
  `Info.plist` says 26.0. Before this, the shims were built for the SDK's own 27.

## 7. Luna's window on the Mac

First launch, 2026-10-24 (`open dist/Luna.app`): the home screen draws correctly. That
covers the starfield, the moon, "Drop a camera folder", the report header and the
destination (`~/Documents/Luna Reports`), in an 880×640 window.

Found so far:

- **Window controls: fixed** (merged to master). On macOS the window keeps its
  system frame: a transparent, full-size title bar with no title text, so the traffic lights
  sit on Luna's own bar.
  - Luna's own minimize/maximize/close and resize edges are hidden there.
  - The bar is 32 px tall, the system's height, so the lights line up with the logo.
  - The setting is `native-frame` in `main.slint`, set in `ui.rs`. Windows is unchanged.
  - Checked by screencapture. Dragging and double-clicking the bar haven't been tried.

**Parity check, 2026-10-24.** These were run without clicks, against the whole corpus on the
external drive.

*Code:* every place with a Windows-only implementation has a macOS side: decoding, the
Sparkle updater, the window and the R3D add-on names. Opening files, file dialogs, the
settings and log locations, and worker processes go through cross-platform crates
(`open`, `rfd`, `dirs`) or work the same way on both.

*Behaviour:*

| What | Result on the Mac |
|---|---|
| Report from the window | Launched as `Luna.app --args report <folder> --output <dir>`: the window scans, decodes and shows "Report ready". 1 of 27 clips needs attention: the VENICE 2 MXF, which is cut short on purpose. |
| Report content | All 27 clips have their 3 thumbnails and metadata, including ProRes RAW, R3D, N-RAW and BRAW. The PDF (8 pages), HTML and CSV are all written. |
| Workers | Two worker processes during a run, with no extra Dock icons. |
| Settings, log | `~/Library/Application Support/Luna/` (`settings.toml`, `luna.log`). A run with `--output` leaves the saved destination alone. |
| Updates | Sparkle loads in the packaged app and checks quietly. There's no release to find yet. |
| `luna-cli` | `report`, `inspect` (21 cards) and `mcp` (4 tools) work from inside the bundle. |
| `cargo xtask check` | Passes. |

**Click tests, 2026-10-24.** Run with computer use in background mode, so Shaked could keep
working: clicks go through accessibility or as background mouse events, and nothing takes
over the screen. Reports went to a scratch folder and `settings.toml` was put back afterwards.

| Test | Result |
|---|---|
| Click the window to choose a folder | **Pass.** A click on the home screen opens the native folder sheet (Recents, Documents, the drive) with *New Folder* and *Open*. *Cancel* returns to the home screen with nothing changed. Picking a folder in the sheet: **not run** (see below). |
| Drag a folder onto the window | **Not run** (see below). |
| Settings: every field | **Pass.** Production, Company, Report by, Light/Dark, *Add field* (two rows), label and value, moving a row up, *Choose logo* and its *Cancel*. *Unsaved changes* shows with *Discard* and *Save*; *Save* writes everything to `settings.toml`, custom fields in their new order. |
| Settings after a restart | **Pass.** After Quit and a relaunch the header bar reads "Click Test Prod · Unit Main · Shoot day 7" and Settings shows the same values. |
| Report header in the report | **Pass.** Production, company, author and both custom fields (in order) are in the HTML report. |
| Report header card (expand to edit in place) | **Not run.** Its `TouchArea` has no accessible action, so a background click only selects the group. See below. |
| Logo | **Not run** past opening the chooser (see below). |
| Changing the destination | **Not run** (see below). |
| Report → done screen | **Pass.** "Report ready · 3 clips, no problems" with *Open PDF*, *New report*, *Open HTML*, *Open CSV*, *Show in folder*. PDF, HTML and CSV written. |
| New report | **Pass.** Back to the home screen. |
| Open PDF / HTML / CSV, Show in folder | **Not run:** each opens another app on Shaked's screen (see below). |
| Cancel a running report | **Pass.** At 10 of 27 clips, *Cancel* shows "Cancelled · Nothing was written". No output folder exists and the workers are gone. *Back* returns to the picked folder with *Create report* and *Change folder*. The whole corpus takes about 3 s, so Cancel has to be quick. |
| Traffic lights | **Pass.** Minimize goes to the Dock and the window comes back drawn correctly. Close quits Luna (one-window app); that's fine but is a choice. Full screen: **not run** (see below). |
| Title bar drag and double-click, resizing | **Not run** (see below). |
| *Check for updates*, no release | Sparkle shows its own "Update Error! An error occurred in retrieving update information. Please try again later." with *Cancel Update*. The feed `…/releases/latest/download/appcast-macos.xml` returns 404. It goes away once a release with an appcast exists. Shaked to decide whether that's acceptable before the first release. |

**Checked by hand by Shaked, 2026-10-24:** picking and dropping a folder, the report header
card, changing the destination, Open PDF / HTML / CSV and Show in folder, and ⌘C / ⌘V / ⌘A
all work. Moving, double-clicking and resizing the window work too, and so does the logo
(PNG, JPEG and SVG samples).

**Fixed on `mac/window-polish`:**

- **Native menu bar.** The app menu has About, *Settings…* (⌘,), Services, Hide and Quit,
  and there's a Window menu (Minimize, Zoom, Full Screen, plus what macOS adds there).
  - It's built with `muda`, the crate Slint uses for menus, in `mac_menu.rs`.
  - Slint's own default menu is turned off on the winit backend
    (`i-slint-backend-winit`, pinned to Slint's version). Slint's `MenuBar` can't put
    items in the app menu.
  - With `SLINT_BACKEND` set, Luna uses Slint's backend selector as before, with
    Slint's menu.
  - There's no Edit menu: ⌘C / ⌘V / ⌘A already work in the fields.
- **Full screen.** macOS hides the traffic lights there, but Luna's bar kept their 80 px
  gap, so the logo sat away from the edge. In full screen the bar now starts 16 px in.
  Zoom (double-click, Window → Zoom) was already fine. Shaked confirmed this was the glitch.
- **`--output` shown as the destination.** After `report <folder> --output <dir>`, the
  box shows `<dir>` while that run is on screen, and goes back to the saved destination
  after *New report*. *Change* during that run moves the run too. This one isn't
  Mac-specific.

Checked through the window: *Settings…* opens Settings, Zoom and Full Screen in and out,
and the destination with `--output`. `cargo xtask check` passes.

**Left as found:** with no release, *Check for updates* shows Sparkle's "Update Error!".
The report header card can't be opened from the keyboard or with VoiceOver (Shaked: not
needed).

**Done when** the whole flow (folder → report → open it) works through the window on the
Mac, and the window looks and behaves like a Mac app.

## Reporting back

Both machines push to GitHub, so report through it:

- **Code changes:** branches `mac/<topic>` in LunaApp or luna-decoders, pushed.
- **Findings:** one file per session, `mac-reports/YYYY-MM-DD.md` in
  luna-decoders (private), on a `mac/report-YYYY-MM-DD` branch. Per item:
  - what you ran;
  - pass/fail, with the full text of failures;
  - what you changed, and why;
  - anything Shaked has to decide.

  Don't put SDK files or clips in it.
- Mark items done in this file (on your `mac/` branch) as they pass.
