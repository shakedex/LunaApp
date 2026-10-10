//! Partition packs (ST 377-1 §7) and finding every partition of a file.

use std::io::{Read, Seek};

use super::klv::{RIP, is_fill, is_partition, read_klv};
use super::reader::Reader;
use crate::types::FormatError;

/// How far into the file a run-in may push the header partition.
const MAX_RUN_IN: u64 = 65536;
/// More partitions than this is a broken or hostile file.
const MAX_PARTITIONS: usize = 1 << 20;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Kind {
    Header,
    Body,
    Footer,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Partition {
    pub kind: Kind,
    /// Key byte 14: 1 open incomplete, 2 closed incomplete, 3 open complete,
    /// 4 closed complete.
    pub status: u8,
    /// File offset of the partition pack's key.
    pub offset: u64,
    /// File offset just past the partition pack.
    pub pack_end: u64,
    pub footer: u64,
    pub previous: u64,
    pub header_byte_count: u64,
    pub index_byte_count: u64,
    pub index_sid: u32,
    pub body_offset: u64,
    pub body_sid: u32,
}

impl Partition {
    /// How much to trust this partition's header metadata (higher is better).
    pub(crate) fn metadata_rank(&self) -> u8 {
        match self.status {
            4 => 4,
            3 => 3,
            2 => 2,
            _ => 1,
        }
    }
}

/// Reads the partition pack at `off`.
pub(crate) fn read_partition<R: Read + Seek>(
    r: &mut Reader<R>,
    off: u64,
) -> Result<Partition, FormatError> {
    let k = read_klv(r, off)?;
    if !is_partition(&k.key) {
        return Err(FormatError::Corrupt(format!(
            "expected a partition pack at offset {off}"
        )));
    }
    let v = r.read_vec(k.value, k.len.min(88), 88)?;
    if v.len() < 64 {
        return Err(FormatError::Corrupt(format!(
            "short partition pack at offset {off}"
        )));
    }
    let u32_at = |i: usize| u32::from_be_bytes(v[i..i + 4].try_into().expect("4"));
    let u64_at = |i: usize| u64::from_be_bytes(v[i..i + 8].try_into().expect("8"));
    let kind = match k.key.0[13] {
        0x02 => Kind::Header,
        0x03 => Kind::Body,
        _ => Kind::Footer,
    };
    Ok(Partition {
        kind,
        status: k.key.0[14],
        offset: off,
        pack_end: k.end(),
        previous: u64_at(16),
        footer: u64_at(24),
        header_byte_count: u64_at(32),
        index_byte_count: u64_at(40),
        index_sid: u32_at(48),
        body_offset: u64_at(52),
        body_sid: u32_at(60),
    })
}

/// Skips any fill KLVs starting at `off`; returns the first non-fill offset.
pub(crate) fn skip_fill<R: Read + Seek>(r: &mut Reader<R>, mut off: u64) -> u64 {
    for _ in 0..64 {
        match read_klv(r, off) {
            Ok(k) if is_fill(&k.key) && k.end() <= r.len() => off = k.end(),
            _ => break,
        }
    }
    off
}

/// Where this partition's header metadata starts (after the pack and its fill).
pub(crate) fn metadata_start<R: Read + Seek>(r: &mut Reader<R>, p: &Partition) -> u64 {
    skip_fill(r, p.pack_end)
}

/// Where this partition's essence starts: after the pack, its header metadata
/// and its index segments.
pub(crate) fn essence_start<R: Read + Seek>(r: &mut Reader<R>, p: &Partition) -> u64 {
    let after = metadata_start(r, p)
        .saturating_add(p.header_byte_count)
        .saturating_add(p.index_byte_count);
    skip_fill(r, after)
}

/// Every partition the file declares, in file order, plus whether the list is
/// known to be complete (a RIP or an intact footer chain).
pub(crate) struct Partitions {
    pub list: Vec<Partition>,
    pub complete: bool,
}

pub(crate) fn find_partitions<R: Read + Seek>(
    r: &mut Reader<R>,
) -> Result<Partitions, FormatError> {
    let header = find_header(r)?;
    if let Some(list) = from_rip(r, &header) {
        return Ok(Partitions {
            list,
            complete: true,
        });
    }
    if let Some(list) = from_footer_chain(r, &header) {
        return Ok(Partitions {
            list,
            complete: true,
        });
    }
    Ok(Partitions {
        list: vec![header],
        complete: false,
    })
}

fn find_header<R: Read + Seek>(r: &mut Reader<R>) -> Result<Partition, FormatError> {
    if let Ok(p) = read_partition(r, 0) {
        return Ok(p);
    }
    // ST 377-1 allows up to 64 KiB of run-in before the header partition.
    let head = r.read_vec(0, r.len().min(MAX_RUN_IN), MAX_RUN_IN)?;
    let prefix = [
        0x06, 0x0e, 0x2b, 0x34, 0x02, 0x05, 0x01, 0x01, 0x0d, 0x01, 0x02,
    ];
    for i in 1..head.len().saturating_sub(16) {
        if head[i..].starts_with(&prefix)
            && head[i + 13] == 0x02
            && let Ok(p) = read_partition(r, i as u64)
        {
            return Ok(p);
        }
    }
    Err(FormatError::Corrupt("no MXF header partition".into()))
}

fn from_rip<R: Read + Seek>(r: &mut Reader<R>, header: &Partition) -> Option<Vec<Partition>> {
    let len = r.len();
    let tail = r.peek(len.checked_sub(4)?, 4).ok()?;
    let rip_len = u32::from_be_bytes(tail.try_into().ok()?) as u64;
    if rip_len < 20 || rip_len > len || rip_len > 12 * MAX_PARTITIONS as u64 + 32 {
        return None;
    }
    let start = len - rip_len;
    let k = read_klv(r, start).ok()?;
    if !k.key.is(&RIP) || k.end() != len {
        return None;
    }
    let body = r
        .read_vec(k.value, k.len, 12 * MAX_PARTITIONS as u64 + 32)
        .ok()?;
    let mut list = Vec::new();
    for e in body[..body.len().saturating_sub(4)].as_chunks::<12>().0 {
        let off = u64::from_be_bytes(e[4..12].try_into().ok()?) + header.offset;
        list.push(read_partition(r, off).ok()?);
    }
    list.sort_by_key(|p| p.offset);
    list.dedup_by_key(|p| p.offset);
    (!list.is_empty() && list[0].kind == super::partition::Kind::Header).then_some(list)
}

fn from_footer_chain<R: Read + Seek>(
    r: &mut Reader<R>,
    header: &Partition,
) -> Option<Vec<Partition>> {
    if header.footer == 0 || header.footer >= r.len() {
        return None;
    }
    let mut list = Vec::new();
    let mut off = header.footer + header.offset;
    loop {
        let p = read_partition(r, off).ok()?;
        let prev = p.previous + header.offset;
        let at = p.offset;
        list.push(p);
        if at == header.offset || list.len() > MAX_PARTITIONS {
            break;
        }
        if prev >= at {
            return None;
        }
        off = prev;
    }
    list.reverse();
    (list[0].offset == header.offset).then_some(list)
}
