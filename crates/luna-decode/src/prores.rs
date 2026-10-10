//! ProRes (MOV or MXF) through our own decoder, `luna-prores`. Frames are read
//! from the parser's byte-range index and decoded at a reduced scale.

use luna_formats::{ClipFiles, ColourHint, FrameIndex, Probe};

use crate::util::{self, gamut_for, matrix_for, transfer_for};
use crate::{DecodeError, Decoded, Frame};

pub(crate) fn decode(
    clip: &ClipFiles,
    probe: &Probe,
    positions: &[u64],
    min_width: u32,
) -> Vec<Result<Frame, DecodeError>> {
    let FrameIndex::Ranges(ranges) = &probe.frames else {
        return util::all_err(
            positions.len(),
            DecodeError::Failed("ProRes frames were not indexed".into()),
        );
    };
    let width = if probe.layout.stored_width > 0 {
        probe.layout.stored_width
    } else {
        probe.meta.width.unwrap_or(0)
    };
    let downscale = downscale_for(width, min_width);
    util::per_frame("ProRes", positions, ranges.len() as u64, |i| {
        let r = ranges[i as usize];
        let bytes = util::read_range(&clip.main, r.offset, r.len)?;
        let pic = luna_prores::decode(&bytes, downscale).map_err(|e| match e {
            luna_prores::Error::Unsupported(m) => {
                DecodeError::Unsupported(format!("This ProRes frame isn't supported: {m}"))
            }
            luna_prores::Error::Corrupt(m) => {
                DecodeError::Failed(format!("The ProRes frame is damaged: {m}"))
            }
        })?;
        to_frame(pic, probe.meta.colour)
    })
}

/// The coarsest of ⅛, ¼, ½ and full scale that is still `min_width` wide.
fn downscale_for(width: u32, min_width: u32) -> u8 {
    [8u8, 4, 2]
        .into_iter()
        .find(|&d| width / d as u32 >= min_width && width > 0)
        .unwrap_or(1)
}

fn to_frame(pic: luna_prores::Picture, clip_hint: ColourHint) -> Result<Frame, DecodeError> {
    let (w, h) = (pic.width, pic.height);
    let cw = if pic.chroma_format == 3 {
        w
    } else {
        w.div_ceil(2)
    };
    let luma = w as usize * h as usize;
    let chroma = cw as usize * h as usize;
    if w == 0 || h == 0 || pic.y.len() != luma || pic.cb.len() != chroma || pic.cr.len() != chroma {
        return Err(DecodeError::Failed(format!(
            "The ProRes decoder returned inconsistent planes ({w}×{h}, chroma format {})",
            pic.chroma_format
        )));
    }
    let matrix = matrix_for(pic.matrix_coefficients, h);
    let hint = ColourHint {
        transfer: clip_hint
            .transfer
            .or(transfer_for(pic.transfer_characteristic)),
        gamut: clip_hint.gamut.or(gamut_for(pic.color_primaries)),
    };
    Ok(Frame {
        image: Decoded::Ycbcr {
            width: w,
            height: h,
            chroma_width: cw,
            chroma_height: h,
            bits: pic.bits,
            y: pic.y,
            cb: pic.cb,
            cr: pic.cr,
            matrix,
            // ProRes is always limited ("video") range.
            full_range: false,
        },
        hint,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::YcbcrMatrix;
    use luna_formats::{Gamut, Transfer};

    fn pic(w: u32, h: u32, chroma_format: u8, matrix: u8) -> luna_prores::Picture {
        let cw = if chroma_format == 3 { w } else { w.div_ceil(2) };
        luna_prores::Picture {
            width: w,
            height: h,
            chroma_format,
            y: vec![64; (w * h) as usize],
            cb: vec![512; (cw * h) as usize],
            cr: vec![512; (cw * h) as usize],
            bits: 10,
            color_primaries: 1,
            transfer_characteristic: 1,
            matrix_coefficients: matrix,
        }
    }

    #[test]
    fn picks_coarsest_scale_still_wide_enough() {
        assert_eq!(downscale_for(8192, 640), 8);
        assert_eq!(downscale_for(3840, 640), 4);
        assert_eq!(downscale_for(1920, 640), 2);
        assert_eq!(downscale_for(1280, 640), 2);
        assert_eq!(downscale_for(1000, 640), 1);
        assert_eq!(downscale_for(0, 640), 1);
    }

    #[test]
    fn maps_422_planes_and_matrix() {
        let f = to_frame(pic(240, 135, 2, 1), ColourHint::default()).unwrap();
        let Decoded::Ycbcr {
            width,
            chroma_width,
            chroma_height,
            matrix,
            full_range,
            bits,
            ..
        } = f.image
        else {
            panic!("not ycbcr")
        };
        assert_eq!((width, chroma_width, chroma_height), (240, 120, 135));
        assert_eq!(matrix, YcbcrMatrix::Bt709);
        assert!(!full_range);
        assert_eq!(bits, 10);
        assert_eq!(f.hint.transfer, Some(Transfer::Rec709));
        assert_eq!(f.hint.gamut, Some(Gamut::Rec709));
    }

    #[test]
    fn maps_444_and_2020() {
        let f = to_frame(pic(64, 32, 3, 9), ColourHint::default()).unwrap();
        let Decoded::Ycbcr {
            chroma_width,
            matrix,
            ..
        } = f.image
        else {
            panic!("not ycbcr")
        };
        assert_eq!(chroma_width, 64);
        assert_eq!(matrix, YcbcrMatrix::Bt2020);
    }

    #[test]
    fn clip_hint_wins_over_frame_header() {
        let clip = ColourHint {
            transfer: Some(Transfer::LogC3),
            gamut: Some(Gamut::ArriWideGamut3),
        };
        let f = to_frame(pic(64, 32, 2, 1), clip).unwrap();
        assert_eq!(f.hint, clip);
    }

    #[test]
    fn unspecified_matrix_by_size() {
        assert_eq!(matrix_for(2, 1080), YcbcrMatrix::Bt709);
        assert_eq!(matrix_for(2, 480), YcbcrMatrix::Bt601);
    }

    #[test]
    fn inconsistent_planes_are_errors() {
        let mut p = pic(64, 32, 2, 1);
        p.cb.pop();
        assert!(to_frame(p, ColourHint::default()).is_err());
    }
}
