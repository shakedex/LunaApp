//! CAVLC residual blocks (7.3.5.3.2, 9.2): coeff_token, levels, total_zeros
//! and run_before, from the code tables 9-5, 9-7, 9-8, 9-9 and 9-10.

use std::sync::OnceLock;

use crate::Error;
use crate::bits::BitReader;

/// A prefix-code lookup over `bits` bits: entry = (length << 8) | value,
/// length 0 marks an invalid code.
pub(crate) struct Vlc {
    bits: u32,
    table: Vec<u16>,
}

impl Vlc {
    /// Builds from (code as '0'/'1' text, value). Panics on overlapping codes
    /// (a table typo), which the unit tests exercise.
    fn build(codes: &[(&str, u8)]) -> Vlc {
        let bits = codes.iter().map(|c| c.0.len()).max().unwrap_or(1) as u32;
        let mut table = vec![0u16; 1 << bits];
        for &(code, value) in codes {
            let len = code.len() as u32;
            let v = u32::from_str_radix(code, 2).unwrap_or(0);
            let base = (v << (bits - len)) as usize;
            for e in &mut table[base..base + (1 << (bits - len))] {
                assert!(*e == 0, "overlapping VLC code {code}");
                *e = ((len as u16) << 8) | value as u16;
            }
        }
        Vlc { bits, table }
    }

    #[inline]
    pub(crate) fn read(&self, r: &mut BitReader) -> Result<u8, Error> {
        let e = self.table[(r.peek32() >> (32 - self.bits)) as usize];
        let len = (e >> 8) as u32;
        if len == 0 {
            return Err(Error::corrupt("invalid CAVLC code"));
        }
        r.skip(len);
        Ok(e as u8)
    }

    #[cfg(test)]
    fn kraft(codes: &[(&str, u8)]) -> f64 {
        codes.iter().map(|c| 0.5f64.powi(c.0.len() as i32)).sum()
    }
}

