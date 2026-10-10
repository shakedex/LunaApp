//! CABAC arithmetic decoding engine (9.3.1.2, 9.3.3.2) and the context
//! initialisation of I slices (Tables 9-12 … 9-33, the I-slice columns).

use crate::Error;
use crate::bits::BitReader;

/// Context indices used by I slices of frame pictures in 4:2:0 / 4:2:2.
pub(crate) const NUM_CTX: usize = 460;

/// (m, n) for ctxIdx in each listed range, I slices. Unlisted indices are
/// not used by intra frame slices.
const INIT: &[(usize, &[(i8, i8)])] = &[
    (
        0,
        &[
            (20, -15),
            (2, 54),
            (3, 74),
            (20, -15),
            (2, 54),
            (3, 74),
            (-28, 127),
            (-23, 104),
            (-6, 53),
            (-1, 54),
            (7, 51),
        ],
    ),
    (
        60,
        &[
            (0, 41),
            (0, 63),
            (0, 63),
            (0, 63),
            (-9, 83),
            (4, 86),
            (0, 97),
            (-7, 72),
            (13, 41),
            (3, 62),
        ],
    ),
    (
        70,
        &[
            (0, 11),
            (1, 55),
            (0, 69),
            (-17, 127),
            (-13, 102),
            (0, 82),
            (-7, 74),
            (-21, 107),
            (-27, 127),
            (-31, 127),
            (-24, 127),
            (-18, 95),
            (-27, 127),
            (-21, 114),
            (-30, 127),
            (-17, 123),
            (-12, 115),
            (-16, 122),
            (-11, 115),
            (-12, 63),
            (-2, 68),
            (-15, 84),
            (-13, 104),
            (-3, 70),
            (-8, 93),
            (-10, 90),
            (-30, 127),
            (-1, 74),
            (-6, 97),
            (-7, 91),
            (-20, 127),
            (-4, 56),
            (-5, 82),
            (-7, 76),
            (-22, 125),
        ],
    ),
    (
        105,
        &[
            (-7, 93),
            (-11, 87),
            (-3, 77),
            (-5, 71),
            (-4, 63),
            (-4, 68),
            (-12, 84),
            (-7, 62),
            (-7, 65),
            (8, 61),
            (5, 56),
            (-2, 66),
            (1, 64),
            (0, 61),
            (-2, 78),
            (1, 50),
            (7, 52),
            (10, 35),
            (0, 44),
            (11, 38),
            (1, 45),
            (0, 46),
            (5, 44),
            (31, 17),
            (1, 51),
            (7, 50),
            (28, 19),
            (16, 33),
            (14, 62),
            (-13, 108),
            (-15, 100),
            (-13, 101),
            (-13, 91),
            (-12, 94),
            (-10, 88),
            (-16, 84),
            (-10, 86),
            (-7, 83),
            (-13, 87),
            (-19, 94),
            (1, 70),
            (0, 72),
            (-5, 74),
            (18, 59),
            (-8, 102),
            (-15, 100),
            (0, 95),
            (-4, 75),
            (2, 72),
            (-11, 75),
            (-3, 71),
            (15, 46),
            (-13, 69),
            (0, 62),
            (0, 65),
            (21, 37),
            (-15, 72),
            (9, 57),
            (16, 54),
            (0, 62),
            (12, 72),
        ],
    ),
    (
        166,
        &[
            (24, 0),
            (15, 9),
            (8, 25),
            (13, 18),
            (15, 9),
            (13, 19),
            (10, 37),
            (12, 18),
            (6, 29),
            (20, 33),
            (15, 30),
            (4, 45),
            (1, 58),
            (0, 62),
            (7, 61),
            (12, 38),
            (11, 45),
            (15, 39),
            (11, 42),
            (13, 44),
            (16, 45),
            (12, 41),
            (10, 49),
            (30, 34),
            (18, 42),
            (10, 55),
            (17, 51),
            (17, 46),
            (0, 89),
            (26, -19),
            (22, -17),
            (26, -17),
            (30, -25),
            (28, -20),
            (33, -23),
            (37, -27),
            (33, -23),
            (40, -28),
            (38, -17),
            (33, -11),
            (40, -15),
            (41, -6),
            (38, 1),
            (41, 17),
            (30, -6),
            (27, 3),
            (26, 22),
            (37, -16),
            (35, -4),
            (38, -8),
            (38, -3),
            (37, 3),
            (38, 5),
            (42, 0),
            (35, 16),
            (39, 22),
            (14, 48),
            (27, 37),
            (21, 60),
            (12, 68),
            (2, 97),
        ],
    ),
    (
        227,
        &[
            (-3, 71),
            (-6, 42),
            (-5, 50),
            (-3, 54),
            (-2, 62),
            (0, 58),
            (1, 63),
            (-2, 72),
            (-1, 74),
            (-9, 91),
            (-5, 67),
            (-5, 27),
            (-3, 39),
            (-2, 44),
            (0, 46),
            (-16, 64),
            (-8, 68),
            (-10, 78),
            (-6, 77),
            (-10, 86),
            (-12, 92),
            (-15, 55),
            (-10, 60),
            (-6, 62),
            (-4, 65),
            (-12, 73),
            (-8, 76),
            (-7, 80),
            (-9, 88),
            (-17, 110),
            (-11, 97),
            (-20, 84),
            (-11, 79),
            (-6, 73),
            (-4, 74),
            (-13, 86),
            (-13, 96),
            (-11, 97),
            (-19, 117),
            (-8, 78),
            (-5, 33),
            (-4, 48),
            (-2, 53),
            (-3, 62),
            (-13, 71),
            (-10, 79),
            (-12, 86),
            (-13, 90),
            (-14, 97),
        ],
    ),
    (399, &[(31, 21), (31, 31), (25, 50)]),
    (
        402,
        &[
            (-17, 120),
            (-20, 112),
            (-18, 114),
            (-11, 85),
            (-15, 92),
            (-14, 89),
            (-26, 71),
            (-15, 81),
            (-14, 80),
            (0, 68),
            (-14, 70),
            (-24, 56),
            (-23, 68),
            (-24, 50),
            (-11, 74),
            (23, -13),
            (26, -13),
            (40, -15),
            (49, -14),
            (44, 3),
            (45, 6),
            (44, 34),
            (33, 54),
            (19, 82),
            (-3, 75),
            (-1, 23),
            (1, 34),
            (1, 43),
            (0, 54),
            (-2, 55),
            (0, 61),
            (1, 64),
            (0, 68),
            (-9, 92),
        ],
    ),
];

