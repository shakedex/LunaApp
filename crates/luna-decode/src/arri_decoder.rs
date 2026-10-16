//! ARRIRAW and ARRICORE through Luna's verified ARRI decoder (the private
//! `arri` crate, checked against ARRI's own `art-cmd`): a binned linear preview
//! (from the mosaic, or from ARRICORE's JPEG XS base image), ARRI's as-shot
//! colour model to AWG4 scene-linear, then LogC4-encoded so the colour stage
//! renders it exactly like ARRI's own LogC4 → Rec.709 path.

use luna_formats::{ClipFiles, ColourHint, Gamut, Probe, Transfer};

use crate::util::{self, failed};
use crate::{DecodeError, Decoded, Frame};

pub(crate) fn decode(
    clip: &ClipFiles,
    probe: &Probe,
    positions: &[u64],
    min_width: u32,
) -> Vec<Result<Frame, DecodeError>> {
    let what = probe.codec.label();
    let opened = util::guard(&what, || arri::Clip::open(&clip.main).map_err(failed));
    let a = match opened {
        Ok(a) => a,
        Err(e) => return util::all_err(positions.len(), e),
    };
    let width = a.picture().active.width;
    let scale = [arri::Scale::Eighth, arri::Scale::Quarter, arri::Scale::Half]
        .into_iter()
        .find(|s| width / s.step() >= min_width as usize)
        .unwrap_or(arri::Scale::Half);
    let hint = ColourHint {
        transfer: Some(Transfer::LogC4),
        gamut: Some(Gamut::ArriWideGamut4),
    };
    util::per_frame(&what, positions, a.frame_count() as u64, |i| {
        let i = i as usize;
        let preview = a.decode_preview(i, scale).map_err(failed)?;
        let colour = a.color(i).map_err(failed)?;
        let data = preview
            .to_awg4(&colour)
            .into_iter()
            .map(|x| {
                (arri::logc4::encode(x.max(0.0) as f64).clamp(0.0, 1.0) * 65535.0).round() as u16
            })
            .collect();
        Ok(Frame {
            image: Decoded::Rgb16 {
                width: preview.width as u32,
                height: preview.height as u32,
                data,
                transfer: Transfer::LogC4,
                gamut: Gamut::ArriWideGamut4,
            },
            hint,
        })
    })
}
