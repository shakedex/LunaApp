//! Atom (box) walking: over an in-memory buffer for everything inside `moov`,
//! and over the file itself for the top level, where `mdat` is only skipped.

use super::Source;
use crate::types::FormatError;

/// One atom inside a buffer: its type and the byte range of its payload.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Atom<'a> {
    pub kind: [u8; 4],
    pub body: &'a [u8],
}

/// Iterates the atoms packed in `buf`. Stops at the first header that doesn't
/// fit or whose size runs past the buffer, and records that in `damaged`.
pub(crate) struct Atoms<'a> {
    buf: &'a [u8],
    pos: usize,
    damaged: bool,
}

impl<'a> Atoms<'a> {
    pub(crate) fn new(buf: &'a [u8]) -> Self {
        Atoms {
            buf,
            pos: 0,
            damaged: false,
        }
    }

    /// True when iteration stopped on a malformed header rather than at the end.
    pub(crate) fn damaged(&self) -> bool {
        self.damaged
    }
}

impl<'a> Iterator for Atoms<'a> {
    type Item = Atom<'a>;

    fn next(&mut self) -> Option<Atom<'a>> {
        let rest = self.buf.get(self.pos..)?;
        if rest.len() < 8 {
            // A 4-byte zero terminator (QuickTime sample entries, udta) is legal.
            if !rest.is_empty() && rest.iter().any(|&b| b != 0) {
                self.damaged = true;
            }
            self.pos = self.buf.len();
            return None;
        }
        let size32 = u32::from_be_bytes([rest[0], rest[1], rest[2], rest[3]]);
        let kind = [rest[4], rest[5], rest[6], rest[7]];
        let (size, header) = match size32 {
            0 => {
                if kind == [0; 4] {
                    // Zero padding, not an atom.
                    self.pos = self.buf.len();
                    return None;
                }
                (rest.len() as u64, 8)
            }
            1 => {
                let Some(b) = rest.get(8..16) else {
                    self.damaged = true;
                    self.pos = self.buf.len();
                    return None;
                };
                let mut a = [0u8; 8];
                a.copy_from_slice(b);
                (u64::from_be_bytes(a), 16)
            }
            n => (n as u64, 8),
        };
        if size < header as u64 || size > rest.len() as u64 {
            self.damaged = true;
            self.pos = self.buf.len();
            return None;
        }
        let size = size as usize;
        self.pos += size;
        Some(Atom {
            kind,
            body: &rest[header..size],
        })
    }
}

/// The first child of `kind` in `buf`.
pub(crate) fn child<'a>(buf: &'a [u8], kind: &[u8; 4]) -> Option<&'a [u8]> {
    Atoms::new(buf).find(|a| &a.kind == kind).map(|a| a.body)
}

/// Follows a path of child types, e.g. `[b"mdia", b"minf", b"stbl"]`.
pub(crate) fn path<'a>(buf: &'a [u8], kinds: &[&[u8; 4]]) -> Option<&'a [u8]> {
    kinds.iter().try_fold(buf, |b, k| child(b, k))
}

/// What the top level of the file holds.
#[derive(Debug, Default)]
pub(crate) struct TopLevel {
    pub major_brand: Option<[u8; 4]>,
    pub compatible: Vec<[u8; 4]>,
    /// File offset and length of the `moov` payload (header excluded).
    pub moov: Option<(u64, u64)>,
    pub warnings: Vec<String>,
}

/// More top-level atoms than this is not a camera file.
const MAX_TOP_LEVEL_ATOMS: usize = 100_000;

