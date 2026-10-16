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
