//! MSB-first bit reader and the RDD 36 variable-length codes.
//!
//! Every entropy-coded value in ProRes uses one family of codes: a Rice code
//! for small values that switches to an exponential-Golomb code once the
//! unary prefix exceeds a limit. A codebook is the triple (Rice order,
//! exp-Golomb order, last Rice prefix).

/// One adaptive codebook: Rice order `k_rice` while the unary prefix is at
/// most `last_rice_q`, exp-Golomb order `k_exp` beyond it.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Codebook {
    pub k_rice: u8,
    pub k_exp: u8,
    pub last_rice_q: u8,
}

pub(crate) const fn cb(k_rice: u8, k_exp: u8, last_rice_q: u8) -> Codebook {
    Codebook {
        k_rice,
        k_exp,
        last_rice_q,
    }
}

/// The longest unary prefix accepted. Real streams stay far below this; the
/// limit keeps every codeword inside one 64-bit peek.
const MAX_PREFIX: u32 = 24;

pub(crate) struct BitReader<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> BitReader<'a> {
    pub fn new(data: &'a [u8]) -> Self {
        Self { data, pos: 0 }
    }

    /// The next 64 bits (zero past the end), left-aligned; at least the top
    /// 57 are meaningful.
    #[inline(always)]
    fn peek64(&self) -> u64 {
        let byte = self.pos >> 3;
        let word = if let Some(chunk) = self.data.get(byte..byte + 8) {
            u64::from_be_bytes(chunk.try_into().unwrap())
        } else {
            let mut buf = [0u8; 8];
            if byte < self.data.len() {
                let tail = &self.data[byte..];
                buf[..tail.len()].copy_from_slice(tail);
            }
            u64::from_be_bytes(buf)
        };
        word << (self.pos & 7)
    }

    /// Bits consumed so far.
    #[inline(always)]
    pub fn position(&self) -> usize {
        self.pos
    }

    /// True once more bits have been consumed than the buffer holds.
    #[inline(always)]
    pub fn overrun(&self) -> bool {
        self.pos > self.data.len() * 8
    }

    #[inline(always)]
    pub fn read_bit(&mut self) -> u32 {
        let bit = (self.peek64() >> 63) as u32;
        self.pos += 1;
        bit
    }

    /// Reads one codeword. Fails on an over-long prefix (only garbage or a
    /// read past the end produces one).
    #[inline(always)]
    pub fn read_code(&mut self, book: Codebook) -> Option<u32> {
        let word = self.peek64();
        let q = word.leading_zeros();
        if q > MAX_PREFIX {
            return None;
        }
        let rice_prefixes = book.last_rice_q as u32 + 1;
        if q < rice_prefixes {
            // Rice: q zeros, a one, then k_rice suffix bits.
            let k = book.k_rice as u32;
            let suffix = if k == 0 {
                0
            } else {
                ((word << (q + 1)) >> (64 - k)) as u32
            };
            self.pos += (q + 1 + k) as usize;
            Some((q << k) | suffix)
        } else {
            // Escape prefix of `rice_prefixes` zeros, then an exp-Golomb code
            // of order k_exp, offset past the Rice range.
            let zeros = q - rice_prefixes;
            let len = zeros + book.k_exp as u32 + 1;
            let value = ((word << q) >> (64 - len)) as u32;
            self.pos += (q + len) as usize;
            Some(value - (1 << book.k_exp) + (rice_prefixes << book.k_rice))
        }
    }
}

/// Bit index one past the last set bit of `data` (0 when all bits are zero).
/// Coefficient data ends where only zero padding remains.
pub(crate) fn end_of_ones(data: &[u8]) -> usize {
    match data.iter().rposition(|&b| b != 0) {
        Some(i) => i * 8 + 8 - data[i].trailing_zeros() as usize,
        None => 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Packs a string of '0'/'1' into bytes, MSB first.
    fn bits(s: &str) -> Vec<u8> {
        let mut out = vec![0u8; s.len().div_ceil(8) + 8];
        for (i, c) in s.chars().enumerate() {
            if c == '1' {
                out[i / 8] |= 0x80 >> (i % 8);
            }
        }
        out
    }

    #[test]
    fn exp_golomb_order_zero_via_rice_escape() {
        // (Rice 0, EG 1, last Rice prefix 0) is the order-0 exp-Golomb code.
        let book = cb(0, 1, 0);
        let data = bits(&"1 010 011 00100 00111".replace(" ", ""));
        let mut r = BitReader::new(&data);
        let got: Vec<u32> = (0..5).map(|_| r.read_code(book).unwrap()).collect();
        assert_eq!(got, [0, 1, 2, 3, 6]);
        assert_eq!(r.position(), 1 + 3 + 3 + 5 + 5);
    }

    #[test]
    fn rice_then_exp_golomb() {
        // Rice order 1 for prefixes 0..=1, then EG order 2 offset by 4.
        let book = cb(1, 2, 1);
        let data = bits(&"10 11 010 011 00100 0001111".replace(" ", ""));
        let mut r = BitReader::new(&data);
        let got: Vec<u32> = (0..6).map(|_| r.read_code(book).unwrap()).collect();
        // 00 + EG2 "100" = 0 -> 4 ; 00 + EG2 "01111" = 11 -> 15
        assert_eq!(got, [0, 1, 2, 3, 4, 15]);
    }

    #[test]
    fn over_long_prefix_is_rejected() {
        let mut r = BitReader::new(&[0u8; 8]);
        assert!(r.read_code(cb(0, 1, 0)).is_none());
    }

    #[test]
    fn end_of_ones_finds_padding() {
        assert_eq!(end_of_ones(&[0, 0]), 0);
        assert_eq!(end_of_ones(&[0x80, 0]), 1);
        assert_eq!(end_of_ones(&[0x00, 0x01, 0x00]), 16);
    }
}
