//! Small helpers shared by the backends.

use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::{Path, PathBuf};

use luna_formats::{Gamut, Transfer};

use crate::{DecodeError, Frame, YcbcrMatrix};

/// Runs `f`, turning a panic inside a decoder into an error instead of taking
/// the worker down.
pub(crate) fn guard<T>(
    what: &str,
    f: impl FnOnce() -> Result<T, DecodeError>,
) -> Result<T, DecodeError> {
    match catch_unwind(AssertUnwindSafe(f)) {
        Ok(r) => r,
        Err(payload) => {
            let detail = payload
                .downcast_ref::<&str>()
                .map(|s| s.to_string())
                .or_else(|| payload.downcast_ref::<String>().cloned())
                .unwrap_or_default();
            Err(DecodeError::Failed(if detail.is_empty() {
                format!("The {what} decoder crashed")
            } else {
                format!("The {what} decoder crashed: {detail}")
            }))
        }
    }
}

/// The same error for every position.
pub(crate) fn all_err(n: usize, e: DecodeError) -> Vec<Result<Frame, DecodeError>> {
    vec![Err(e); n]
}

/// A 0-based frame number clamped into a clip of `count` frames.
pub(crate) fn clamp(position: u64, count: u64) -> u64 {
    position.min(count.saturating_sub(1))
}

/// Decodes each position independently (clamped to `count`), so one bad frame
/// never stops the others and a panic becomes that frame's error.
pub(crate) fn per_frame(
    what: &str,
    positions: &[u64],
    count: u64,
    mut f: impl FnMut(u64) -> Result<Frame, DecodeError>,
) -> Vec<Result<Frame, DecodeError>> {
    if count == 0 {
        return all_err(
            positions.len(),
            DecodeError::Failed("The clip has no frames".into()),
        );
    }
    positions
        .iter()
        .map(|&p| guard(what, || f(clamp(p, count))))
        .collect()
}

/// The path as UTF-8, which the vendor decoders need.
pub(crate) fn path_str(p: &Path) -> Result<&str, DecodeError> {
    p.to_str()
        .ok_or_else(|| DecodeError::Failed(format!("The path is not valid UTF-8: {}", p.display())))
}

/// The folder holding the running executable.
pub(crate) fn exe_dir() -> Option<PathBuf> {
    std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(Path::to_path_buf))
}

/// Reads `len` bytes at `offset`.
pub(crate) fn read_range(path: &Path, offset: u64, len: u64) -> Result<Vec<u8>, DecodeError> {
    let io = |e: std::io::Error| DecodeError::Failed(format!("Could not read the frame: {e}"));
    // Coded frames are megabytes at most; refuse absurd ranges from a bad index.
    if len > 1 << 30 {
        return Err(DecodeError::Failed(format!(
            "The frame index is damaged ({len}-byte frame)"
        )));
    }
    let mut f = File::open(path).map_err(io)?;
    f.seek(SeekFrom::Start(offset)).map_err(io)?;
    let mut buf = vec![0u8; len as usize];
    f.read_exact(&mut buf).map_err(io)?;
    Ok(buf)
}

pub(crate) fn failed(e: impl std::fmt::Display) -> DecodeError {
    DecodeError::Failed(e.to_string())
}

/// ITU-T H.273 matrix code; unspecified falls back on size (SD → 601).
pub(crate) fn matrix_for(code: u8, height: u32) -> YcbcrMatrix {
    match code {
        1 => YcbcrMatrix::Bt709,
        9 => YcbcrMatrix::Bt2020,
        5 | 6 => YcbcrMatrix::Bt601,
        _ if height >= 720 => YcbcrMatrix::Bt709,
        _ => YcbcrMatrix::Bt601,
    }
}

/// ITU-T H.273 transfer characteristics Luna can display.
pub(crate) fn transfer_for(code: u8) -> Option<Transfer> {
    match code {
        1 | 6 | 14 | 15 => Some(Transfer::Rec709),
        16 => Some(Transfer::Pq),
        18 => Some(Transfer::Hlg),
        _ => None,
    }
}

/// ITU-T H.273 colour primaries Luna can display.
pub(crate) fn gamut_for(code: u8) -> Option<Gamut> {
    match code {
        1 => Some(Gamut::Rec709),
        9 => Some(Gamut::Rec2020),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clamps_to_last_frame() {
        assert_eq!(clamp(0, 10), 0);
        assert_eq!(clamp(9, 10), 9);
        assert_eq!(clamp(10, 10), 9);
        assert_eq!(clamp(u64::MAX, 10), 9);
        assert_eq!(clamp(5, 0), 0);
    }

    #[test]
    fn panics_become_errors() {
        let r: Result<(), _> = guard("test", || panic!("boom"));
        assert_eq!(
            r,
            Err(DecodeError::Failed("The test decoder crashed: boom".into()))
        );
    }

    #[test]
    fn per_frame_isolates_failures() {
        let out = per_frame("test", &[0, 1, 2, 50], 3, |i| match i {
            1 => panic!("bad frame"),
            2 => Err(DecodeError::Failed("nope".into())),
            _ => Ok(Frame {
                image: crate::Decoded::Display8 {
                    width: 1,
                    height: 1,
                    data: vec![i as u8; 3],
                },
                hint: Default::default(),
            }),
        });
        assert_eq!(out.len(), 4);
        assert!(out[0].is_ok());
        assert!(out[1].is_err());
        assert!(out[2].is_err());
        // 50 clamps to frame 2, which fails
        assert_eq!(out[3], Err(DecodeError::Failed("nope".into())));
    }

    #[test]
    fn empty_clip_fails_every_position() {
        let out = per_frame("test", &[0, 1], 0, |_| unreachable!());
        assert_eq!(out.len(), 2);
        assert!(out.iter().all(|r| r.is_err()));
    }
}
