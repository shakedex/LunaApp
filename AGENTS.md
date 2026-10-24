# Luna

Native Rust app (Windows + macOS) that turns camera originals into a camera report.

## Before you commit

- `cargo fmt --all`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo test --workspace`

## Rules

- Add dependencies with `cargo add` (latest), never by hand-writing versions.
- Slint lives only in `luna-app`; platform APIs only in `luna-decode` and `luna-update`.
- Camera files never enter the repository.
- Private decoders live in the private `shakedex/luna-decoders` repo, pinned by
  `rev` in `Cargo.toml`: push a decoder change there first, then move the pin.
- Open Mac work is listed in `docs/macos.md`, open Windows work in `docs/windows.md`.
