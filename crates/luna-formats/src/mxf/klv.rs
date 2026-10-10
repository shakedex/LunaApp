//! SMPTE ST 336 KLV: 16-byte universal labels and BER-coded lengths.

use std::fmt;
use std::io::{Read, Seek};

use super::reader::Reader;
use crate::types::FormatError;

/// A SMPTE universal label (or key).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Default)]
pub(crate) struct Ul(pub [u8; 16]);

impl Ul {
    pub(crate) fn from_slice(b: &[u8]) -> Option<Ul> {
        b.get(..16).map(|s| Ul(s.try_into().expect("16 bytes")))
    }

    /// Equal ignoring byte 7, the registry version, which writers bump freely.
    pub(crate) fn is(&self, other: &Ul) -> bool {
        self.0[..7] == other.0[..7] && self.0[8..] == other.0[8..]
    }

    /// Starts with `prefix`, ignoring byte 7.
    pub(crate) fn starts(&self, prefix: &[u8]) -> bool {
        prefix
            .iter()
            .enumerate()
            .all(|(i, b)| i == 7 || self.0.get(i) == Some(b))
    }

    pub(crate) fn is_smpte(&self) -> bool {
        self.0[..4] == [0x06, 0x0e, 0x2b, 0x34]
    }

    pub(crate) fn hex(&self) -> String {
        self.0.iter().map(|b| format!("{b:02x}")).collect()
    }
}

impl fmt::Debug for Ul {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.hex())
    }
}

/// Builds a [`Ul`] from a hex literal at compile time.
pub(crate) const fn ul(hex: &str) -> Ul {
    let b = hex.as_bytes();
    assert!(b.len() == 32);
    let mut out = [0u8; 16];
    let mut i = 0;
    while i < 16 {
        out[i] = nibble(b[2 * i]) << 4 | nibble(b[2 * i + 1]);
        i += 1;
    }
    Ul(out)
}

const fn nibble(c: u8) -> u8 {
    match c {
        b'0'..=b'9' => c - b'0',
        b'a'..=b'f' => c - b'a' + 10,
        b'A'..=b'F' => c - b'A' + 10,
        _ => panic!("not hex"),
    }
}

/// Decodes a BER length at the start of `b`: `(length, bytes used)`.
/// Indefinite (0x80) and lengths over 8 bytes are refused.
pub(crate) fn ber(b: &[u8]) -> Option<(u64, usize)> {
    let first = *b.first()?;
    if first < 0x80 {
        return Some((first as u64, 1));
    }
    let n = (first & 0x7f) as usize;
    if n == 0 || n > 8 || b.len() < 1 + n {
        return None;
    }
    let v = b[1..=n].iter().fold(0u64, |a, &x| a << 8 | x as u64);
    Some((v, 1 + n))
}

/// One KLV triplet's header.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Klv {
    pub key: Ul,
    /// File offset of the key.
    pub offset: u64,
    /// File offset of the value.
    pub value: u64,
    pub len: u64,
}

impl Klv {
    /// Offset just past the value (saturating, for hostile lengths).
    pub(crate) fn end(&self) -> u64 {
        self.value.saturating_add(self.len)
    }
}

/// Reads the key and length at `off`. The value may run past the end of the
/// file; callers decide whether a truncated value is fatal (see [`Klv::end`]).
pub(crate) fn read_klv<R: Read + Seek>(r: &mut Reader<R>, off: u64) -> Result<Klv, FormatError> {
    let head = r.peek(off, 25)?;
    if head.len() < 17 {
        return Err(FormatError::Corrupt(format!(
            "the file ends inside a KLV header at offset {off}"
        )));
    }
    let key = Ul::from_slice(head).expect("16 bytes");
    if !key.is_smpte() {
        return Err(FormatError::Corrupt(format!(
            "no SMPTE key at offset {off}"
        )));
    }
    let (len, used) = ber(&head[16..])
        .ok_or_else(|| FormatError::Corrupt(format!("bad BER length at offset {off}")))?;
    Ok(Klv {
        key,
        offset: off,
        value: off + 16 + used as u64,
        len,
    })
}

// ---- Keys ---------------------------------------------------------------

/// KLV fill item.
pub(crate) const FILL: Ul = ul("060e2b34010101010301021001000000");
/// Primer pack.
pub(crate) const PRIMER: Ul = ul("060e2b34020501010d01020101050100");
/// Random index pack.
pub(crate) const RIP: Ul = ul("060e2b34020501010d01020101110100");
/// Index table segment.
pub(crate) const INDEX_SEGMENT: Ul = ul("060e2b34025301010d01020101100100");

pub(crate) fn is_fill(k: &Ul) -> bool {
    // Older writers use registry version 01 and a different byte 7.
    k.0[..4] == FILL.0[..4] && k.0[4..6] == [0x01, 0x01] && k.0[8..] == FILL.0[8..]
}

/// Partition pack key: `…0d0102010102|03|04 xx`.
pub(crate) fn is_partition(k: &Ul) -> bool {
    k.starts(&[
        0x06, 0x0e, 0x2b, 0x34, 0x02, 0x05, 0x01, 0x01, 0x0d, 0x01, 0x02, 0x01, 0x01,
    ]) && matches!(k.0[13], 0x02..=0x04)
}

/// A header-metadata local set (2-byte tags; 2-byte or BER item lengths).
pub(crate) fn is_local_set(k: &Ul) -> bool {
    k.is_smpte() && k.0[4] == 0x02 && (k.0[5] == 0x53 || k.0[5] == 0x13)
}

/// A generic-container essence element (`06 0e 2b 34 01 02 01 xx …`).
pub(crate) fn is_essence_element(k: &Ul) -> bool {
    k.is_smpte() && k.0[4] == 0x01 && k.0[5] == 0x02 && k.0[6] == 0x01
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn ber_short_and_long_forms() {
        assert_eq!(ber(&[0x05]), Some((5, 1)));
        assert_eq!(ber(&[0x83, 0x00, 0x01, 0x00]), Some((256, 4)));
        assert_eq!(ber(&[0x80]), None);
        assert_eq!(ber(&[0x89, 0, 0, 0, 0, 0, 0, 0, 0, 1]), None);
        assert_eq!(ber(&[0x84, 0x00]), None);
        assert_eq!(ber(&[]), None);
    }

    #[test]
    fn ul_compare_ignores_version_byte() {
        let a = ul("060e2b34010101020301021001000000");
        assert!(is_fill(&a));
        assert!(a.is(&FILL));
        assert!(!a.is(&PRIMER));
    }

    #[test]
    fn read_klv_reports_header_and_refuses_non_keys() {
        let mut d = FILL.0.to_vec();
        d.extend([0x83, 0, 0, 3, 1, 2, 3]);
        let mut r = Reader::new(Cursor::new(d)).unwrap();
        let k = read_klv(&mut r, 0).unwrap();
        assert_eq!((k.value, k.len, k.end()), (20, 3, 23));
        let mut r = Reader::new(Cursor::new(vec![0u8; 40])).unwrap();
        assert!(read_klv(&mut r, 0).is_err());
    }
}
