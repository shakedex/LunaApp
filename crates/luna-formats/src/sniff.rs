//! Content-based file classification. The extension is only a tie-breaker.

use std::path::Path;

use crate::types::Sniffed;

/// How many leading bytes [`sniff`] wants.
pub const SNIFF_LEN: usize = 4096;

/// Classifies a file from its first bytes (up to [`SNIFF_LEN`]).
pub fn sniff(path: &Path, head: &[u8]) -> Sniffed {
    let _ = (path, head);
    Sniffed::Other
}
