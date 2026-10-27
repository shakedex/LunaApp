//! Header metadata: the primer pack and the local sets it keys, indexed by
//! InstanceUID so strong references resolve.

use std::collections::HashMap;
use std::io::{Read, Seek};

use super::klv::{PRIMER, Ul, ber, is_fill, is_local_set, is_partition, read_any_klv, ul};
use super::partition::{Partition, metadata_start};
use super::reader::Reader;
use crate::types::FormatError;

/// Largest local set we read in full. Real ones are a few hundred bytes; ARRI
/// JSON documents are a few KiB.
const MAX_SET: u64 = 1 << 20;
/// How much of a large vendor blob we keep (the ARRI legacy header is 4 KiB in).
const BLOB_HEAD: u64 = 16 * 1024;
const MAX_SETS: usize = 100_000;

/// A metadata property: its static local tag and its UL.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Prop {
    pub tag: u16,
    pub ul: Ul,
}

pub(crate) const fn prop(tag: u16, hex: &str) -> Prop {
    Prop { tag, ul: ul(hex) }
}

#[derive(Debug, Clone)]
pub(crate) struct Item {
    pub tag: u16,
    pub ul: Option<Ul>,
    pub value: Vec<u8>,
}

#[derive(Debug, Clone)]
pub(crate) struct LocalSet {
    pub key: Ul,
    pub items: Vec<Item>,
}

impl LocalSet {
    /// The value of `p`, found by UL through the primer, or by its static tag.
    pub(crate) fn get(&self, p: Prop) -> Option<&[u8]> {
        self.items
            .iter()
            .find(|i| i.ul.is_some_and(|u| u.is(&p.ul)))
            .or_else(|| {
                (p.tag < 0x8000)
                    .then(|| self.items.iter().find(|i| i.tag == p.tag && i.ul.is_none()))
                    .flatten()
            })
            .map(|i| i.value.as_slice())
    }

    pub(crate) fn has(&self, p: Prop) -> bool {
        self.get(p).is_some()
    }

    pub(crate) fn u8(&self, p: Prop) -> Option<u8> {
        self.get(p).and_then(|v| v.first().copied())
    }

    pub(crate) fn u16(&self, p: Prop) -> Option<u16> {
        self.get(p)
            .and_then(|v| v.get(..2))
            .map(|b| u16::from_be_bytes(b.try_into().expect("2")))
    }

    pub(crate) fn u32(&self, p: Prop) -> Option<u32> {
        self.get(p)
            .and_then(|v| v.get(..4))
            .map(|b| u32::from_be_bytes(b.try_into().expect("4")))
    }

    pub(crate) fn i64(&self, p: Prop) -> Option<i64> {
        self.get(p)
            .and_then(|v| v.get(..8))
            .map(|b| i64::from_be_bytes(b.try_into().expect("8")))
    }

    pub(crate) fn ul(&self, p: Prop) -> Option<Ul> {
        self.get(p).and_then(Ul::from_slice)
    }

    /// A (numerator, denominator) pair of i32s.
    pub(crate) fn rational(&self, p: Prop) -> Option<(i32, i32)> {
        let v = self.get(p)?.get(..8)?;
        Some((
            i32::from_be_bytes(v[..4].try_into().expect("4")),
            i32::from_be_bytes(v[4..].try_into().expect("4")),
        ))
    }

    /// A UTF-16BE string, trimmed of NULs and spaces.
    pub(crate) fn utf16(&self, p: Prop) -> Option<String> {
        let v = self.get(p)?;
        let units: Vec<u16> = v
            .as_chunks::<2>()
            .0
            .iter()
            .map(|c| u16::from_be_bytes([c[0], c[1]]))
            .take_while(|&u| u != 0)
            .collect();
        let s = String::from_utf16_lossy(&units);
        let s = s.trim();
        (!s.is_empty()).then(|| s.to_string())
    }

    /// The UIDs of a strong/weak reference batch or array (`count, size, items`).
    pub(crate) fn refs(&self, p: Prop) -> Vec<[u8; 16]> {
        let Some(v) = self.get(p) else {
            return Vec::new();
        };
        if v.len() < 8 {
            return Vec::new();
        }
        let n = u32::from_be_bytes(v[..4].try_into().expect("4")) as usize;
        let size = u32::from_be_bytes(v[4..8].try_into().expect("4")) as usize;
        if size < 16 {
            return Vec::new();
        }
        v[8..]
            .chunks_exact(size)
            .take(n)
            .map(|c| c[..16].try_into().expect("16"))
            .collect()
    }

