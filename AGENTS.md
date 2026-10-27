# Luna

Native Rust app (Windows + macOS) that makes camera reports.

## Before you commit

- `cargo fmt --all`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo test --workspace`

## Rules

- Add dependencies with `cargo add` (latest), never by hand-writing versions.
- Slint lives only in `luna-app`; platform APIs only in `luna-decode` and `luna-update`.
- Camera files never enter the repository.
- Every change users or the release would notice gets a line under
  **Unreleased** in `CHANGELOG.md`, in the same commit. Moving the decoders pin
  updates that section's decoders commit (and luna-decoders' own `CHANGELOG.md`).
- Private decoders live in the private `shakedex/luna-decoders` repo, pinned by
  `rev` in `Cargo.toml`: push a decoder change there first, then move the pin.
- Open Mac and Windows work is listed in the private `shakedex/luna-decoders`
  repo: `docs/macos.md` and `docs/windows.md`.
