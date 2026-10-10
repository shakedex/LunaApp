//! Fixed tables of ITU-T H.264 (scans, quantisation, defaults, deblocking).

/// 4×4 zig-zag scan (frame macroblocks): scan index → raster index (y*4+x).
pub(crate) const ZIGZAG4: [u8; 16] = [0, 1, 4, 8, 5, 2, 3, 6, 9, 12, 13, 10, 7, 11, 14, 15];

/// 8×8 zig-zag scan (frame macroblocks): scan index → raster index (y*8+x).
pub(crate) const ZIGZAG8: [u8; 64] = [
    0, 1, 8, 16, 9, 2, 3, 10, 17, 24, 32, 25, 18, 11, 4, 5, 12, 19, 26, 33, 40, 48, 41, 34, 27, 20,
    13, 6, 7, 14, 21, 28, 35, 42, 49, 56, 57, 50, 43, 36, 29, 22, 15, 23, 30, 37, 44, 51, 58, 59,
    52, 45, 38, 31, 39, 46, 53, 60, 61, 54, 47, 55, 62, 63,
];

/// QPc as a function of qPI for qPI ≥ 30 (Table 8-15); below 30 QPc = qPI.
const QPC_HIGH: [u8; 22] = [
    29, 30, 31, 32, 32, 33, 34, 34, 35, 35, 36, 36, 37, 37, 37, 38, 38, 38, 39, 39, 39, 39,
];

/// QPc for a qPI in −QpBdOffsetC..=51.
pub(crate) fn chroma_qp(qpi: i32) -> i32 {
    if qpi < 30 {
        qpi
    } else {
        QPC_HIGH[(qpi.min(51) - 30) as usize] as i32
    }
}

/// Default scaling lists (Table 7-3, 7-4), in zig-zag order.
pub(crate) const DEFAULT_4X4_INTRA: [u8; 16] = [
    6, 13, 13, 20, 20, 20, 28, 28, 28, 28, 32, 32, 32, 37, 37, 42,
];
pub(crate) const DEFAULT_4X4_INTER: [u8; 16] = [
    10, 14, 14, 20, 20, 20, 24, 24, 24, 24, 27, 27, 27, 30, 30, 34,
];
pub(crate) const DEFAULT_8X8_INTRA: [u8; 64] = [
    6, 10, 10, 13, 11, 13, 16, 16, 16, 16, 18, 18, 18, 18, 18, 23, 23, 23, 23, 23, 23, 25, 25, 25,
    25, 25, 25, 25, 27, 27, 27, 27, 27, 27, 27, 27, 29, 29, 29, 29, 29, 29, 29, 31, 31, 31, 31, 31,
    31, 33, 33, 33, 33, 33, 36, 36, 36, 36, 38, 38, 38, 40, 40, 42,
];
pub(crate) const DEFAULT_8X8_INTER: [u8; 64] = [
    9, 13, 13, 15, 13, 15, 17, 17, 17, 17, 19, 19, 19, 19, 19, 21, 21, 21, 21, 21, 21, 22, 22, 22,
    22, 22, 22, 22, 24, 24, 24, 24, 24, 24, 24, 24, 25, 25, 25, 25, 25, 25, 25, 27, 27, 27, 27, 27,
    27, 28, 28, 28, 28, 28, 30, 30, 30, 30, 32, 32, 32, 33, 33, 35,
];

/// normAdjust4x4 (8-315): v[m][class].
const NORM4: [[i32; 3]; 6] = [
    [10, 16, 13],
    [11, 18, 14],
    [13, 20, 16],
    [14, 23, 18],
    [16, 25, 20],
    [18, 29, 23],
];

/// normAdjust8x8 (8-318): v[m][class].
const NORM8: [[i32; 6]; 6] = [
    [20, 18, 32, 19, 25, 24],
    [22, 19, 35, 21, 28, 26],
    [26, 23, 42, 24, 33, 31],
    [28, 25, 45, 26, 35, 33],
    [32, 28, 51, 30, 40, 38],
    [36, 32, 58, 34, 46, 43],
];

pub(crate) fn norm4(m: usize, i: usize, j: usize) -> i32 {
    let class = match (i & 1, j & 1) {
        (0, 0) => 0,
        (1, 1) => 1,
        _ => 2,
    };
    NORM4[m][class]
}

pub(crate) fn norm8(m: usize, i: usize, j: usize) -> i32 {
    let (i4, j4) = (i & 3, j & 3);
    let class = if i4 == 0 && j4 == 0 {
        0
    } else if i & 1 == 1 && j & 1 == 1 {
        1
    } else if i4 == 2 && j4 == 2 {
        2
    } else if (i4 == 0 && j & 1 == 1) || (i & 1 == 1 && j4 == 0) {
        3
    } else if (i4 == 0 && j4 == 2) || (i4 == 2 && j4 == 0) {
        4
    } else {
        5
    };
    NORM8[m][class]
}

