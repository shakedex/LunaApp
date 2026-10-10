//! A bounds-checked big-endian cursor. Every read returns `None` past the end,
//! so a short or lying atom can never index out of range.

#[derive(Clone)]
pub(crate) struct Reader<'a> {
    buf: &'a [u8],
    pos: usize,
}

impl<'a> Reader<'a> {
    pub(crate) fn new(buf: &'a [u8]) -> Self {
        Reader { buf, pos: 0 }
    }

    pub(crate) fn remaining(&self) -> usize {
        self.buf.len() - self.pos
    }

    pub(crate) fn rest(&self) -> &'a [u8] {
        &self.buf[self.pos..]
    }

    pub(crate) fn skip(&mut self, n: usize) -> Option<()> {
        self.take(n).map(|_| ())
    }

    pub(crate) fn take(&mut self, n: usize) -> Option<&'a [u8]> {
        let end = self.pos.checked_add(n)?;
        let s = self.buf.get(self.pos..end)?;
        self.pos = end;
        Some(s)
    }

    pub(crate) fn u8(&mut self) -> Option<u8> {
        self.take(1).map(|b| b[0])
    }

    pub(crate) fn u16(&mut self) -> Option<u16> {
        self.take(2).map(|b| u16::from_be_bytes([b[0], b[1]]))
    }

    pub(crate) fn u32(&mut self) -> Option<u32> {
        self.take(4)
            .map(|b| u32::from_be_bytes([b[0], b[1], b[2], b[3]]))
    }

    pub(crate) fn i32(&mut self) -> Option<i32> {
        self.u32().map(|v| v as i32)
    }

    pub(crate) fn u64(&mut self) -> Option<u64> {
        self.take(8).map(|b| {
            let mut a = [0u8; 8];
            a.copy_from_slice(b);
            u64::from_be_bytes(a)
        })
    }

    pub(crate) fn fourcc(&mut self) -> Option<[u8; 4]> {
        self.take(4).map(|b| [b[0], b[1], b[2], b[3]])
    }
}

/// `count` entries of `entry` bytes each must fit in what is left, so a table
/// length can never drive an allocation larger than the atom that holds it.
pub(crate) fn table_fits(r: &Reader<'_>, count: u32, entry: usize) -> bool {
    (count as usize)
        .checked_mul(entry)
        .is_some_and(|n| n <= r.remaining())
}

/// A text value as display-ready string: trailing NULs and whitespace trimmed,
/// invalid UTF-8 replaced. `None` when nothing is left.
pub(crate) fn clean_text(b: &[u8]) -> Option<String> {
    let end = b.iter().position(|&c| c == 0).unwrap_or(b.len());
    let s = String::from_utf8_lossy(&b[..end]);
    let t = s.trim();
    (!t.is_empty()).then(|| t.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_are_bounded() {
        let mut r = Reader::new(&[0, 1, 2]);
        assert_eq!(r.u16(), Some(1));
        assert_eq!(r.u16(), None);
        assert_eq!(r.u8(), Some(2));
        assert_eq!(r.u8(), None);
        assert!(r.take(usize::MAX).is_none());
    }

    #[test]
    fn table_fits_rejects_lying_counts() {
        let r = Reader::new(&[0; 16]);
        assert!(table_fits(&r, 4, 4));
        assert!(!table_fits(&r, 5, 4));
        assert!(!table_fits(&r, u32::MAX, usize::MAX));
    }

    #[test]
    fn clean_text_trims_nuls_and_space() {
        assert_eq!(clean_text(b"ARRI \0\0junk").as_deref(), Some("ARRI"));
        assert_eq!(clean_text(b"\0\0"), None);
    }
}