/// rangeTabLPS[pStateIdx][qCodIRangeIdx] (Table 9-44).
const RANGE_LPS: [[u8; 4]; 64] = [
    [128, 176, 208, 240],
    [128, 167, 197, 227],
    [128, 158, 187, 216],
    [123, 150, 178, 205],
    [116, 142, 169, 195],
    [111, 135, 160, 185],
    [105, 128, 152, 175],
    [100, 122, 144, 166],
    [95, 116, 137, 158],
    [90, 110, 130, 150],
    [85, 104, 123, 142],
    [81, 99, 117, 135],
    [77, 94, 111, 128],
    [73, 89, 105, 122],
    [69, 85, 100, 116],
    [66, 80, 95, 110],
    [62, 76, 90, 104],
    [59, 72, 86, 99],
    [56, 69, 81, 94],
    [53, 65, 77, 89],
    [51, 62, 73, 85],
    [48, 59, 69, 80],
    [46, 56, 66, 76],
    [43, 53, 63, 72],
    [41, 50, 59, 69],
    [39, 48, 56, 65],
    [37, 45, 54, 62],
    [35, 43, 51, 59],
    [33, 41, 48, 56],
    [32, 39, 46, 53],
    [30, 37, 43, 50],
    [29, 35, 41, 48],
    [27, 33, 39, 45],
    [26, 31, 37, 43],
    [24, 30, 35, 41],
    [23, 28, 33, 39],
    [22, 27, 32, 37],
    [21, 26, 30, 35],
    [20, 24, 29, 33],
    [19, 23, 27, 31],
    [18, 22, 26, 30],
    [17, 21, 25, 28],
    [16, 20, 23, 27],
    [15, 19, 22, 25],
    [14, 18, 21, 24],
    [14, 17, 20, 23],
    [13, 16, 19, 22],
    [12, 15, 18, 21],
    [12, 14, 17, 20],
    [11, 14, 16, 19],
    [11, 13, 15, 18],
    [10, 12, 15, 17],
    [10, 12, 14, 16],
    [9, 11, 13, 15],
    [9, 11, 12, 14],
    [8, 10, 12, 14],
    [8, 9, 11, 13],
    [7, 9, 11, 12],
    [7, 9, 10, 12],
    [7, 8, 10, 11],
    [6, 8, 9, 11],
    [6, 7, 9, 10],
    [6, 7, 8, 9],
    [2, 2, 2, 2],
];

/// transIdxLPS (Table 9-45); transIdxMPS is min(s + 1, 62).
const TRANS_LPS: [u8; 64] = [
    0, 0, 1, 2, 2, 4, 4, 5, 6, 7, 8, 9, 9, 11, 11, 12, 13, 13, 15, 15, 16, 16, 18, 18, 19, 19, 21,
    21, 22, 22, 23, 24, 24, 25, 26, 26, 27, 27, 28, 29, 29, 30, 30, 30, 31, 32, 32, 33, 33, 33, 34,
    34, 35, 35, 35, 36, 36, 36, 37, 37, 37, 38, 38, 63,
];

/// Context states as (pStateIdx << 1) | valMPS.
pub(crate) fn init_contexts(slice_qp: i32) -> [u8; NUM_CTX] {
    let mut ctx = [0u8; NUM_CTX];
    let qp = slice_qp.clamp(0, 51);
    for &(start, vals) in INIT {
        for (k, &(m, n)) in vals.iter().enumerate() {
            let pre = (((m as i32 * qp) >> 4) + n as i32).clamp(1, 126);
            ctx[start + k] = if pre <= 63 {
                ((63 - pre) as u8) << 1
            } else {
                (((pre - 64) as u8) << 1) | 1
            };
        }
    }
    ctx
}