/// coded_block_pattern for Intra_4x4 / Intra_8x8 from its me(v) codeNum
/// (Table 9-4, ChromaArrayType 1 or 2).
pub(crate) const CBP_INTRA: [u8; 48] = [
    47, 31, 15, 0, 23, 27, 29, 30, 7, 11, 13, 14, 39, 43, 45, 46, 16, 3, 5, 10, 12, 19, 21, 26, 28,
    35, 37, 42, 44, 1, 2, 4, 8, 17, 18, 20, 24, 6, 9, 22, 25, 32, 33, 34, 36, 40, 38, 41,
];

/// α′ and β′ of the deblocking filter by indexA / indexB (Table 8-16).
pub(crate) const ALPHA: [u8; 52] = [
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 4, 4, 5, 6, 7, 8, 9, 10, 12, 13, 15, 17, 20,
    22, 25, 28, 32, 36, 40, 45, 50, 56, 63, 71, 80, 90, 101, 113, 127, 144, 162, 182, 203, 226,
    255, 255,
];
pub(crate) const BETA: [u8; 52] = [
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 2, 2, 2, 3, 3, 3, 3, 4, 4, 4, 6, 6, 7, 7, 8, 8,
    9, 9, 10, 10, 11, 11, 12, 12, 13, 13, 14, 14, 15, 15, 16, 16, 17, 17, 18, 18,
];

/// t′C0 for bS = 1, 2, 3 by indexA (Table 8-17).
pub(crate) const TC0: [[u8; 3]; 52] = [
    [0, 0, 0],
    [0, 0, 0],
    [0, 0, 0],
    [0, 0, 0],
    [0, 0, 0],
    [0, 0, 0],
    [0, 0, 0],
    [0, 0, 0],
    [0, 0, 0],
    [0, 0, 0],
    [0, 0, 0],
    [0, 0, 0],
    [0, 0, 0],
    [0, 0, 0],
    [0, 0, 0],
    [0, 0, 0],
    [0, 0, 0],
    [0, 0, 1],
    [0, 0, 1],
    [0, 0, 1],
    [0, 0, 1],
    [0, 1, 1],
    [0, 1, 1],
    [1, 1, 1],
    [1, 1, 1],
    [1, 1, 1],
    [1, 1, 1],
    [1, 1, 2],
    [1, 1, 2],
    [1, 1, 2],
    [1, 1, 2],
    [1, 2, 3],
    [1, 2, 3],
    [2, 2, 3],
    [2, 2, 4],
    [2, 3, 4],
    [2, 3, 4],
    [3, 3, 5],
    [3, 4, 6],
    [3, 4, 6],
    [4, 5, 7],
    [4, 5, 8],
    [4, 6, 9],
    [5, 7, 10],
    [6, 8, 11],
    [6, 8, 13],
    [7, 10, 14],
    [8, 11, 16],
    [9, 12, 18],
    [10, 13, 20],
    [11, 15, 23],
    [13, 17, 25],
];

/// Position (x, y) in luma samples of each 4×4 block by luma4x4BlkIdx.
pub(crate) const BLK4_XY: [(u8, u8); 16] = [
    (0, 0),
    (4, 0),
    (0, 4),
    (4, 4),
    (8, 0),
    (12, 0),
    (8, 4),
    (12, 4),
    (0, 8),
    (4, 8),
    (0, 12),
    (4, 12),
    (8, 8),
    (12, 8),
    (8, 12),
    (12, 12),
];

/// luma4x4BlkIdx of the 4×4 block at column `x` and row `y` (in 4×4 units).
pub(crate) const BLK4_AT: [[u8; 4]; 4] =
    [[0, 1, 4, 5], [2, 3, 6, 7], [8, 9, 12, 13], [10, 11, 14, 15]];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scans_are_permutations() {
        let mut s4 = ZIGZAG4.to_vec();
        s4.sort();
        assert_eq!(s4, (0..16).collect::<Vec<u8>>());
        let mut s8 = ZIGZAG8.to_vec();
        s8.sort();
        assert_eq!(s8, (0..64).collect::<Vec<u8>>());
        let mut c = CBP_INTRA.to_vec();
        c.sort();
        assert_eq!(c, (0..48).collect::<Vec<u8>>());
    }

    #[test]
    fn block_index_maps_agree() {
        for (i, &(x, y)) in BLK4_XY.iter().enumerate() {
            assert_eq!(BLK4_AT[(y / 4) as usize][(x / 4) as usize] as usize, i);
        }
    }

    #[test]
    fn chroma_qp_table() {
        assert_eq!(chroma_qp(-12), -12);
        assert_eq!(chroma_qp(29), 29);
        assert_eq!(chroma_qp(30), 29);
        assert_eq!(chroma_qp(51), 39);
    }
}
