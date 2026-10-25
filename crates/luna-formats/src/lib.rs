//! Luna's own container and metadata parsers. Pure Rust, no platform APIs.
//!
//! Two entry points:
//! - [`sniff`] classifies one file from its first bytes (content, not extension).
//! - [`probe`] reads one assembled clip: its metadata, its codec, and where its
//!   coded frames sit, for the decoders that read frames from our index.

mod mxf;
mod qt;
mod sniff;
mod trace;
mod types;

pub use mxf::arriraw_image_offset;
pub use sniff::{SNIFF_LEN, sniff};
pub use types::*;

use std::path::Path;

/// Reads a clip's container and sidecars. Never panics on bad input: a corrupt
/// or truncated file is an `Err`, and a field the file doesn't carry is `None`.
pub fn probe(clip: &ClipFiles) -> Result<Probe, FormatError> {
    match clip.container {
        Container::QuickTime => qt::probe(clip),
        Container::Mxf => mxf::probe(clip),
        other => Err(FormatError::Unsupported(format!(
            "{other:?} is not read by luna-formats"
        ))),
    }
}

/// Convenience for tests and tools: sniff a file on disk.
pub fn sniff_path(path: &Path) -> std::io::Result<Sniffed> {
    use std::io::Read;
    let mut head = vec![0u8; SNIFF_LEN];
    let mut f = std::fs::File::open(path)?;
    let mut n = 0;
    while n < head.len() {
        match f.read(&mut head[n..])? {
            0 => break,
            k => n += k,
        }
    }
    head.truncate(n);
    Ok(sniff(path, &head))
}
