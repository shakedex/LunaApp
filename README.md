<div align="center">

<img src="Assets/luna-logo-lg.webp" alt="Luna" width="160" />

# Luna

**Camera reports from camera originals. Offline, on Windows and macOS.**

[![License: MIT](https://img.shields.io/badge/license-MIT-blue)](LICENSE) · [Disclaimer](DISCLAIMER.md)

</div>

---

Pick a source folder, either one camera card or a whole drive of cards from several
cameras, and Luna writes a camera report as PDF, HTML and CSV: every clip
grouped by camera and card, with its metadata and a strip of thumbnails.

## Develop

Requires a stable Rust toolchain, the private decoder checkouts in
`../decoders/`, and the Blackmagic RAW SDK. `.cargo/config.toml` points
`cargo run` and `cargo test` at the R3D add-on and the camera corpus.

| Command                         | What it does                                                 |
| ------------------------------- | ------------------------------------------------------------ |
| `cargo run -p luna-app`         | Run Luna from source                                         |
| `cargo xtask check`             | fmt check, clippy and tests; run before every commit         |
| `cargo xtask package`           | Build the app into `dist/` (`dist\Luna` or `dist/Luna.app`)  |
| `cargo xtask installer`         | Package, then build the installer into `dist/`               |
| `cargo xtask release <bump>`    | Check, bump the version, build the installer, commit and tag |
| `cargo xtask publish`           | Push the tag and upload this machine's installer to GitHub   |

`<bump>` is `patch`, `minor` or an exact version (`0.4.0`). The installers are
`Luna-<version>-windows-x64-setup.exe` (needs [Inno Setup](https://jrsoftware.org/isdl.php))
and `Luna-<version>-macos.dmg`. `cargo xtask` lists the environment variables
that override the SDK and tool locations.

### Releasing

1. On Windows, from a clean `master`: `cargo xtask release minor`. This leaves
   a release commit, a `v<version>` tag and the Windows installer in `dist/`.
   Nothing is pushed.
2. `cargo xtask publish` pushes `master` and the tag, and creates the GitHub
   release with the Windows installer.
3. On the Mac: `git fetch --tags && git checkout v<version>`, then
   `cargo xtask installer && cargo xtask publish` adds the DMG to the same release.

There is no GitHub CI: the private decoders and SDKs exist only on the build
machines, so `cargo xtask check` runs locally.

## Repo layout

| Path                  | What it is                                     |
| --------------------- | ---------------------------------------------- |
| `crates/luna-report`  | Report model and the PDF, HTML and CSV writers |

The earlier browser app is preserved at tag `luna-web-final`. The original
.NET desktop app is at branch `archive/dotnet` (tag `dotnet-final`).