/// coeff_token codes by [TotalCoeff][TrailingOnes] for 0≤nC<2, 2≤nC<4,
/// 4≤nC<8, nC = −1 (4:2:0 chroma DC) and nC = −2 (4:2:2 chroma DC).
const CT_0: [[&str; 4]; 17] = [
    ["1", "", "", ""],
    ["000101", "01", "", ""],
    ["00000111", "000100", "001", ""],
    ["000000111", "00000110", "0000101", "00011"],
    ["0000000111", "000000110", "00000101", "000011"],
    ["00000000111", "0000000110", "000000101", "0000100"],
    ["0000000001111", "00000000110", "0000000101", "00000100"],
    ["0000000001011", "0000000001110", "00000000101", "000000100"],
    [
        "0000000001000",
        "0000000001010",
        "0000000001101",
        "0000000100",
    ],
    [
        "00000000001111",
        "00000000001110",
        "0000000001001",
        "00000000100",
    ],
    [
        "00000000001011",
        "00000000001010",
        "00000000001101",
        "0000000001100",
    ],
    [
        "000000000001111",
        "000000000001110",
        "00000000001001",
        "00000000001100",
    ],
    [
        "000000000001011",
        "000000000001010",
        "000000000001101",
        "00000000001000",
    ],
    [
        "0000000000001111",
        "000000000000001",
        "000000000001001",
        "000000000001100",
    ],
    [
        "0000000000001011",
        "0000000000001110",
        "0000000000001101",
        "000000000001000",
    ],
    [
        "0000000000000111",
        "0000000000001010",
        "0000000000001001",
        "0000000000001100",
    ],
    [
        "0000000000000100",
        "0000000000000110",
        "0000000000000101",
        "0000000000001000",
    ],
];
const CT_2: [[&str; 4]; 17] = [
    ["11", "", "", ""],
    ["001011", "10", "", ""],
    ["000111", "00111", "011", ""],
    ["0000111", "001010", "001001", "0101"],
    ["00000111", "000110", "000101", "0100"],
    ["00000100", "0000110", "0000101", "00110"],
    ["000000111", "00000110", "00000101", "001000"],
    ["00000001111", "000000110", "000000101", "000100"],
    ["00000001011", "00000001110", "00000001101", "0000100"],
    ["000000001111", "00000001010", "00000001001", "000000100"],
    [
        "000000001011",
        "000000001110",
        "000000001101",
        "00000001100",
    ],
    [
        "000000001000",
        "000000001010",
        "000000001001",
        "00000001000",
    ],
    [
        "0000000001111",
        "0000000001110",
        "0000000001101",
        "000000001100",
    ],
    [
        "0000000001011",
        "0000000001010",
        "0000000001001",
        "0000000001100",
    ],
    [
        "0000000000111",
        "00000000001011",
        "0000000000110",
        "0000000001000",
    ],
    [
        "00000000001001",
        "00000000001000",
        "00000000001010",
        "0000000000001",
    ],
    [
        "00000000000111",
        "00000000000110",
        "00000000000101",
        "00000000000100",
    ],
];
const CT_4: [[&str; 4]; 17] = [
    ["1111", "", "", ""],
    ["001111", "1110", "", ""],
    ["001011", "01111", "1101", ""],
    ["001000", "01100", "01110", "1100"],
    ["0001111", "01010", "01011", "1011"],
    ["0001011", "01000", "01001", "1010"],
    ["0001001", "001110", "001101", "1001"],
    ["0001000", "001010", "001001", "1000"],
    ["00001111", "0001110", "0001101", "01101"],
    ["00001011", "00001110", "0001010", "001100"],
    ["000001111", "00001010", "00001101", "0001100"],
    ["000001011", "000001110", "00001001", "00001100"],
    ["000001000", "000001010", "000001101", "00001000"],
    ["0000001101", "000000111", "000001001", "000001100"],
    ["0000001001", "0000001100", "0000001011", "0000001010"],
    ["0000000101", "0000001000", "0000000111", "0000000110"],
    ["0000000001", "0000000100", "0000000011", "0000000010"],
];
const CT_M1: [[&str; 4]; 5] = [
    ["01", "", "", ""],
    ["000111", "1", "", ""],
    ["000100", "000110", "001", ""],
    ["000011", "0000011", "0000010", "000101"],
    ["000010", "00000011", "00000010", "0000000"],
];
const CT_M2: [[&str; 4]; 9] = [
    ["1", "", "", ""],
    ["0001111", "01", "", ""],
    ["0001110", "0001101", "001", ""],
    ["000000111", "0001100", "0001011", "00001"],
    ["000000110", "000000101", "0001010", "000001"],
    ["0000000111", "0000000110", "000000100", "0001001"],
    ["00000000111", "00000000110", "0000000101", "0001000"],
    ["000000000111", "000000000110", "00000000101", "0000000100"],
    [
        "0000000000111",
        "000000000101",
        "000000000100",
        "00000000100",
    ],
];

