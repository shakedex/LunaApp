//! H.264 through our own intra decoder, `luna-h264`: XAVC-I and XF-AVC in
//! MXF, and High 4:2:2 MOV/MP4 the platform decoder refuses. Access units
//! come from the parser's byte-range index; for each position the nearest
//! intra (I or IDR) access unit at or before it is decoded, since P and B
//! pictures are out of the decoder's scope.

use luna_formats::{ClipFiles, ColourHint, FrameIndex, Probe};
use luna_h264::{DecoderConfig, FrameKind};

use crate::util::{self, gamut_for, matrix_for, transfer_for};
use crate::{DecodeError, Decoded, Frame};

/// How far back to look for an intra access unit (camera GOPs are at most
/// a few seconds).
const MAX_WALK_BACK: u64 = 600;
/// How far past the start to look when a clip opens with leading pictures.
const MAX_LEAD: u64 = 60;
/// Bytes read to classify an access unit; enough to reach the first slice
/// header behind the parameter sets and SEI of camera streams.
const PREFIX: u64 = 16 * 1024;

pub(crate) fn decode(
    clip: &ClipFiles,
    probe: &Probe,
    positions: &[u64],
    _min_width: u32,
) -> Vec<Result<Frame, DecodeError>> {
    let FrameIndex::Ranges(ranges) = &probe.frames else {
        return util::all_err(
            positions.len(),
            DecodeError::Unsupported("H.264 frames were not indexed".into()),
        );
    };
    let config = match &probe.codec_config {
        Some(rec) => match DecoderConfig::from_avcc(rec) {
            Ok(c) => c,
            Err(e) => return util::all_err(positions.len(), map_err(e)),
        },
        None => DecoderConfig::annex_b(),
    };
    util::per_frame("H.264", positions, ranges.len() as u64, |i| {
        let k = nearest_intra(clip, ranges, i, &config)?;
        let r = ranges[k as usize];
        let au = util::read_range(&clip.main, r.offset, r.len)?;
        let pic = luna_h264::decode_access_unit(&au, &config).map_err(map_err)?;
        to_frame(pic, probe.meta.colour)
    })
}

/// The nearest I or IDR access unit at or before `i`. When the clip opens
/// with leading pictures (open GOP), so nothing intra comes before `i`, the
/// first intra access unit after it stands in.
fn nearest_intra(
    clip: &ClipFiles,
    ranges: &[luna_formats::ByteRange],
    i: u64,
    config: &DecoderConfig,
) -> Result<u64, DecodeError> {
    let is_intra = |k: u64| -> Result<bool, DecodeError> {
        let r = ranges[k as usize];
        let head = util::read_range(&clip.main, r.offset, r.len.min(PREFIX))?;
        let kind = match luna_h264::access_unit_kind_prefix(&head, config) {
            Some(kind) => kind,
            None if r.len > PREFIX => {
                let au = util::read_range(&clip.main, r.offset, r.len)?;
                luna_h264::access_unit_kind(&au, config)
            }
            None => FrameKind::Other,
        };
        Ok(kind != FrameKind::Other)
    };
    let mut k = i;
    loop {
        if is_intra(k)? {
            return Ok(k);
        }
        if i - k >= MAX_WALK_BACK {
            break;
        }
        if k == 0 {
            let last = (ranges.len() as u64).min(i + 1 + MAX_LEAD);
            for k in i + 1..last {
                if is_intra(k)? {
                    return Ok(k);
                }
            }
            break;
        }
        k -= 1;
    }
    Err(DecodeError::Unsupported(
        "No intra frame of this H.264 clip is near the frame".into(),
    ))
}

fn map_err(e: luna_h264::Error) -> DecodeError {
    match e {
        luna_h264::Error::Unsupported(m) => {
            DecodeError::Unsupported(format!("This H.264 frame isn't supported: {m}"))
        }
        luna_h264::Error::Corrupt(m) => {
            DecodeError::Failed(format!("The H.264 frame is damaged: {m}"))
        }
    }
}