/// Walks the file's top-level atoms by seeking from header to header. Never
/// reads an atom body except `ftyp`'s first bytes.
pub(crate) fn scan_top_level(src: &dyn Source) -> Result<TopLevel, FormatError> {
    let len = src.len();
    let mut top = TopLevel::default();
    let mut pos = 0u64;
    let mut count = 0usize;
    while pos + 8 <= len {
        count += 1;
        if count > MAX_TOP_LEVEL_ATOMS {
            return Err(FormatError::Corrupt("too many top-level atoms".into()));
        }
        let mut h = [0u8; 16];
        let avail = (len - pos).min(16) as usize;
        src.read_at(pos, &mut h[..avail])?;
        let size32 = u32::from_be_bytes([h[0], h[1], h[2], h[3]]);
        let kind = [h[4], h[5], h[6], h[7]];
        let (size, header) = match size32 {
            0 => (len - pos, 8u64),
            1 if avail >= 16 => {
                let mut a = [0u8; 8];
                a.copy_from_slice(&h[8..16]);
                (u64::from_be_bytes(a), 16)
            }
            1 => (0, 16),
            n => (n as u64, 8),
        };
        if size < header {
            if top.moov.is_some() {
                top.warnings
                    .push("The file has unreadable data after the movie header.".into());
                break;
            }
            return Err(FormatError::Corrupt(format!(
                "a top-level atom at byte {pos} has an impossible size"
            )));
        }
        let end = pos.saturating_add(size);
        if end > len {
            if &kind == b"moov" {
                return Err(FormatError::Corrupt(
                    "the movie header (moov) is cut off; the file is truncated".into(),
                ));
            }
            top.warnings.push(format!(
                "The file is shorter than its '{}' atom says; it may be truncated.",
                String::from_utf8_lossy(&kind)
            ));
            break;
        }
        match &kind {
            b"ftyp" => {
                let body_len = (size - header).min(256) as usize;
                let mut b = vec![0u8; body_len];
                src.read_at(pos + header, &mut b)?;
                if b.len() >= 4 {
                    top.major_brand = Some([b[0], b[1], b[2], b[3]]);
                }
                top.compatible = b.get(8..).unwrap_or(&[]).as_chunks::<4>().0.to_vec();
            }
            b"moov" if top.moov.is_none() => top.moov = Some((pos + header, size - header)),
            _ => {}
        }
        pos = end;
    }
    Ok(top)
}

#[cfg(test)]
mod tests {
    use super::*;

    pub(crate) fn atom(kind: &[u8; 4], body: &[u8]) -> Vec<u8> {
        let mut v = ((body.len() + 8) as u32).to_be_bytes().to_vec();
        v.extend_from_slice(kind);
        v.extend_from_slice(body);
        v
    }

    #[test]
    fn walks_32_and_64_bit_sizes() {
        let mut buf = atom(b"free", b"ab");
        buf.extend_from_slice(&1u32.to_be_bytes());
        buf.extend_from_slice(b"wide");
        buf.extend_from_slice(&19u64.to_be_bytes());
        buf.extend_from_slice(b"xyz");
        let kinds: Vec<_> = Atoms::new(&buf)
            .map(|a| (a.kind, a.body.to_vec()))
            .collect();
        assert_eq!(
            kinds,
            vec![(*b"free", b"ab".to_vec()), (*b"wide", b"xyz".to_vec())]
        );
    }

    #[test]
    fn size_zero_runs_to_the_end() {
        let mut buf = 0u32.to_be_bytes().to_vec();
        buf.extend_from_slice(b"mdat0123");
        let a: Vec<_> = Atoms::new(&buf).collect();
        assert_eq!(a.len(), 1);
        assert_eq!(a[0].body, b"0123");
    }

    #[test]
    fn oversized_atom_stops_and_flags_damage() {
        let mut buf = atom(b"free", b"");
        buf.extend_from_slice(&100u32.to_be_bytes());
        buf.extend_from_slice(b"trak");
        let mut it = Atoms::new(&buf);
        assert_eq!(it.next().map(|a| a.kind), Some(*b"free"));
        assert!(it.next().is_none());
        assert!(it.damaged());
    }

    #[test]
    fn zero_terminator_is_not_damage() {
        let mut buf = atom(b"free", b"");
        buf.extend_from_slice(&[0, 0, 0, 0]);
        let mut it = Atoms::new(&buf);
        assert!(it.next().is_some());
        assert!(it.next().is_none());
        assert!(!it.damaged());
    }

    #[test]
    fn top_level_skips_mdat_and_finds_moov() {
        let mut file = atom(b"ftyp", b"qt  \0\0\0\0qt  ");
        file.extend(atom(b"mdat", &[0xAA; 64]));
        file.extend(atom(b"moov", b"payload"));
        let top = scan_top_level(&file.as_slice()).unwrap();
        assert_eq!(top.major_brand, Some(*b"qt  "));
        assert_eq!(top.compatible, vec![*b"qt  "]);
        let (off, len) = top.moov.unwrap();
        assert_eq!(&file[off as usize..(off + len) as usize], b"payload");
        assert!(top.warnings.is_empty());
    }

    #[test]
    fn truncated_moov_is_an_error() {
        let mut file = atom(b"moov", &[0; 32]);
        file.truncate(20);
        assert!(matches!(
            scan_top_level(&file.as_slice()),
            Err(FormatError::Corrupt(_))
        ));
    }

    #[test]
    fn truncated_mdat_after_moov_is_a_warning() {
        let mut file = atom(b"moov", b"m");
        file.extend(atom(b"mdat", &[0; 32]));
        file.truncate(file.len() - 10);
        let top = scan_top_level(&file.as_slice()).unwrap();
        assert!(top.moov.is_some());
        assert_eq!(top.warnings.len(), 1);
    }
}
