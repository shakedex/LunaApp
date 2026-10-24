<div align="center">

<img src="Assets/luna-logo-lg.webp" alt="Luna" width="160" />

# Luna

**Camera reports from camera originals. Offline, on Windows and macOS.**

[Download](https://github.com/shakedex/LunaApp/releases/latest) · [License](#license) · [Disclaimer](DISCLAIMER.md)

</div>

---

Pick a source folder, either one camera card or a whole drive of cards from
several cameras, and Luna writes a camera report as PDF, HTML and CSV: every
clip grouped by camera and card, with its metadata and a strip of thumbnails.

Luna works without a network connection and doesn't change the source
files. With **Also save on the source drive** on in Settings (off by
default), each report is also written to `Luna\<report name>\` at the root of
the source drive (`D:\Luna\…`, `/Volumes/<drive>/Luna/…`), unless the report
destination is already on that drive.

## Formats

| Camera               | Formats                                       |
| -------------------- | --------------------------------------------- |
| ARRI                 | ARRIRAW (`.ari`, `.arx`), ARRICORE, ProRes    |
| Sony                 | X-OCN, XAVC (MXF)                             |
| RED                  | R3D                                           |
| Blackmagic Design    | Blackmagic RAW                                |
| Canon                | Cinema RAW Light (`.crm`), XF-AVC, XF-HEVC    |
| Nikon                | N-RAW (`.nev`), R3D NE                        |
| Apple ProRes         | ProRes, ProRes RAW                            |
| Other cameras        | H.264 and HEVC in MOV and MP4                 |

Sidecars read alongside the clips: Sony and Canon XML, RED RMD, Blackmagic
RAW sidecars and ALE.

Every format above reads on both Windows and macOS.

## Install

Installers are on the [releases page](https://github.com/shakedex/LunaApp/releases).
No release is published yet; the first will be `2026.10.0`.

- Windows: `Luna-<version>-windows-x64-setup.exe`. The installer is unsigned,
  so SmartScreen shows "Windows protected your PC": choose **More info**, then
  **Run anyway**.
- macOS: `Luna-<version>-macos.dmg`, signed with a Developer ID and notarized.

Luna updates itself from signed feeds (WinSparkle on Windows, Sparkle on
macOS).

`luna-cli` is installed next to the app: `luna-cli report <folders…>` writes
a report without a window, `luna-cli inspect <folders…>` lists cameras, cards
and clips, and `luna-cli mcp` serves both over MCP for AI agents.

## Develop

This repository is Luna's open-source core. Building the full app also needs:

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

### Releasing

1. Write the release's description in `packaging/release-notes/<version>.md`
   and commit it.
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

## License

The source in this repository is under the [MIT License](LICENSE).

The installers also contain components that are not MIT: Luna's closed-source
decoders, the R3D add-on under its own [EULA](packaging/R3D-EULA.txt), the
Blackmagic RAW SDK libraries, and FFmpeg under the LGPL. [NOTICE](NOTICE)
lists each one with its licence.

Earlier versions: the browser app is at tag `luna-web-final` and the .NET app
at branch `archive/dotnet` (tag `dotnet-final`). Neither is maintained.
