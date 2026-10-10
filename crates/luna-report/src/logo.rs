//! The company logo, validated once so a broken file can't sink a report.

use crate::pdf::check_image;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogoFormat {
    Png,
    Jpeg,
    Svg,
}

impl LogoFormat {
    pub fn mime(self) -> &'static str {
        match self {
            Self::Png => "image/png",
            Self::Jpeg => "image/jpeg",
            Self::Svg => "image/svg+xml",
        }
    }

    pub fn extension(self) -> &'static str {
        match self {
            Self::Png => "png",
            Self::Jpeg => "jpg",
            Self::Svg => "svg",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LogoError {
    /// Not PNG, JPEG or SVG.
    UnknownFormat,
    /// Looks like a supported format but doesn't decode.
    Unreadable(String),
}

impl std::fmt::Display for LogoError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnknownFormat => f.write_str("the logo must be a PNG, JPEG or SVG file"),
            Self::Unreadable(why) => write!(f, "the logo file can't be read: {why}"),
        }
    }
}

impl std::error::Error for LogoError {}

/// A logo that is known to decode. Only [`Logo::from_bytes`] makes one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Logo {
    bytes: Vec<u8>,
    format: LogoFormat,
}

impl Logo {
    pub fn from_bytes(bytes: Vec<u8>) -> Result<Self, LogoError> {
        let format = sniff(&bytes).ok_or(LogoError::UnknownFormat)?;
        // Checked the way the PDF places it: an SVG that decodes on its own can
        // still link images from outside, which the report can't load.
        check_image(&format!("/logo.{}", format.extension()), &bytes)
            .map_err(LogoError::Unreadable)?;
        Ok(Self { bytes, format })
    }

    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub fn format(&self) -> LogoFormat {
        self.format
    }
}

fn sniff(bytes: &[u8]) -> Option<LogoFormat> {
    if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        return Some(LogoFormat::Png);
    }
    if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
        return Some(LogoFormat::Jpeg);
    }
    let head = String::from_utf8_lossy(&bytes[..bytes.len().min(4096)]);
    head.contains("<svg").then_some(LogoFormat::Svg)
}
