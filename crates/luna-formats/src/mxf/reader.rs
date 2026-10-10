//! Positioned, bounded reads over a seekable byte source.
//!
//! Every read goes through here so no length field from the file can make us
//! allocate or read more than the caller's explicit cap.

use std::io::{Read, Seek, SeekFrom};

use crate::types::FormatError;

/// Bytes kept around a recent read, so walking small KLV headers in sequence
/// costs one syscall per window instead of one per header.
const WINDOW: usize = 64 * 1024;

pub(crate) struct Reader<R> {
    inner: R,
    len: u64,
    buf: Vec<u8>,
    buf_off: u64,
}

impl<R: Read + Seek> Reader<R> {
    pub(crate) fn new(mut inner: R) -> Result<Self, FormatError> {
        let len = inner.seek(SeekFrom::End(0))?;
        Ok(Reader {
            inner,
            len,
            buf: Vec::new(),
            buf_off: 0,
        })
    }

    pub(crate) fn len(&self) -> u64 {
        self.len
    }

    /// Up to `n` bytes at `off` (fewer at end of file). `n` must stay small
    /// (≤ the window) — use [`Reader::read_vec`] for larger, capped reads.
    pub(crate) fn peek(&mut self, off: u64, n: usize) -> Result<&[u8], FormatError> {
        debug_assert!(n <= WINDOW);
        if off >= self.len {
            return Ok(&[]);
        }
        let avail = (self.len - off).min(n as u64) as usize;
        let cached =
            off >= self.buf_off && off + avail as u64 <= self.buf_off + self.buf.len() as u64;
        if !cached {
            let want = (self.len - off).min(WINDOW as u64) as usize;
            self.buf.resize(want, 0);
            self.inner.seek(SeekFrom::Start(off))?;
            read_full(&mut self.inner, &mut self.buf)?;
            self.buf_off = off;
        }
        let start = (off - self.buf_off) as usize;
        Ok(&self.buf[start..start + avail])
    }

    /// Exactly `n` bytes at `off`, refusing anything above `cap` or past the end.
    pub(crate) fn read_vec(&mut self, off: u64, n: u64, cap: u64) -> Result<Vec<u8>, FormatError> {
        if n > cap {
            return Err(FormatError::Corrupt(format!(
                "a {n}-byte item exceeds the {cap}-byte limit"
            )));
        }
        if off.checked_add(n).is_none_or(|end| end > self.len) {
            return Err(FormatError::Corrupt(format!(
                "a {n}-byte item at offset {off} runs past the end of the file"
            )));
        }
        if n as usize <= WINDOW {
            return Ok(self.peek(off, n as usize)?.to_vec());
        }
        let mut v = vec![0u8; n as usize];
        self.inner.seek(SeekFrom::Start(off))?;
        read_full(&mut self.inner, &mut v)?;
        Ok(v)
    }
}

fn read_full<R: Read>(r: &mut R, buf: &mut [u8]) -> Result<(), FormatError> {
    let mut n = 0;
    while n < buf.len() {
        match r.read(&mut buf[n..]) {
            Ok(0) => {
                return Err(FormatError::Io("unexpected end of file".into()));
            }
            Ok(k) => n += k,
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => {}
            Err(e) => return Err(e.into()),
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn peek_serves_short_reads_at_the_end() {
        let mut r = Reader::new(Cursor::new((0u8..10).collect::<Vec<_>>())).unwrap();
        assert_eq!(r.peek(8, 5).unwrap(), &[8, 9]);
        assert_eq!(r.peek(20, 5).unwrap(), &[] as &[u8]);
        assert_eq!(r.peek(2, 3).unwrap(), &[2, 3, 4]);
    }

    #[test]
    fn read_vec_refuses_oversized_and_overlong() {
        let mut r = Reader::new(Cursor::new(vec![0u8; 100])).unwrap();
        assert!(r.read_vec(0, 50, 10).is_err());
        assert!(r.read_vec(90, 20, 1000).is_err());
        assert!(r.read_vec(u64::MAX, 2, 1000).is_err());
        assert_eq!(r.read_vec(90, 10, 1000).unwrap().len(), 10);
    }
}
