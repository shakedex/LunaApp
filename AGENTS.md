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
  **Unreleased** in `CHANGELOG.md`, in the same commit.
- Private decoders live in the private `shakedex/luna-decoders` repo, pinned by
  `rev` in `Cargo.toml`: push a decoder change there first, then move the pin.

## Procedures and open work

Kept in the private luna-decoders checkout beside this one (`../decoders`): read its
`AGENTS.md` before moving the decoders pin, releasing, or doing Mac or Windows work.
Open work is issues there, labelled `mac` or `windows`.