/// total_zeros codes by [TotalCoeff − 1][total_zeros] for 4×4 blocks.
const TZ_4X4: [&[&str]; 15] = [
    &[
        "1",
        "011",
        "010",
        "0011",
        "0010",
        "00011",
        "00010",
        "000011",
        "000010",
        "0000011",
        "0000010",
        "00000011",
        "00000010",
        "000000011",
        "000000010",
        "000000001",
    ],
    &[
        "111", "110", "101", "100", "011", "0101", "0100", "0011", "0010", "00011", "00010",
        "000011", "000010", "000001", "000000",
    ],
    &[
        "0101", "111", "110", "101", "0100", "0011", "100", "011", "0010", "00011", "00010",
        "000001", "00001", "000000",
    ],
    &[
        "00011", "111", "0101", "0100", "110", "101", "100", "0011", "011", "0010", "00010",
        "00001", "00000",
    ],
    &[
        "0101", "0100", "0011", "111", "110", "101", "100", "011", "0010", "00001", "0001", "00000",
    ],
    &[
        "000001", "00001", "111", "110", "101", "100", "011", "010", "0001", "001", "000000",
    ],
    &[
        "000001", "00001", "101", "100", "011", "11", "010", "0001", "001", "000000",
    ],
    &[
        "000001", "0001", "00001", "011", "11", "10", "010", "001", "000000",
    ],
    &["000001", "000000", "0001", "11", "10", "001", "01", "00001"],
    &["00001", "00000", "001", "11", "10", "01", "0001"],
    &["0000", "0001", "001", "010", "1", "011"],
    &["0000", "0001", "01", "1", "001"],
    &["000", "001", "1", "01"],
    &["00", "01", "1"],
    &["0", "1"],
];
/// total_zeros for 2×2 (4:2:0) chroma DC by [TotalCoeff − 1].
const TZ_DC420: [&[&str]; 3] = [&["1", "01", "001", "000"], &["1", "01", "00"], &["1", "0"]];
/// total_zeros for 2×4 (4:2:2) chroma DC by [TotalCoeff − 1].
const TZ_DC422: [&[&str]; 7] = [
    &["1", "010", "011", "0010", "0011", "0001", "00001", "00000"],
    &["000", "01", "001", "100", "101", "110", "111"],
    &["000", "001", "01", "10", "110", "111"],
    &["110", "00", "01", "10", "111"],
    &["00", "01", "10", "11"],
    &["00", "01", "1"],
    &["0", "1"],
];
/// run_before by [min(zerosLeft, 7) − 1][run_before].
const RUN_BEFORE: [&[&str]; 7] = [
    &["1", "0"],
    &["1", "01", "00"],
    &["11", "10", "01", "00"],
    &["11", "10", "01", "001", "000"],
    &["11", "10", "011", "010", "001", "000"],
    &["11", "000", "001", "011", "010", "101", "100"],
    &[
        "111",
        "110",
        "101",
        "100",
        "011",
        "010",
        "001",
        "0001",
        "00001",
        "000001",
        "0000001",
        "00000001",
        "000000001",
        "0000000001",
        "00000000001",
    ],
];

