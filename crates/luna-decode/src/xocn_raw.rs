//! Sony X-OCN through our own decoder (the private `xocn` crate).
//!
//! Thumbnails use the decoder's reduced-resolution preview (one linear RGB
//! value per 8×8 or 4×4 photosites, decoded from the coarse wavelet levels
//! only), which is many times faster than the full mosaic. The preview is
//! camera RGB, not white balanced, so it travels as a [`Decoded::Mosaic`]
//! (each preview pixel laid out as an R G / G B cell) with the clip's black
//! level; the colour stage then white-balances and converts it like any raw
//! frame.

use luna_formats::{ClipFiles, ColourHint, Gamut, Probe, Transfer};

use crate::util::{self, failed};
use crate::{Cfa, DecodeError, Decoded, Frame, mosaic};

pub(crate) fn decode(
    clip: &ClipFiles,
    probe: &Probe,
    positions: &[u64],
    min_width: u32,
) -> Vec<Result<Frame, DecodeError>> {
    let opened = util::path_str(&clip.main)
        .and_then(|p| util::guard("X-OCN", || xocn::Clip::open(p).map_err(failed)));
    let x = match opened {
        Ok(x) => x,
        Err(e) => return util::all_err(positions.len(), e),
    };
    let active = x.picture().active;
    let scale = scale_for(active.width, min_width);
    let (black, white) = levels(x.raw_levels());
    let hint = ColourHint {
        transfer: Some(Transfer::Linear),
        // Sensor-native RGB; S-Gamut3 is the closest named gamut (Sony defines
        // S-Gamut3 from the sensor's own primaries).
        gamut: probe.meta.colour.gamut.or(Some(Gamut::SGamut3)),
    };
    util::per_frame("X-OCN", positions, x.frame_count() as u64, |i| {
        let pf = x.decode_preview(i as usize, scale).map_err(failed)?;
        let p = &pf.preview;
        if p.data.len() < p.width * p.height * 3 {
            return Err(DecodeError::Failed("X-OCN preview is truncated".into()));
        }
        // Crop the preview to the active picture area.
        let a = pf.active;
        if a.width == 0 || a.height == 0 {
            return Err(DecodeError::Failed(
                "X-OCN active picture area is empty".into(),
            ));
        }
        let mut rgb = Vec::with_capacity(a.width * a.height * 3);
        for y in a.y..a.y + a.height {
            let row = &p.data[(y * p.width + a.x) * 3..(y * p.width + a.x + a.width) * 3];
            rgb.extend_from_slice(row);
        }
        Ok(Frame {
            image: Decoded::Mosaic {
                width: (a.width * 2) as u32,
                height: (a.height * 2) as u32,
                cfa: Cfa::Rggb,
                data: mosaic::rgb_to_rggb(&rgb, a.width, a.height),
                black,
                white,
                wb: None,
                camera_to_xyz: None,
            },
            hint,
        })
    })
}

/// Eighth resolution when that is still at least `min_width` picture pixels
/// wide (6K and up for a 640 px thumbnail), else quarter.
fn scale_for(active_width: usize, min_width: u32) -> xocn::Scale {
    if active_width / 8 >= min_width as usize {
        xocn::Scale::Eighth
    } else {
        xocn::Scale::Quarter
    }
}

/// Black from the clip's acquisition metadata (or the curve's floor), white at
/// the top of the 16-bit linear range the decoder's curve maps code 4095 to.
fn levels(raw: Option<xocn::RawLevels>) -> (f32, f32) {
    let white = xocn::curve::linear(4095) as f32;
    let black = raw
        .map(|l| l.black as f32)
        .unwrap_or_else(|| xocn::curve::linear(0) as f32);
    (black, white)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn eighth_only_when_wide_enough() {
        assert_eq!(scale_for(8640, 640), xocn::Scale::Eighth);
        assert_eq!(scale_for(6048, 640), xocn::Scale::Eighth);
        assert_eq!(scale_for(4096, 640), xocn::Scale::Quarter);
        assert_eq!(scale_for(8640, 1200), xocn::Scale::Quarter);
    }

    #[test]
    fn levels_fall_back_to_the_curve() {
        let (black, white) = levels(None);
        assert_eq!(black, xocn::curve::linear(0) as f32);
        assert_eq!(white, 65535.0);
        let (black, _) = levels(Some(xocn::RawLevels {
            black: 512,
            gray: 4000,
            white: 30000,
        }));
        assert_eq!(black, 512.0);
    }
}