fn to_frame(pic: luna_h264::Picture, clip_hint: ColourHint) -> Result<Frame, DecodeError> {
    let (w, h) = (pic.width, pic.height);
    let (cw, ch) = (pic.chroma_width(), pic.chroma_height());
    if w == 0
        || h == 0
        || pic.y.len() != (w * h) as usize
        || pic.cb.len() != (cw * ch) as usize
        || pic.cr.len() != (cw * ch) as usize
    {
        return Err(DecodeError::Failed(format!(
            "The H.264 decoder returned inconsistent planes ({w}×{h})"
        )));
    }
    if pic.bit_depth_chroma != pic.bit_depth {
        return Err(DecodeError::Unsupported(format!(
            "H.264 with {}-bit luma and {}-bit chroma",
            pic.bit_depth, pic.bit_depth_chroma
        )));
    }
    let c = pic.colour;
    let hint = ColourHint {
        transfer: clip_hint.transfer.or(transfer_for(c.transfer)),
        gamut: clip_hint.gamut.or(gamut_for(c.primaries)),
    };
    Ok(Frame {
        image: Decoded::Ycbcr {
            width: w,
            height: h,
            chroma_width: cw,
            chroma_height: ch,
            bits: pic.bit_depth,
            y: pic.y,
            cb: pic.cb,
            cr: pic.cr,
            matrix: matrix_for(c.matrix, h),
            full_range: c.full_range,
        },
        hint,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::YcbcrMatrix;
    use luna_formats::{Gamut, Transfer};
    use luna_h264::Colour;

    fn pic(chroma_format: u8, colour: Colour) -> luna_h264::Picture {
        let (w, h) = (64u32, 32u32);
        let ch = if chroma_format == 1 { h / 2 } else { h };
        luna_h264::Picture {
            width: w,
            height: h,
            chroma_format,
            bit_depth: 10,
            bit_depth_chroma: 10,
            y: vec![64; (w * h) as usize],
            cb: vec![512; (w / 2 * ch) as usize],
            cr: vec![512; (w / 2 * ch) as usize],
            colour,
        }
    }

    #[test]
    fn maps_planes_range_and_vui_colour() {
        let colour = Colour {
            primaries: 1,
            transfer: 1,
            matrix: 1,
            full_range: true,
        };
        let f = to_frame(pic(2, colour), ColourHint::default()).unwrap();
        let Decoded::Ycbcr {
            chroma_width,
            chroma_height,
            bits,
            matrix,
            full_range,
            ..
        } = f.image
        else {
            panic!("not ycbcr")
        };
        assert_eq!((chroma_width, chroma_height, bits), (32, 32, 10));
        assert_eq!(matrix, YcbcrMatrix::Bt709);
        assert!(full_range);
        assert_eq!(f.hint.transfer, Some(Transfer::Rec709));
        assert_eq!(f.hint.gamut, Some(Gamut::Rec709));
    }

    #[test]
    fn clip_hint_wins_and_unspecified_says_nothing() {
        let clip = ColourHint {
            transfer: Some(Transfer::SLog3),
            gamut: Some(Gamut::SGamut3Cine),
        };
        let f = to_frame(pic(1, Colour::default()), clip).unwrap();
        assert_eq!(f.hint, clip);
        let f = to_frame(pic(1, Colour::default()), ColourHint::default()).unwrap();
        assert_eq!(f.hint, ColourHint::default());
        let Decoded::Ycbcr { chroma_height, .. } = f.image else {
            panic!()
        };
        assert_eq!(chroma_height, 16);
    }

    #[test]
    fn inconsistent_planes_are_errors() {
        let mut p = pic(2, Colour::default());
        p.cr.pop();
        assert!(to_frame(p, ColourHint::default()).is_err());
        let mut p = pic(2, Colour::default());
        p.bit_depth_chroma = 8;
        assert!(matches!(
            to_frame(p, ColourHint::default()),
            Err(DecodeError::Unsupported(_))
        ));
    }
}