fn coeff_token_codes(t: &[[&'static str; 4]]) -> Vec<(&'static str, u8)> {
    let mut v = Vec::new();
    for (tc, row) in t.iter().enumerate() {
        for (t1, &code) in row.iter().enumerate() {
            if !code.is_empty() {
                v.push((code, ((tc as u8) << 2) | t1 as u8));
            }
        }
    }
    v
}

/// The fixed-length coeff_token of 8 ≤ nC.
fn coeff_token_flc() -> Vec<(String, u8)> {
    let mut v = vec![("000011".to_string(), 0u8)];
    for tc in 1..=16u8 {
        for t1 in 0..=tc.min(3) {
            v.push((format!("{:04b}{:02b}", tc - 1, t1), (tc << 2) | t1));
        }
    }
    v
}

fn list_codes(list: &[&'static str]) -> Vec<(&'static str, u8)> {
    list.iter()
        .enumerate()
        .map(|(i, &c)| (c, i as u8))
        .collect()
}

pub(crate) struct Tables {
    coeff_token: [Vlc; 6],
    tz4: Vec<Vlc>,
    tz420: Vec<Vlc>,
    tz422: Vec<Vlc>,
    run: Vec<Vlc>,
}

pub(crate) fn tables() -> &'static Tables {
    static T: OnceLock<Tables> = OnceLock::new();
    T.get_or_init(|| {
        let flc = coeff_token_flc();
        let flc: Vec<(&str, u8)> = flc.iter().map(|(s, v)| (s.as_str(), *v)).collect();
        Tables {
            coeff_token: [
                Vlc::build(&coeff_token_codes(&CT_0)),
                Vlc::build(&coeff_token_codes(&CT_2)),
                Vlc::build(&coeff_token_codes(&CT_4)),
                Vlc::build(&flc),
                Vlc::build(&coeff_token_codes(&CT_M1)),
                Vlc::build(&coeff_token_codes(&CT_M2)),
            ],
            tz4: TZ_4X4.iter().map(|l| Vlc::build(&list_codes(l))).collect(),
            tz420: TZ_DC420
                .iter()
                .map(|l| Vlc::build(&list_codes(l)))
                .collect(),
            tz422: TZ_DC422
                .iter()
                .map(|l| Vlc::build(&list_codes(l)))
                .collect(),
            run: RUN_BEFORE
                .iter()
                .map(|l| Vlc::build(&list_codes(l)))
                .collect(),
        }
    })
}

/// Which code tables a block uses.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum BlockKind {
    /// Luma or chroma AC / 4×4 with the given nC.
    Normal(i32),
    ChromaDc420,
    ChromaDc422,
}

/// Parses one residual_block_cavlc. Writes `max_coeff` levels in scan order
/// into `out[0..max_coeff]` (positions not written stay zero) and returns
/// TotalCoeff.
pub(crate) fn residual_block(
    r: &mut BitReader,
    t: &Tables,
    kind: BlockKind,
    max_coeff: usize,
    out: &mut [i32],
) -> Result<u32, Error> {
    let vlc = match kind {
        BlockKind::Normal(nc) if nc < 2 => &t.coeff_token[0],
        BlockKind::Normal(nc) if nc < 4 => &t.coeff_token[1],
        BlockKind::Normal(nc) if nc < 8 => &t.coeff_token[2],
        BlockKind::Normal(_) => &t.coeff_token[3],
        BlockKind::ChromaDc420 => &t.coeff_token[4],
        BlockKind::ChromaDc422 => &t.coeff_token[5],
    };
    let tok = vlc.read(r)?;
    let total = (tok >> 2) as usize;
    let t1 = (tok & 3) as usize;
    if total == 0 {
        return Ok(0);
    }
    if total > max_coeff {
        return Err(Error::corrupt("too many coefficients in a block"));
    }
    let mut level = [0i32; 16];
    let mut suffix_len: u32 = if total > 10 && t1 < 3 { 1 } else { 0 };
    for (i, lv) in level.iter_mut().enumerate().take(total) {
        if i < t1 {
            *lv = if r.flag() { -1 } else { 1 };
            continue;
        }
        let w = r.peek32();
        let prefix = w.leading_zeros();
        if prefix > 28 {
            return Err(Error::corrupt("level_prefix too long"));
        }
        r.skip(prefix + 1);
        let suffix_size = if prefix == 14 && suffix_len == 0 {
            4
        } else if prefix >= 15 {
            prefix - 3
        } else {
            suffix_len
        };
        let suffix = if suffix_size > 0 { r.u(suffix_size) } else { 0 };
        let mut code = ((prefix.min(15) << suffix_len) + suffix) as i64;
        if prefix >= 15 && suffix_len == 0 {
            code += 15;
        }
        if prefix >= 16 {
            code += (1i64 << (prefix - 3)) - 4096;
        }
        if i == t1 && t1 < 3 {
            code += 2;
        }
        let v = if code % 2 == 0 {
            (code + 2) >> 1
        } else {
            (-code - 1) >> 1
        };
        if v.abs() > 1 << 24 {
            return Err(Error::corrupt("coefficient level out of range"));
        }
        *lv = v as i32;
        if suffix_len == 0 {
            suffix_len = 1;
        }
        if v.unsigned_abs() > (3u64 << (suffix_len - 1)) && suffix_len < 6 {
            suffix_len += 1;
        }
    }
    let mut zeros_left = if total < max_coeff {
        let tz = match kind {
            BlockKind::ChromaDc420 => &t.tz420[total - 1],
            BlockKind::ChromaDc422 => &t.tz422[total - 1],
            BlockKind::Normal(_) => &t.tz4[total - 1],
        };
        tz.read(r)? as usize
    } else {
        0
    };
    if total + zeros_left > max_coeff {
        return Err(Error::corrupt("total_zeros out of range"));
    }
    // Place levels from the highest frequency down.
    let mut pos = total + zeros_left; // one past the highest coefficient
    for (i, &lv) in level.iter().enumerate().take(total) {
        let run = if zeros_left > 0 && i + 1 < total {
            let run = t.run[zeros_left.min(7) - 1].read(r)? as usize;
            if run > zeros_left {
                return Err(Error::corrupt("run_before out of range"));
            }
            run
        } else {
            zeros_left
        };
        pos -= 1;
        out[pos] = lv;
        pos -= run;
        zeros_left -= run;
    }
    Ok(total as u32)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bits::Rbsp;

    fn all_tables() -> Vec<(String, Vec<(&'static str, u8)>, bool)> {
        // (name, codes, complete?) — coeff_token for nC 0..8 and run_before
        // are complete prefix codes; the others leave codes unused.
        let mut v = vec![
            ("ct0".to_string(), coeff_token_codes(&CT_0), false),
            ("ct2".to_string(), coeff_token_codes(&CT_2), false),
            ("ct4".to_string(), coeff_token_codes(&CT_4), false),
            ("ctm1".to_string(), coeff_token_codes(&CT_M1), false),
            ("ctm2".to_string(), coeff_token_codes(&CT_M2), false),
        ];
        for (i, l) in TZ_4X4.iter().enumerate() {
            v.push((format!("tz{}", i + 1), list_codes(l), false));
        }
        for (i, l) in TZ_DC420.iter().enumerate() {
            v.push((format!("tz420_{}", i + 1), list_codes(l), true));
        }
        for (i, l) in TZ_DC422.iter().enumerate() {
            v.push((format!("tz422_{}", i + 1), list_codes(l), true));
        }
        for (i, l) in RUN_BEFORE.iter().enumerate() {
            v.push((format!("run{}", i + 1), list_codes(l), i < 6));
        }
        v
    }

    #[test]
    fn code_tables_are_prefix_free() {
        for (name, codes, complete) in all_tables() {
            let k = Vlc::kraft(&codes);
            assert!(k <= 1.0 + 1e-12, "{name}: Kraft sum {k}");
            if complete {
                assert!((k - 1.0).abs() < 1e-12, "{name}: incomplete code {k}");
            }
            let _ = Vlc::build(&codes); // panics on overlap
        }
        let _ = tables();
    }

    fn bits(s: &str) -> Rbsp {
        let mut bytes = vec![0u8; s.len().div_ceil(8) + 1];
        for (i, c) in s.chars().enumerate() {
            if c == '1' {
                bytes[i / 8] |= 0x80 >> (i % 8);
            }
        }
        Rbsp::new(&bytes)
    }

    #[test]
    fn full_block_example() {
        // Scan-order levels 0,3,-1,0,0,-1,1,0,1 with nC = 0, coded by hand:
        // coeff_token (TC 5, T1 3), T1 signs (+,+,-), level -1 (prefix 1),
        // level +3 (suffixLength 1: prefix 2, suffix 0), total_zeros 4,
        // runs 1, 0, 2, 0.
        let s = ["0000100", "001", "01", "0010", "110", "10", "11", "01", "1"].concat();
        let rb = bits(&s);
        let mut r = rb.reader();
        let mut out = [0i32; 16];
        let n = residual_block(&mut r, tables(), BlockKind::Normal(0), 16, &mut out).unwrap();
        assert_eq!(n, 5);
        assert_eq!(&out[..9], &[0, 3, -1, 0, 0, -1, 1, 0, 1]);
    }
}
