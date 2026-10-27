# Developing Luna

This repository is Luna's open-source core. For using Luna, see the
[README](README.md). Building the full app also needs:

- the private `shakedex/luna-decoders` repository (X-OCN, Cinema RAW Light,
  ARRIRAW/ARRICORE, and the Blackmagic RAW, RED and FFmpeg bindings), pinned
  by commit in `Cargo.toml`;
- the Blackmagic RAW SDK and the RED R3D SDK, installed locally;
- a stable Rust toolchain.

`.cargo/config.toml` points `cargo run` and `cargo test` at the R3D add-on and
the camera corpus. `cargo xtask` lists the environment variables that override
the SDK and tool locations.

| Command                 | What it does                                                     |
| ----------------------- | ---------------------------------------------------------------- |
| `cargo run -p luna-app` | Run Luna from source                                             |
| `cargo xtask check`     | fmt check, clippy and tests; run before every commit             |
| `cargo xtask package`   | Build the app into `dist/` (`dist\Luna` or `dist/Luna.app`)      |
| `cargo xtask installer` | Package, then build the installer into `dist/`                   |
| `cargo xtask release`   | Check, set the next version, build the installer, commit and tag |
| `cargo xtask publish`   | Push the tag and upload this machine's installer to GitHub       |

Versions are calendar based, `YEAR.MONTH.N`: the first release in October 2026
is `2026.10.0`, the next that month `2026.10.1`, and November starts at
`2026.11.0`. `cargo xtask release 2026.11.0` sets one exactly. The Windows
installer needs [Inno Setup](https://jrsoftware.org/isdl.php).

## Releasing

1. In [`CHANGELOG.md`](CHANGELOG.md), turn **Unreleased** into the version's
   heading (date, tag, decoders commit) and start a new, empty **Unreleased**.
   Do the same in luna-decoders' `CHANGELOG.md`. Then write the release's
   description for users in `packaging/release-notes/<version>.md`, from those
   entries, and commit it.
2. On Windows, from a clean `master`: `cargo xtask release`. This leaves
   a release commit, a `v<version>` tag and the Windows installer in `dist/`.
   Nothing is pushed.
3. `cargo xtask publish` pushes `master` and the tag, and creates the GitHub
   release as a draft with the Windows installer and that description.
4. On the Mac: `git fetch --tags && git checkout v<version>`, then
   `cargo xtask installer && cargo xtask publish`. The installer is signed with
   the keychain's Developer ID Application identity and notarized (app, then
   DMG, both stapled). `publish` adds the DMG and the update zip, and once both
   platforms are on the release it leaves draft and becomes the latest, which
   is where both update feeds point.

The Windows installer is unsigned. On the Mac, notarization reads the
`notarytool` keychain profile `luna-notary` (`LUNA_NOTARY_PROFILE` names
another), created once with
`xcrun notarytool store-credentials luna-notary --apple-id <id> --team-id <team>`.

There is no GitHub CI: the private decoders and SDKs exist only on the build
machines, so `cargo xtask check` runs locally.

## Repo layout

| Path                  | What it is                                                         |
| --------------------- | ------------------------------------------------------------------ |
| `crates/luna-app`     | The desktop window (Slint)                                         |
| `crates/luna-cli`     | `luna-cli`: headless `report` and `inspect`, and the MCP server    |
| `crates/luna-core`    | A run: scan, group clips into cameras and cards, worker pool, output |
| `crates/luna-worker`  | One clip job in a worker process: metadata, frames, colour, JPEG   |
| `crates/luna-formats` | Container and metadata parsers (MXF, QuickTime, sidecars)          |
| `crates/luna-decode`  | Thumbnail decoding: platform decoders and the vendor decoders      |
| `crates/luna-prores`  | ProRes decoder, written from SMPTE RDD 36                          |
| `crates/luna-h264`    | H.264 intra-picture decoder, written from ITU-T H.264              |
| `crates/luna-color`   | Decoded frames to Rec.709 thumbnails                               |
| `crates/luna-report`  | Report model and the PDF, HTML and CSV writers                     |
| `crates/luna-update`  | Updates through WinSparkle and Sparkle                             |
| `xtask`               | Build, package and release tasks                                   |
| `packaging`           | Installer script, R3D add-on EULA, update signing key              |
