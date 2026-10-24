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

## 1. ProRes RAW colour and block artifacts (FFmpeg path)

Luna on Windows decodes ProRes RAW through FFmpeg 8.1's `prores_raw`
(`crates/luna-decode/src/proresraw.rs`, header parsing in luna-decoders
`proresraw-ffmpeg/src/lib.rs`). Its thumbnails of the DJI Ronin 4D clip
(`S_cam/S006/A003C0011_210922_0000_ronin4d_proresraw.mov`) are wrong in two
ways:

- **Colour:** they are magenta.
- **Damage:** horizontal block and stripe artifacts cross the cows and the field.

macOS gets this clip right. There, Luna uses Apple's VideoToolbox
(`platform_macos.rs`, since 2026-10-24) and the thumbnails match Apple's own
rendering. So the Mac is the reference.

**Evidence:** all of it is in luna-decoders `proresraw-mac-check/`:

- `FEEDBACK.md`: what Apple's decoder reports, and how its mosaic compares with
  FFmpeg's.
- `out/apple_processed_f0.png`: Apple's rendering of frame 0. **This is the
  target.**
- `out/result.txt`: Apple's metadata and mosaic statistics.
- `luna/*.png`: what Luna on Windows renders today.
- `ffmpeg-frame0-reference.txt`: FFmpeg's frame 0 statistics.

**What Apple's decoder says about this clip** (frame 0; the same on every frame):

| Apple's field | Value | Luna on Windows today |
|---|---|---|
| Black level | 256 | 0 |
| White level | 65535 | 65535 |
| WB red / blue factors | 2.6357 / 1.9136 | the same: header 0x1c / 0x20, used as `[r, 1, b]` |
| Colour matrix (camera → XYZ D65) | identical to header 0x24 | the same |
| Gain factor | 21.112 | header 0x48, read as "unknown" and unused |
| Bayer pattern | RGGB, same sensel phase as FFmpeg | RGGB |

So white balance, the matrix and the CFA are already right. The difference is in
the sample values themselves:

- **Apple's values are linear.** Apple returns linear sensels (black 256, the
  frame's maximum about 10,650).
- **FFmpeg's values are not.** FFmpeg returns the stored 12-bit codes shifted
  to 16 bits.
- **The relationship is a curve, not a scale.** Here is Apple's mean value for
  each FFmpeg code over frame 0:

  | FFmpeg 12-bit code | Apple value |
  |---|---|
  | 64 | 292 |
  | 128 | 325 |
  | 256 | 375 |
  | 512 | 476 |
  | 1024 | 999 |

  Codes 4–33 all land at about 275–330, which is black plus noise.

**Leads, in the order to try them.** These are hypotheses from the numbers; none
is confirmed.

1. **The header table at 0x90 is the curve.** The header holds eight u16 at 0x90:
   `240 512 1024 2048 4096 8192 16384 32768` (see
   `proresraw-mac-check/luna/header_values.txt`), and Apple's mosaic floor is
   exactly 240. To test it:
   - Read the table in `proresraw_ffmpeg::header`.
   - Find the piecewise mapping from 12-bit code to linear value that it implies.
   - Check that mapping against the code → Apple table above.

   Fitting it needs Apple's mosaic. Either rerun `check.swift` on the Mac (it
   writes the `bp16` statistics), or ask a Mac session to dump a full code → value
   table.
2. **Black is 256.** It is not 0, and not FFmpeg's code 4. Use it once the values
   are linear.
3. **Set `scene_scale = gain / (white − black)`,** with the gain from header 0x48.
   That is what the Mac does, and its exposure matches Apple's.
4. **Block artifacts.** These are separate from colour. They aren't in Apple's
   decode of the same frame.
   - Check whether FFmpeg's own CLI decode
     (`ffmpeg -i clip -frames:v 1 -pix_fmt bayer_rggb16le`) shows them too.
     If it does, the fault is FFmpeg's decoder, not Luna's.
   - Another lead: DJI pads samples past the frame's own size, and
     `proresraw_ffmpeg::frame_bytes` trims to the size in the frame header. A wrong
     trim would damage slices.

**Done when** Luna's Windows thumbnails of frames 0, 302 and 704 match
`out/apple_processed_f0.png` in colour (neutral cattle, blue sky), with no block
artifacts.

- Keep `proresraw_ronin4d_through_ffmpeg` passing, and update its expectations if
  black and scale change.
- Then delete this item.

## Later: releases

A release on either platform waits until Luna is a working product. The
release flow is in the README.
