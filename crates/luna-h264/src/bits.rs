//! RBSP extraction (emulation-prevention removal) and an MSB-first bit reader
//! with Exp-Golomb codes (ITU-T H.264 7.2, 7.4.1, 9.1).

use crate::Error;

/// Bytes of zero padding kept after an RBSP so the reader can always load
/// eight bytes at once.
const PAD: usize = 8;

/// The RBSP of a NAL unit payload (after the NAL header byte): every
/// `00 00 03` loses its `03`. Padded with zeros; `len` is the real length.
pub(crate) struct Rbsp {
    data: Vec<u8>,
    len: usize,
}

impl Rbsp {
    pub(crate) fn new(payload: &[u8]) -> Rbsp {
        let mut data = Vec::with_capacity(payload.len() + PAD);
        let mut zeros = 0usize;
        for &b in payload {
            if zeros >= 2 && b == 3 {
                zeros = 0;
                continue;
            }
            zeros = if b == 0 { zeros + 1 } else { 0 };
            data.push(b);
        }
        let len = data.len();
        data.resize(len + PAD, 0);
        Rbsp { data, len }
    }

    pub(crate) fn reader(&self) -> BitReader<'_> {
        // The last 1 bit of the RBSP is the rbsp_stop_one_bit.
        let mut end = self.len * 8;
        let mut i = self.len;
        while i > 0 && self.data[i - 1] == 0 {
            i -= 1;
        }
        if i > 0 {
            end = i * 8 - 1 - self.data[i - 1].trailing_zeros() as usize;
        }
        BitReader {
            data: &self.data,
            pos: 0,
            len: self.len * 8,
            stop: end,
        }
    }
}

/// Reads bits MSB first. Reads past the end yield zeros; callers check
/// [`BitReader::overrun`] at safe points so damage ends in an error.
#[derive(Clone)]
pub(crate) struct BitReader<'a> {
    data: &'a [u8],
    pos: usize,
    len: usize,
    /// Bit position of the rbsp_stop_one_bit (or `len` when there is none).
    stop: usize,
}

impl<'a> BitReader<'a> {
    /// The next 32 bits without consuming them.
    #[inline]
    pub(crate) fn peek32(&self) -> u32 {
        let byte = self.pos >> 3;
        match self.data.get(byte..byte + 8) {
            Some(b) => {
                let w = u64::from_be_bytes(b.try_into().unwrap_or([0; 8]));
                ((w << (self.pos & 7)) >> 32) as u32
            }
            None => 0,
        }
    }

    #[inline]
    pub(crate) fn skip(&mut self, n: u32) {
        self.pos += n as usize;
    }

    /// `n` bits (0..=32) as an unsigned value.
    #[inline]
    pub(crate) fn u(&mut self, n: u32) -> u32 {
        if n == 0 {
            return 0;
        }
        let v = self.peek32() >> (32 - n);
        self.pos += n as usize;
        v
    }

    #[inline]
    pub(crate) fn flag(&mut self) -> bool {
        self.u(1) != 0
    }

    /// ue(v).
    #[inline]
    pub(crate) fn ue(&mut self) -> Result<u32, Error> {
        let w = self.peek32();
        let lz = w.leading_zeros();
        if lz < 16 {
            let n = 2 * lz + 1;
            self.pos += n as usize;
            return Ok((w >> (32 - n)) - 1);
        }
        if lz > 31 {
            return Err(Error::corrupt("Exp-Golomb code too long"));
        }
        self.pos += lz as usize;
        let v = self.u(lz + 1);
        Ok(v - 1)
    }

    /// se(v).
    #[inline]
    pub(crate) fn se(&mut self) -> Result<i32, Error> {
        let k = self.ue()? as i64;
        Ok(if k & 1 == 1 {
            ((k + 1) / 2) as i32
        } else {
            (-(k / 2)) as i32
        })
    }

