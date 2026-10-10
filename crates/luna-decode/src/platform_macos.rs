//! H.264 / HEVC on macOS. AVFoundation (AVAssetImageGenerator) is the plan;
//! it isn't written yet because it can't be built or checked without a Mac.

use luna_formats::{ClipFiles, Probe};

use crate::util;
use crate::{DecodeError, Frame};

pub(crate) fn decode(
    _clip: &ClipFiles,
    probe: &Probe,
    positions: &[u64],
    _min_width: u32,
) -> Vec<Result<Frame, DecodeError>> {
    util::all_err(
        positions.len(),
        DecodeError::Unsupported(format!(
            "{} isn't supported on macOS yet",
            probe.codec.label()
        )),
    )
}
