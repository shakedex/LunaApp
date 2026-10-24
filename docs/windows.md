# Windows: everything still open

The one list of Windows work. Written for a Claude Code session on Shaked's
Windows PC. Each item says where its details are and what "done" means. The
Mac's list is [macos.md](macos.md).

## Ground rules

- Camera files never enter a repository. The corpus is
  `D:\LUNA_TEST\TEST_PROJECT_LUNA\CAMERA` (`LUNA_CORPUS` in `.cargo/config.toml`).
- A decoder change goes to the private `shakedex/luna-decoders` repo first; then
  move the `rev` pin in Luna's `Cargo.toml` (see `AGENTS.md`).
- Before every commit: `cargo fmt --all`,
  `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`.

## Later: releases

A release on either platform waits until Luna is a working product. The
release flow is in the README.
