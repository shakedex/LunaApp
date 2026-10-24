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
- **Apple Silicon only** (`aarch64-apple-darwin`). No Intel Mac support.
- **No reverse engineering** of vendor SDKs (Blackmagic, RED) or formats
  whose licence forbids it. Each decoder's `docs/spec.md` has its rules.
- **Never commit** vendor SDKs, camera files, or the update signing key.
- **Don't push to `master`** of either repo without Shaked's go-ahead. Push
  your work as `mac/<topic>` branches.
- **Headless first.** Don't launch GUI apps unless Shaked says the Mac is free.

## Set up

```bash
mkdir -p ~/Coding && cd ~/Coding
gh auth login && gh auth setup-git   # once: the decoders repo is private
git clone https://github.com/shakedex/LunaApp.git
git clone https://github.com/shakedex/luna-decoders.git decoders
```

Luna's `Cargo.toml` fetches the decoders from GitHub by commit, so
`~/Coding/decoders` is only for working on them. The xtask finds the R3D
add-on at `../decoders/r3d-sdk`, which is why the clone is named `decoders`.

**What Shaked copies over by hand** (never via git):

| What | From the Windows PC | Put it at |
|---|---|---|
| Test clips | `D:\LUNA_TEST\TEST_PROJECT_LUNA\CAMERA` (the whole corpus, or the clips named below) | `~/LunaCorpus/CAMERA` |
| RED SDK subset | `Include/`, `Lib/mac64/`, `Redistributable/mac/REDR3D.dylib` from the R3D SDK | `~/Coding/decoders/r3d-sdk/reference/` |
| Update signing key | `~/.luna/update-key` (only for item 6) | `~/.luna/update-key` |

Blackmagic RAW SDK: install it on the Mac from Blackmagic's site (item 3).
If files arrive quarantined (browser, AirDrop), clear the flag on those copies only:
`xattr -dr com.apple.quarantine <folder>`.

**Environment**, once per shell. `.cargo/config.toml` holds the Windows paths and
doesn't force them, so these win:

```bash
export LUNA_CORPUS=~/LunaCorpus/CAMERA
export BRAW_SDK_DIR="/Applications/Blackmagic RAW/Blackmagic RAW SDK"
export LUNA_R3D_ADDON=~/Coding/decoders/r3d-sdk/target/release/libr3d.dylib
export LUNA_R3D_SDK_DIR=~/Coding/decoders/r3d-sdk/reference/Redistributable/mac
```

## 1. Luna builds and tests on macOS

```bash
cd ~/Coding/LunaApp
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

None of this has run on a Mac. Fix what breaks. Corpus tests skip clips that
are missing, so note which ran.

**Done when:** all three pass, and you've listed the tests that skipped.

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

## 3. Blackmagic RAW on the Mac

The checklist is `~/Coding/decoders/braw-sdk/docs/macos.md`. Its Mac code is
written but has never been compiled. The biggest risk is CFString ownership
in the shim; the checklist says how to measure it.

**Done when:** that checklist is ticked, BRAW tests pass in
`~/Coding/decoders/braw-sdk`, and Luna's `braw_cinema_camera_6k` corpus test
passes. Clip: `D_CAM/D001/D001_09150935_C013.braw`.

## 4. R3D and N-RAW add-on on the Mac

Steps and the expected failure modes are in
`~/Coding/decoders/r3d-sdk/docs/mac-test-handoff.md`. Ignore its bundle and
`~/r3d-test` layout: use `~/Coding/decoders/r3d-sdk` directly, with the SDK
subset in `reference/` as above. Don't re-pin decode checksums over the
Windows values; report Mac values instead.

**Done when:** `cargo test` passes in `r3d-sdk` (one Windows-only test
fewer), and Luna's `r3d_through_the_addon` and `nikon_nraw_through_the_addon`
corpus tests pass. Clips: `R3D/A005_W055_0819CM_001.R3D` and the Nikon `.NEV`.

## 5. ProRes RAW through VideoToolbox

Windows decodes ProRes RAW through FFmpeg, but its **colour is wrong**
(magenta). Apple's decoder is the reference that settles it. Run the check in
`~/Coding/decoders/proresraw-mac-check` (its README). It needs the clip
`S_cam/S006/A003C0011_210922_0000_ronin4d_proresraw.mov`. `cut.py` there
made the 30-frame test clip on Windows; the full clip works too.

Fill in its `FEEDBACK.md`, then commit the filled-in form and `out/` on a
branch `mac/proresraw-check` in luna-decoders (force-add `out/`; it holds
renders of one test frame, not camera files).

**Done when:** FEEDBACK.md is answered and pushed. Wiring VideoToolbox into
Luna is a later step; the hook is `proresraw_decode` in
`crates/luna-decode/src/lib.rs`.

## 6. Package, sign and update

`cargo xtask package` / `installer` / `publish` have macOS branches in
`xtask/src/main.rs` (`assemble_macos`, DMG, Sparkle appcast) that have never
run. The release flow is in the README ("On the Mac: ..."). Sparkle needs the
same ed25519 key as Windows (`~/.luna/update-key`); never generate a new one.

**Done when:** `cargo xtask package` builds `dist/Luna.app` and it runs a
report headlessly with `Contents/MacOS/luna-cli report ...`.
`installer` and `publish` wait until Shaked asks for a release.

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
