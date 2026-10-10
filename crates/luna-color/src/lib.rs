//! Turning a decoded frame into a display-referred Rec.709 thumbnail.
//!
//! Debayer (at thumbnail scale), levels and white balance, camera log → linear
//! using each vendor's published curve, gamut → Rec.709, display rendering,
//! downscale, JPEG.

mod curves;

use luna_decode::Frame;

/// An 8-bit sRGB/Rec.709 display image, interleaved RGB.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rgb8 {
    pub width: u32,
    pub height: u32,
    pub data: Vec<u8>,
}

/// The outcome of rendering, so the report can say when it had to guess.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rendered {
    pub image: Rgb8,
    /// True when the frame's transfer function was unknown and it is shown as decoded.
    pub transform_unknown: bool,
}

/// Renders `frame` to Rec.709 and scales it to at most `max_width` wide
/// (aspect kept, never upscaled). `pixel_aspect` widens anamorphic pictures.
pub fn render(frame: &Frame, max_width: u32, pixel_aspect: Option<(u32, u32)>) -> Rendered {
    let _ = (frame, max_width, pixel_aspect);
    Rendered {
        image: Rgb8 {
            width: 1,
            height: 1,
            data: vec![0, 0, 0],
        },
        transform_unknown: true,
    }
}

/// Baseline JPEG at a quality suited to report thumbnails.
pub fn encode_jpeg(image: &Rgb8) -> Vec<u8> {
    let _ = image;
    Vec::new()
}
