//! Constant tables from RDD 36: coefficient scan orders and the adaptive
//! codebooks for DC differences, AC runs and AC levels.

use crate::bits::{Codebook, cb};

/// Progressive-frame scan: scan index → raster position (row × 8 + column).
pub(crate) const PROGRESSIVE_SCAN: [u8; 64] = [
    0, 1, 8, 9, 2, 3, 10, 11, //
    16, 17, 24, 25, 18, 19, 26, 27, //
    4, 5, 12, 20, 13, 6, 7, 14, //
    21, 28, 29, 22, 15, 23, 30, 31, //
    32, 33, 40, 48, 41, 34, 35, 42, //
    49, 56, 57, 50, 43, 36, 37, 44, //
    51, 58, 59, 52, 45, 38, 39, 46, //
    53, 60, 61, 54, 47, 55, 62, 63, //
];

/// Interlaced-frame (field picture) scan: scan index → raster position.
pub(crate) const INTERLACED_SCAN: [u8; 64] = [
    0, 8, 1, 9, 16, 24, 17, 25, //
    2, 10, 3, 11, 18, 26, 19, 27, //
    32, 40, 33, 34, 41, 48, 56, 49, //
    42, 35, 43, 50, 57, 58, 51, 59, //
    4, 12, 5, 6, 13, 20, 28, 21, //
    14, 7, 15, 22, 29, 36, 44, 37, //
    30, 23, 31, 38, 45, 52, 60, 53, //
    46, 39, 47, 54, 61, 62, 55, 63, //
];

/// Codebook for the first DC coefficient of a slice component.
pub(crate) const FIRST_DC: Codebook = cb(5, 6, 0);

/// Codebook for each later DC difference, chosen by the previous difference
/// codeword (clamped to 6).
pub(crate) const DC: [Codebook; 7] = [
    cb(0, 1, 0),
    cb(1, 2, 0),
    cb(1, 2, 0),
    cb(2, 3, 1),
    cb(2, 3, 1),
    cb(3, 4, 0),
    cb(3, 4, 0),
];

/// Codebook for an AC run, chosen by the previous run (clamped to 15).
pub(crate) const RUN: [Codebook; 16] = [
    cb(0, 1, 2),
    cb(0, 1, 2),
    cb(0, 1, 1),
    cb(0, 1, 1),
    cb(0, 1, 0),
    cb(1, 2, 1),
    cb(1, 2, 1),
    cb(1, 2, 1),
    cb(1, 2, 1),
    cb(1, 2, 0),
    cb(1, 2, 0),
    cb(1, 2, 0),
    cb(1, 2, 0),
    cb(1, 2, 0),
    cb(1, 2, 0),
    cb(2, 3, 0),
];

/// Codebook for an AC level magnitude, chosen by the previous magnitude
/// (clamped to 9).
pub(crate) const LEVEL: [Codebook; 10] = [
    cb(0, 1, 0),
    cb(0, 2, 2),
    cb(0, 1, 1),
    cb(0, 1, 2),
    cb(0, 1, 0),
    cb(1, 2, 0),
    cb(1, 2, 0),
    cb(1, 2, 0),
    cb(1, 2, 0),
    cb(2, 3, 0),
];

#[cfg(test)]
mod tests {
    use super::*;

    fn is_permutation(scan: &[u8; 64]) -> bool {
        let mut seen = [false; 64];
        for &p in scan {
            seen[p as usize] = true;
        }
        seen.iter().all(|&s| s)
    }

    #[test]
    fn scans_are_permutations() {
        assert!(is_permutation(&PROGRESSIVE_SCAN));
        assert!(is_permutation(&INTERLACED_SCAN));
    }
}