    /// A single 16-byte reference.
    pub(crate) fn uid(&self, p: Prop) -> Option<[u8; 16]> {
        self.get(p)?.get(..16).map(|b| b.try_into().expect("16"))
    }

    /// Set kind byte (`0d010101 0101 xx 00` → xx) for SMPTE structural sets.
    pub(crate) fn kind(&self) -> Option<u8> {
        self.key
            .starts(&[
                0x06, 0x0e, 0x2b, 0x34, 0x02, 0x53, 0x01, 0x01, 0x0d, 0x01, 0x01, 0x01, 0x01, 0x01,
            ])
            .then_some(self.key.0[14])
    }
}

pub(crate) const INSTANCE_UID: Prop = prop(0x3c0a, "060e2b34010101010101150200000000");

/// All local sets of one partition's header metadata.
#[derive(Debug, Default)]
pub(crate) struct Metadata {
    pub sets: Vec<LocalSet>,
    /// Offset of the damaged or cut-short item the metadata stopped at.
    pub truncated: Option<u64>,
    by_uid: HashMap<[u8; 16], usize>,
}

impl Metadata {
    pub(crate) fn by_uid(&self, uid: &[u8; 16]) -> Option<&LocalSet> {
        self.by_uid.get(uid).map(|&i| &self.sets[i])
    }

    pub(crate) fn of_kind(&self, kind: u8) -> impl Iterator<Item = &LocalSet> {
        self.sets.iter().filter(move |s| s.kind() == Some(kind))
    }

    pub(crate) fn with_key(&self, prefix: &[u8]) -> impl Iterator<Item = &LocalSet> {
        self.sets.iter().filter(move |s| s.key.starts(prefix))
    }

    fn push(&mut self, s: LocalSet) {
        if let Some(uid) = s.uid(INSTANCE_UID) {
            self.by_uid.entry(uid).or_insert(self.sets.len());
        }
        self.sets.push(s);
    }
}

/// Whether a 0x13-coded vendor blob is worth keeping (ARRI's legacy header).
fn keep_blob(key: &Ul) -> bool {
    key.0[8..] == [0x0f, 0x01, 0x04, 0x02, 0x01, 0x01, 0x01, 0x00]
}

/// Reads one partition's header metadata.
pub(crate) fn read_metadata<R: Read + Seek>(
    r: &mut Reader<R>,
    p: &Partition,
) -> Result<Metadata, FormatError> {
    let start = metadata_start(r, p);
    let end = start.saturating_add(p.header_byte_count).min(r.len());
    let mut primer: HashMap<u16, Ul> = HashMap::new();
    let mut md = Metadata::default();
    let mut off = start;
    while off < end && md.sets.len() < MAX_SETS {
        // A damaged or cut-short tail keeps what was read before it.
        let k = match read_any_klv(r, off) {
            Ok(k) if !k.key.is_smpte() && k.end() <= end => {
                off = k.end(); // an Avid (AAF-keyed) set
                continue;
            }
            Ok(k) if k.key.is_smpte() && k.end() <= r.len() => k,
            Ok(_) | Err(_) if !md.sets.is_empty() => {
                md.truncated = Some(off);
                break;
            }
            Ok(k) if !k.key.is_smpte() => {
                return Err(FormatError::Corrupt(format!(
                    "no SMPTE key at offset {off}"
                )));
            }
            Ok(_) | Err(_) if !md.sets.is_empty() => {
                md.truncated = Some(off);
                break;
            }
            Ok(_) => {
                return Err(FormatError::Corrupt(format!(
                    "header metadata item at offset {off} runs past the end of the file"
                )));
            }
            Err(e) => return Err(e),
        };
        if is_partition(&k.key) {
            break;
        }
        if k.key.is(&PRIMER) {
            let v = r.read_vec(k.value, k.len, MAX_SET)?;
            primer = parse_primer(&v);
        } else if is_local_set(&k.key) && !is_fill(&k.key) {
            let blob = k.key.0[5] == 0x13;
            if blob && !keep_blob(&k.key) {
                // Vendor LUTs and the like: skip unread.
            } else {
                let n = if blob { k.len.min(BLOB_HEAD) } else { k.len };
                if n <= MAX_SET {
                    let v = r.read_vec(k.value, n, MAX_SET)?;
                    md.push(parse_set(k.key, &v, &primer));
                }
            }
        }
        off = k.end();
    }
    if md.sets.is_empty() {
        return Err(FormatError::Corrupt("no header metadata".into()));
    }
    Ok(md)
}

