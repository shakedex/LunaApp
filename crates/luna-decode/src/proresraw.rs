//! ProRes RAW through FFmpeg's LGPL `prores_raw` decoder (the private
//! `proresraw-ffmpeg` crate), Windows only. FFmpeg's DLLs ship in
//! [`util::lib_dir`] and are loaded at run time, so they stay replaceable;
//! `LUNA_FFMPEG_DIR` (at run time, or as it was when Luna was built) points
//! development builds at an FFmpeg folder.
//!
//! Frames come back as the full Bayer mosaic of linear sensels (FFmpeg 9.0)
//! and are binned to thumbnail scale. White balance, the camera → XYZ matrix
//! and the gain come from the frame header, black and white are the levels
//! Apple's VideoToolbox reports; together they match the macOS path.

use std::path::PathBuf;
use std::sync::OnceLock;

use luna_formats::{ClipFiles, ColourHint, FrameIndex, Probe, Transfer};
use proresraw_ffmpeg as prr;

use crate::mosaic::{self, Rect};
use crate::util;
use crate::{Cfa, DecodeError, Decoded, Frame};

/// FFmpeg, loaded once per process.
fn ffmpeg() -> Result<&'static prr::Ffmpeg, DecodeError> {
    static FF: OnceLock<Result<prr::Ffmpeg, String>> = OnceLock::new();
    FF.get_or_init(load)
        .as_ref()
        .map_err(|m| DecodeError::Unsupported(m.clone()))
}

fn load() -> Result<prr::Ffmpeg, String> {
    let mut last = String::from("no folder to look in");
    for dir in dirs() {
        match util::guard("FFmpeg", || {
            prr::Ffmpeg::load(&dir).map_err(|e| DecodeError::Failed(e.to_string()))
        }) {
            Ok(ff) => return Ok(ff),
            Err(e) => last = e.to_string(),
        }
    }
    Err(format!(
        "The FFmpeg libraries for ProRes RAW are missing from this installation ({last})"
    ))
}

fn dirs() -> Vec<PathBuf> {
    let mut dirs: Vec<PathBuf> = util::lib_dir().into_iter().collect();
    let dev = [
        std::env::var_os("LUNA_FFMPEG_DIR").map(PathBuf::from),
        option_env!("LUNA_FFMPEG_DIR").map(PathBuf::from),
    ];
    for d in dev.into_iter().flatten() {
        if !dirs.contains(&d) {
            dirs.push(d);
        }
    }
    dirs
}

/// What every frame is: linear camera RGB; the matrix (when the header has
/// one) carries the colour, so no gamut is assumed.
const HINT: ColourHint = ColourHint {
    transfer: Some(Transfer::Linear),
    gamut: None,
};

pub(crate) fn decode(
    clip: &ClipFiles,
    probe: &Probe,
    positions: &[u64],
    min_width: u32,
) -> Vec<Result<Frame, DecodeError>> {
    let FrameIndex::Ranges(ranges) = &probe.frames else {
        return util::all_err(
            positions.len(),
            DecodeError::Failed("ProRes RAW frames were not indexed".into()),
        );
    };
    let ff = match ffmpeg() {
        Ok(ff) => ff,
        Err(e) => return util::all_err(positions.len(), e),
    };
    let mut dec = match util::guard("ProRes RAW", || {
        ff.decoder().map_err(|e| DecodeError::Failed(e.to_string()))
    }) {
        Ok(d) => d,
        Err(e) => return util::all_err(positions.len(), e),
    };
    util::per_frame("ProRes RAW", positions, ranges.len() as u64, |i| {
        let r = ranges[i as usize];
        let bytes = util::read_range(&clip.main, r.offset, r.len)?;
        let header = prr::header(&bytes);
        let raw = dec.decode(&bytes).map_err(|e| match e {
            prr::Error::Format(m) => {
                DecodeError::Failed(format!("The ProRes RAW frame is damaged: {m}"))
            }
            other => DecodeError::Failed(other.to_string()),
        })?;
        to_frame(raw, header, min_width)
    })
}

fn to_frame(
    raw: prr::Frame,
    header: Option<prr::Header>,
    min_width: u32,
) -> Result<Frame, DecodeError> {
    let (w, h) = (raw.width as usize, raw.height as usize);
    if w < 2 || h < 2 || raw.data.len() != w * h {
        return Err(DecodeError::Failed(format!(
            "FFmpeg returned an inconsistent ProRes RAW frame ({w}×{h}, {} samples)",
            raw.data.len()
        )));
    }
    let f = mosaic::bin_factor(w, min_width);
    let full = Rect {
        x: 0,
        y: 0,
        width: w,
        height: h,
    };
    let (data, bw, bh) = mosaic::crop_bin(&raw.data, w, h, full, f);
    let (black, white) = (prr::BLACK as f32, prr::WHITE as f32);
    Ok(Frame {
        image: Decoded::Mosaic {
            width: bw as u32,
            height: bh as u32,
            cfa: match raw.cfa {
                prr::Cfa::Rggb => Cfa::Rggb,
                prr::Cfa::Grbg => Cfa::Grbg,
                prr::Cfa::Gbrg => Cfa::Gbrg,
                prr::Cfa::Bggr => Cfa::Bggr,
            },
            data,
            black,
            white,
            wb: header.as_ref().map(|h| h.wb),
            camera_to_xyz: header.as_ref().map(|h| h.camera_to_xyz),
            // As on macOS: the gain is scene-linear per unit above black over
            // the clip range.
            scene_scale: header
                .as_ref()
                .filter(|h| h.gain > 0.0)
                .map(|h| h.gain / (white - black)),
        },
        hint: HINT,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frame_is_binned_and_keeps_header_colour() {
        let (w, h) = (64usize, 8usize);
        let raw = prr::Frame {
            width: w as u32,
            height: h as u32,
            cfa: prr::Cfa::Rggb,
            data: vec![1000; w * h],
        };
        let header = prr::Header {
            creator: *b"DJI0",
            width: w as u16,
            height: h as u16,
            wb: [2.0, 1.0, 1.5],
            camera_to_xyz: [1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0],
            colour_temperature: 5600,
            gain: 21.0,
        };
        let f = to_frame(raw, Some(header), 8).unwrap();
        let Decoded::Mosaic {
            width,
            cfa,
            wb,
            camera_to_xyz,
            black,
            scene_scale,
            ..
        } = f.image
        else {
            panic!("not a mosaic");
        };
        assert_eq!((width, cfa, black), (16, Cfa::Rggb, 256.0));
        assert_eq!(scene_scale, Some(21.0 / (65535.0 - 256.0)));
        assert_eq!(wb, Some([2.0, 1.0, 1.5]));
        assert!(camera_to_xyz.is_some());
        assert_eq!(f.hint.transfer, Some(Transfer::Linear));
    }

    #[test]
    fn inconsistent_frame_is_an_error() {
        let raw = prr::Frame {
            width: 8,
            height: 8,
            cfa: prr::Cfa::Rggb,
            data: vec![0; 10],
        };
        assert!(to_frame(raw, None, 4).is_err());
    }
}