    /// ue(v) that must not exceed `max`.
    pub(crate) fn ue_max(&mut self, max: u32, what: &str) -> Result<u32, Error> {
        let v = self.ue()?;
        if v > max {
            return Err(Error::corrupt(format!("{what} out of range ({v})")));
        }
        Ok(v)
    }

    /// se(v) within `min..=max`.
    pub(crate) fn se_range(&mut self, min: i32, max: i32, what: &str) -> Result<i32, Error> {
        let v = self.se()?;
        if v < min || v > max {
            return Err(Error::corrupt(format!("{what} out of range ({v})")));
        }
        Ok(v)
    }

    pub(crate) fn pos(&self) -> usize {
        self.pos
    }

    pub(crate) fn set_pos(&mut self, pos: usize) {
        self.pos = pos;
    }

    /// The padded RBSP bytes and the current bit position.
    pub(crate) fn parts(&self) -> (&'a [u8], usize) {
        (self.data, self.pos)
    }

    /// Length of the RBSP in bits (without padding).
    pub(crate) fn len_bits(&self) -> usize {
        self.len
    }

    pub(crate) fn align(&mut self) {
        self.pos = (self.pos + 7) & !7;
    }

    /// True once more bits were consumed than the RBSP holds.
    pub(crate) fn overrun(&self) -> bool {
        self.pos > self.len
    }

    pub(crate) fn check(&self) -> Result<(), Error> {
        if self.overrun() {
            Err(Error::corrupt("the slice data ends early"))
        } else {
            Ok(())
        }
    }

    /// more_rbsp_data(): is there anything before the rbsp_stop_one_bit?
    pub(crate) fn more_rbsp_data(&self) -> bool {
        self.pos < self.stop
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn removes_emulation_prevention() {
        let r = Rbsp::new(&[0, 0, 3, 1, 0, 0, 3, 0, 0, 3]);
        assert_eq!(&r.data[..r.len], &[0, 0, 1, 0, 0, 0, 0]);
    }

    #[test]
    fn reads_bits_and_exp_golomb() {
        // 1 | 010 | 011 | 00100 | 00101 | 1 (stop) ...
        // ue: 0, 1, 2, 3, 4 ; se of the same codes: 0, 1, -1, 2, -2
        let bits = "1010011001000010110000000";
        let mut bytes = vec![0u8; bits.len().div_ceil(8)];
        for (i, c) in bits.chars().enumerate() {
            if c == '1' {
                bytes[i / 8] |= 0x80 >> (i % 8);
            }
        }
        let r = Rbsp::new(&bytes);
        let mut br = r.reader();
        assert_eq!(br.ue().unwrap(), 0);
        assert_eq!(br.ue().unwrap(), 1);
        assert_eq!(br.ue().unwrap(), 2);
        assert_eq!(br.ue().unwrap(), 3);
        assert!(br.more_rbsp_data());
        assert_eq!(br.ue().unwrap(), 4);
        assert!(!br.more_rbsp_data());
        let mut br = r.reader();
        let v: Vec<i32> = (0..5).map(|_| br.se().unwrap()).collect();
        assert_eq!(v, vec![0, 1, -1, 2, -2]);
    }

    #[test]
    fn long_codes_and_overrun() {
        // 20 zeros, a one, 20 bits of value
        let mut br_bytes = vec![0u8; 2];
        br_bytes.push(0b0000_1000);
        br_bytes.extend_from_slice(&[0, 0, 0x10]);
        let r = Rbsp::new(&br_bytes);
        let mut br = r.reader();
        assert_eq!(br.ue().unwrap(), (1 << 20) - 1);
        let r = Rbsp::new(&[0, 0, 0, 0, 0]);
        let mut br = r.reader();
        assert!(br.ue().is_err());
        let mut br = r.reader();
        br.skip(41);
        assert!(br.overrun());
        assert_eq!(br.u(8), 0);
    }
}