fn parse_primer(v: &[u8]) -> HashMap<u16, Ul> {
    let mut m = HashMap::new();
    if v.len() < 8 {
        return m;
    }
    let n = u32::from_be_bytes(v[..4].try_into().expect("4")) as usize;
    let size = u32::from_be_bytes(v[4..8].try_into().expect("4")) as usize;
    if size < 18 {
        return m;
    }
    for e in v[8..].chunks_exact(size).take(n) {
        let tag = u16::from_be_bytes([e[0], e[1]]);
        if let Some(u) = Ul::from_slice(&e[2..]) {
            m.insert(tag, u);
        }
    }
    m
}

/// Parses a local set's items. Byte 5 of the key says how item lengths are
/// coded: 0x53 → 2-byte, 0x13 → BER. A truncated last item is dropped.
pub(crate) fn parse_set(key: Ul, v: &[u8], primer: &HashMap<u16, Ul>) -> LocalSet {
    let ber_len = key.0[5] == 0x13;
    let mut items = Vec::new();
    let mut i = 0usize;
    while i + 3 <= v.len() {
        let tag = u16::from_be_bytes([v[i], v[i + 1]]);
        let (len, used) = if ber_len {
            match ber(&v[i + 2..]) {
                Some((l, u)) => (l, u),
                None => break,
            }
        } else {
            if i + 4 > v.len() {
                break;
            }
            (u16::from_be_bytes([v[i + 2], v[i + 3]]) as u64, 2)
        };
        let s = i + 2 + used;
        let Some(e) = usize::try_from(len).ok().and_then(|l| s.checked_add(l)) else {
            break;
        };
        if e > v.len() {
            // Keep a truncated tail of a blob we only read the head of.
            if ber_len {
                items.push(Item {
                    tag,
                    ul: primer.get(&tag).copied(),
                    value: v[s..].to_vec(),
                });
            }
            break;
        }
        items.push(Item {
            tag,
            ul: primer.get(&tag).copied(),
            value: v[s..e].to_vec(),
        });
        i = e;
    }
    LocalSet { key, items }
}

/// An MXF timestamp (`year u16, month, day, hour, minute, second, ms/4`).
pub(crate) fn timestamp(v: &[u8]) -> Option<(u16, u8, u8, u8, u8, u8)> {
    let v = v.get(..8)?;
    let y = u16::from_be_bytes([v[0], v[1]]);
    let (mo, d, h, mi, s) = (v[2], v[3], v[4], v[5], v[6]);
    let ok = (1980..=2200).contains(&y)
        && (1..=12).contains(&mo)
        && (1..=31).contains(&d)
        && h < 24
        && mi < 60
        && s < 61;
    ok.then_some((y, mo, d, h, mi, s))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn set_items_resolve_through_primer_or_static_tag() {
        let key = ul("060e2b34025301010d01010101013000");
        let mut primer = HashMap::new();
        let company = ul("060e2b34010101020520070102010000");
        primer.insert(0x3c01, company);
        // 3c01 "AB" in UTF-16, then an unknown dynamic tag.
        let v = [
            0x3c, 0x01, 0x00, 0x04, 0x00, b'A', 0x00, b'B', 0x80, 0x01, 0x00, 0x01, 0x07,
        ];
        let s = parse_set(key, &v, &primer);
        assert_eq!(
            s.utf16(prop(0x3c01, "060e2b34010101020520070102010000")),
            Some("AB".into())
        );
        assert_eq!(s.kind(), Some(0x30));
        assert_eq!(s.items.len(), 2);
    }

    #[test]
    fn truncated_items_are_dropped() {
        let key = ul("060e2b34025301010d01010101013000");
        let v = [0x3c, 0x01, 0x00, 0x09, 0x00];
        assert!(parse_set(key, &v, &HashMap::new()).items.is_empty());
    }

    #[test]
    fn timestamps_validate() {
        assert_eq!(
            timestamp(&[0x07, 0xe9, 10, 20, 13, 54, 13, 0]),
            Some((2025, 10, 20, 13, 54, 13))
        );
        assert_eq!(timestamp(&[0, 0, 0, 0, 0, 0, 0, 0]), None);
    }
}
