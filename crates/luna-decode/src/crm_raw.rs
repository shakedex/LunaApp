//! Canon Cinema RAW Light through our own decoder (the private `crm` crate).
//!
//! The decoder has no reduced-resolution mode, so each frame is decoded in
//! full, cropped to the active picture area and binned to thumbnail scale.

use luna_formats::{ClipFiles, ColourHint, Gamut, Probe, Transfer};

use crate::mosaic::{self, Rect};
use crate::util::{self, failed};
use crate::{Cfa, DecodeError, Decoded, Frame};

pub(crate) fn decode(
    clip: &ClipFiles,
    probe: &Probe,
    positions: &[u64],
    min_width: u32,
) -> Vec<Result<Frame, DecodeError>> {
    let opened = util::path_str(&clip.main)
        .and_then(|p| util::guard("Cinema RAW Light", || crm::Clip::open(p).map_err(failed)));
    let c = match opened {
        Ok(c) => c,
        Err(e) => return util::all_err(positions.len(), e),
    };
    let (black, white) = levels(c.raw_levels(), c.codec().bits);
    let a = c.active();
    let active = Rect {
        x: a.x as usize,
        y: a.y as usize,
        width: a.width as usize,
        height: a.height as usize,
    };
    let f = mosaic::bin_factor(active.width, min_width);
    let hint = ColourHint {
        transfer: Some(Transfer::Linear),
        gamut: probe.meta.colour.gamut.or(Some(Gamut::CinemaGamut)),
    };
    util::per_frame("Cinema RAW Light", positions, c.frame_count() as u64, |i| {
        let raw = c.decode_frame(i as usize).map_err(failed)?;
        let m = &raw.mosaic;
        // Verified against Canon's SDK (crm research/exposure.md): codes are
        // linear only up to a knee, so they are expanded to scene-linear light
        // before binning; exposure and the camera → Cinema Gamut matrix come
        // from the frame's metadata, and the mosaic is already as-shot balanced.
        let exposure = c.exposure(i as usize);
        let colour = c.colour(i as usize);
        let (samples, black, white, scene_scale) = match &exposure {
            Some(e) => {
                let top = e.clip_linear.max(f32::MIN_POSITIVE);
                let lut: Vec<u16> = e
                    .lut()
                    .into_iter()
                    .map(|l| ((l / top).clamp(0.0, 1.0) * 65535.0).round() as u16)
                    .collect();
                let linear: Vec<u16> = m
                    .data
                    .iter()
                    .map(|&v| lut[(v as usize).min(lut.len() - 1)])
                    .collect();
                (linear, 0.0, 65535.0, Some(top / 65535.0))
            }
            None => (m.data.clone(), black, white, None),
        };
        let (data, w, h) = mosaic::crop_bin(&samples, m.width, m.height, active, f);
        if w == 0 {
            return Err(DecodeError::Failed(
                "Cinema RAW Light active picture area is empty".into(),
            ));
        }
        let wb = match &colour {
            Some(col) => Some(col.wb_gains),
            // Canon's gains on top of the in-camera balance (1, 1 = as shot).
            None => c
                .frame_metadata(i as usize)
                .ok()
                .and_then(|fm| fm.white_balance)
                .map(|b| [b.gains.0, 1.0, b.gains.1])
                .filter(|g| g.iter().all(|v| v.is_finite() && *v > 0.0)),
        };
        Ok(Frame {
            image: Decoded::Mosaic {
                width: w as u32,
                height: h as u32,
                cfa: Cfa::Rggb,
                data,
                black,
                white,
                wb,
                camera_to_xyz: colour.as_ref().map(|col| col.camera_to_xyz),
                scene_scale,
            },
            hint,
        })
    })
}

/// The decoder's measured levels (12-bit clips); for other depths the 12-bit
/// black (259 of 4095) scaled to the sample depth, an estimate.
fn levels(raw: Option<crm::RawLevels>, bits: u32) -> (f32, f32) {
    match raw {
        Some(l) => (l.black as f32, l.white as f32),
        None => {
            let max = ((1u32 << bits.clamp(8, 16)) - 1) as f32;
            (259.0 / 4095.0 * max, max)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn levels_scale_with_depth_when_unmeasured() {
        assert_eq!(
            levels(
                Some(crm::RawLevels {
                    black: 259,
                    white: 4095
                }),
                12
            ),
            (259.0, 4095.0)
        );
        let (b, w) = levels(None, 14);
        assert_eq!(w, 16383.0);
        assert!((b - 1036.0).abs() < 1.0);
    }
}