/// Packed-state transitions: next (pStateIdx << 1 | valMPS) after an MPS
/// and after an LPS (with the MPS swap at state 0).
const fn transitions() -> ([u8; 128], [u8; 128]) {
    let mut mps = [0u8; 128];
    let mut lps = [0u8; 128];
    let mut s = 0;
    while s < 128 {
        let state = s >> 1;
        let m = (s & 1) as u8;
        let up = if state < 62 { state + 1 } else { state };
        mps[s] = ((up as u8) << 1) | m;
        let flip = if state == 0 { 1 - m } else { m };
        lps[s] = (TRANS_LPS[state] << 1) | flip;
        s += 1;
    }
    (mps, lps)
}

const TRANS: ([u8; 128], [u8; 128]) = transitions();

/// TRANS.0 then TRANS.1, indexed by state | (is_lps << 7).
const TRANS_ALL: [u8; 256] = {
    let mut t = [0u8; 256];
    let mut i = 0;
    while i < 128 {
        t[i] = TRANS.0[i];
        t[128 + i] = TRANS.1[i];
        i += 1;
    }
    t
};

/// The arithmetic decoder. `value` holds codIOffset followed by `nb`
/// look-ahead bits, so renormalisation only moves the split point and
/// bytes are loaded in batches.
pub(crate) struct Engine<'a> {
    /// Kept in step only when bits are handed out (I_PCM).
    r: BitReader<'a>,
    data: &'a [u8],
    /// Next byte to load.
    byte: usize,
    value: u64,
    nb: u32,
    len_bits: usize,
    range: u32,
}

impl<'a> Engine<'a> {
    pub(crate) fn new(r: BitReader<'a>) -> Result<Engine<'a>, Error> {
        let (data, _) = r.parts();
        let len_bits = r.len_bits();
        let mut e = Engine {
            r,
            data,
            byte: 0,
            value: 0,
            nb: 0,
            len_bits,
            range: 510,
        };
        e.restart()?;
        Ok(e)
    }

    #[inline]
    fn refill(&mut self) {
        while self.nb <= 46 {
            let b = self.data.get(self.byte).copied().unwrap_or(0);
            self.value = (self.value << 8) | b as u64;
            self.nb += 8;
            self.byte += 1;
        }
    }

    fn bit_pos(&self) -> usize {
        self.byte * 8 - self.nb as usize
    }

    #[inline(always)]
    pub(crate) fn decision(&mut self, s: &mut u8) -> u32 {
        // Branch-free on the MPS/LPS outcome, which is hard to predict.
        let st = *s as usize & 127;
        let lps = RANGE_LPS[st >> 1][((self.range >> 6) & 3) as usize] as u32;
        let r_mps = self.range - lps;
        let scaled = (r_mps as u64) << self.nb;
        let is_lps = self.value >= scaled;
        self.value -= if is_lps { scaled } else { 0 };
        let range = if is_lps { lps } else { r_mps };
        *s = TRANS_ALL[st | (is_lps as usize) << 7];
        let shift = range.leading_zeros() - 23;
        self.range = range << shift;
        self.nb -= shift;
        if self.nb < 8 {
            self.refill();
        }
        (st as u32 & 1) ^ is_lps as u32
    }

    #[inline(always)]
    pub(crate) fn bypass(&mut self) -> u32 {
        self.nb -= 1;
        let scaled = (self.range as u64) << self.nb;
        let bin = if self.value >= scaled {
            self.value -= scaled;
            1
        } else {
            0
        };
        if self.nb < 8 {
            self.refill();
        }
        bin
    }

    pub(crate) fn terminate(&mut self) -> u32 {
        self.range -= 2;
        let scaled = (self.range as u64) << self.nb;
        if self.value >= scaled {
            1
        } else {
            if self.range < 256 {
                self.range <<= 1;
                self.nb -= 1;
                if self.nb < 8 {
                    self.refill();
                }
            }
            0
        }
    }

    pub(crate) fn overrun(&self) -> bool {
        self.bit_pos() > self.len_bits
    }

    /// The bit reader at the engine's position, after a terminate bin of 1
    /// (I_PCM samples follow).
    pub(crate) fn reader_mut(&mut self) -> &mut BitReader<'a> {
        let pos = self.bit_pos();
        self.r.set_pos(pos);
        &mut self.r
    }

    /// (Re-)initialises the engine at the reader's position (9.3.1.2), which
    /// is byte-aligned (slice data start, or after I_PCM samples).
    pub(crate) fn restart(&mut self) -> Result<(), Error> {
        self.r.align();
        self.byte = self.r.pos() / 8;
        self.value = 0;
        self.nb = 0;
        self.refill();
        // The first 9 bits are codIOffset.
        self.nb -= 9;
        let offset = self.value >> self.nb;
        if offset >= 510 {
            return Err(Error::corrupt("invalid CABAC start"));
        }
        self.range = 510;
        Ok(())
    }
}
