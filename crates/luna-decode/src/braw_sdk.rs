//! Blackmagic RAW through Blackmagic's own SDK (the private `braw` crate),
//! whose libraries ship inside Luna.
//!
//! The SDK only outputs debayered RGB, camera-native and as shot: Blackmagic
//! Design Film gamma in Blackmagic Wide Gamut, in the clip's own colour-science
//! generation. It has no Rec.709 output option, so frames are
//! [`Decoded::Rgb16`] tagged Film Gen 5 / Wide Gamut Gen 5 (Luna's only
//! Blackmagic transfer; older generations are close but not identical).

use std::path::PathBuf;
use std::sync::OnceLock;

use luna_formats::{ClipFiles, ColourHint, Gamut, Probe, Transfer};

use crate::util::{self, failed};
use crate::{DecodeError, Decoded, Frame};

/// The SDK library, loaded once per process.
pub(crate) fn sdk() -> Result<&'static braw::Sdk, DecodeError> {
    static SDK: OnceLock<Result<braw::Sdk, String>> = OnceLock::new();
    SDK.get_or_init(load)
        .as_ref()
        .map_err(|m| DecodeError::Unsupported(m.clone()))
}

fn load() -> Result<braw::Sdk, String> {
    let dirs = sdk_dirs();
    let mut last = String::from("no folder to look in");
    for dir in &dirs {
        match util::guard("Blackmagic RAW", || braw::Sdk::load(dir).map_err(failed)) {
            Ok(sdk) => return Ok(sdk),
            Err(e) => last = e.to_string(),
        }
    }
    Err(format!(
        "The Blackmagic RAW libraries are missing from this installation ({last})"
    ))
}

/// Where the SDK libraries may be: [`util::lib_dir`] in a shipped app;
/// for development, the SDK install named by `BRAW_SDK_DIR` (at run time, or
/// as it was when Luna was built).
fn sdk_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    if let Some(lib) = util::lib_dir() {
        dirs.push(lib);
    }
    let platform = if cfg!(target_os = "macos") {
        "Mac"
    } else {
        "Win"
    };
    let roots = [
        std::env::var_os("BRAW_SDK_DIR").map(PathBuf::from),
        option_env!("BRAW_SDK_DIR").map(PathBuf::from),
    ];
    for root in roots.into_iter().flatten() {
        let d = root.join(platform).join("Libraries");
        if !dirs.contains(&d) {
            dirs.push(d);
        }
    }
    dirs
}

/// The hint every SDK frame carries: what the SDK outputs.
pub(crate) const HINT: ColourHint = ColourHint {
    transfer: Some(Transfer::BmdFilmGen5),
    gamut: Some(Gamut::BmdWideGamutGen5),
};

pub(crate) fn decode(
    clip: &ClipFiles,
    _probe: &Probe,
    positions: &[u64],
    min_width: u32,
) -> Vec<Result<Frame, DecodeError>> {
    let opened = sdk()
        .and_then(|sdk| util::guard("Blackmagic RAW", || sdk.open(&clip.main).map_err(failed)));
    let c = match opened {
        Ok(c) => c,
        Err(e) => return util::all_err(positions.len(), e),
    };
    let info = c.info();
    let scale = scale_for(info.width, min_width);
    util::per_frame("Blackmagic RAW", positions, info.frame_count, |i| {
        let f = c.decode(i, scale).map_err(failed)?;
        let braw::Pixels::Rgb16(data) = f.pixels else {
            return Err(DecodeError::Failed(
                "The Blackmagic RAW SDK returned 8-bit pixels".into(),
            ));
        };
        if data.len() != f.width as usize * f.height as usize * 3 {
            return Err(DecodeError::Failed(
                "The Blackmagic RAW SDK returned a short image".into(),
            ));
        }
        Ok(Frame {
            image: Decoded::Rgb16 {
                width: f.width,
                height: f.height,
                data,
                transfer: Transfer::BmdFilmGen5,
                gamut: Gamut::BmdWideGamutGen5,
            },
            hint: HINT,
        })
    })
}

/// The smallest SDK scale still at least `min_width` wide. (The SDK may clamp
/// to a larger size than asked on some cameras; that is fine.)
fn scale_for(width: u32, min_width: u32) -> braw::Scale {
    [
        (8, braw::Scale::Eighth),
        (4, braw::Scale::Quarter),
        (2, braw::Scale::Half),
    ]
    .into_iter()
    .find(|&(d, _)| width / d >= min_width && width > 0)
    .map(|(_, s)| s)
    .unwrap_or(braw::Scale::Full)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scale_choice() {
        assert_eq!(scale_for(6048, 640), braw::Scale::Eighth);
        assert_eq!(scale_for(4096, 640), braw::Scale::Quarter);
        assert_eq!(scale_for(1920, 640), braw::Scale::Half);
        assert_eq!(scale_for(1000, 640), braw::Scale::Full);
    }

    #[test]
    fn shipped_folder_is_tried_first() {
        let dirs = sdk_dirs();
        assert_eq!(Some(&dirs[0]), util::lib_dir().as_ref());
    }
}
