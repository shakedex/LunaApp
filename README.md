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

Requires a stable Rust toolchain.

```bash
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all
```

## Repo layout

| Path                  | What it is                                     |
| --------------------- | ---------------------------------------------- |
| `crates/luna-report`  | Report model and the PDF, HTML and CSV writers |

The earlier browser app is preserved at tag `luna-web-final`. The original
.NET desktop app is at branch `archive/dotnet` (tag `dotnet-final`).
