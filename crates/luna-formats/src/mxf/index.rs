//! Where each picture frame sits: index table segments (ST 377-1 §11) mapped
//! through body partitions, with a sequential KLV walk as the fallback.

use std::collections::{BTreeMap, HashMap};
use std::io::{Read, Seek};

use super::klv::{INDEX_SEGMENT, Klv, Ul, is_essence_element, read_klv};
use super::metadata::{Prop, parse_set, prop};
use super::partition::{Kind, Partition, essence_start, metadata_start};
use super::reader::Reader;
use crate::types::{ByteRange, FormatError};

const EDIT_UNIT_BYTE_COUNT: Prop = prop(0x3f05, "060e2b34010101040406020100000000");
const BODY_SID: Prop = prop(0x3f07, "060e2b34010101040103040400000000");
const INDEX_START: Prop = prop(0x3f0c, "060e2b340101010507020103010a0000");
const SLICE_COUNT: Prop = prop(0x3f08, "060e2b34010101040404040101000000");
const POS_TABLE_COUNT: Prop = prop(0x3f0e, "060e2b34010101050404040107000000");
const INDEX_ENTRIES: Prop = prop(0x3f0a, "060e2b34010101050404040205000000");

/// Largest index segment we read (11-byte entries: ~6 M frames).
const MAX_SEGMENT: u64 = 64 << 20;
/// Most frames we will index (a day at 120 fps).
const MAX_FRAMES: u64 = 24 * 3600 * 120;
/// KLVs to step over inside one edit unit before the picture element.
const MAX_ELEMENTS_PER_UNIT: usize = 64;

#[derive(Debug, Clone, Default)]
pub(crate) struct Segment {
    pub body_sid: u32,
    pub start: i64,
    pub edit_unit_byte_count: u32,
    /// (temporal offset, stream offset) per edit unit from `start`.
    pub entries: Vec<(i8, u64)>,
}

fn parse_segment(v: &[u8]) -> Segment {
    let s = parse_set(INDEX_SEGMENT, v, &HashMap::new());
    let nsl = s.u8(SLICE_COUNT).unwrap_or(0) as usize;
    let npe = s.u8(POS_TABLE_COUNT).unwrap_or(0) as usize;
    let mut entries = Vec::new();
    if let Some(a) = s.get(INDEX_ENTRIES).filter(|a| a.len() >= 8) {
        let n = u32::from_be_bytes(a[..4].try_into().expect("4")) as usize;
        let size = u32::from_be_bytes(a[4..8].try_into().expect("4")) as usize;
        if size >= 11 + 4 * nsl + 8 * npe {
            for e in a[8..].chunks_exact(size).take(n) {
                let so = u64::from_be_bytes(e[3..11].try_into().expect("8"));
                entries.push((e[0] as i8, so));
            }
        }
    }
    Segment {
        body_sid: s.u32(BODY_SID).unwrap_or(0),
        start: s.i64(INDEX_START).unwrap_or(0),
        edit_unit_byte_count: s.u32(EDIT_UNIT_BYTE_COUNT).unwrap_or(0),
        entries,
    }
}

/// All index table segments in the file's partitions.
pub(crate) fn read_segments<R: Read + Seek>(
    r: &mut Reader<R>,
    parts: &[Partition],
) -> Result<Vec<Segment>, FormatError> {
    let mut out = Vec::new();
    for p in parts.iter().filter(|p| p.index_byte_count > 0) {
        let start = metadata_start(r, p).saturating_add(p.header_byte_count);
        let end = start.saturating_add(p.index_byte_count).min(r.len());
        let mut off = start;
        while off < end {
            let Ok(k) = read_klv(r, off) else { break };
            if k.end() > r.len() {
                break;
            }
            if k.key.is(&INDEX_SEGMENT) {
                let v = r.read_vec(k.value, k.len, MAX_SEGMENT)?;
                out.push(parse_segment(&v));
            }
            off = k.end();
        }
    }
    Ok(out)
}

/// One stretch of an essence container's bytes inside a body partition.
#[derive(Debug, Clone, Copy)]
struct Stretch {
    body_offset: u64,
    file_start: u64,
    file_end: u64,
}

fn stretches<R: Read + Seek>(r: &mut Reader<R>, parts: &[Partition], sid: u32) -> Vec<Stretch> {
    let mut out = Vec::new();
    for (i, p) in parts.iter().enumerate() {
        if p.body_sid != sid || p.kind == Kind::Footer {
            continue;
        }
        let end = parts.get(i + 1).map_or(r.len(), |n| n.offset).min(r.len());
        let start = essence_start(r, p);
        if start < end {
            out.push(Stretch {
                body_offset: p.body_offset,
                file_start: start,
                file_end: end,
            });
        }
    }
    out.sort_by_key(|s| s.body_offset);
    out
}

fn to_file(st: &[Stretch], stream: u64) -> Option<u64> {
    let s = st.iter().rev().find(|s| s.body_offset <= stream)?;
    let off = s.file_start.checked_add(stream - s.body_offset)?;
    (off < s.file_end).then_some(off)
}

/// Picture-frame byte ranges plus what went wrong along the way.
#[derive(Debug, Default)]
pub(crate) struct Frames {
    pub ranges: Vec<ByteRange>,
    pub warnings: Vec<String>,
    /// The last frame(s) are cut off by the end of the file.
    pub truncated: bool,
    /// The picture is one clip-wrapped element, not one element per frame.
    pub clip_wrapped: bool,
    /// Key of the first picture element.
    pub first_key: Option<Ul>,
}

pub(crate) struct Target {
    /// Matches the picture essence element key.
    pub track_number: Option<u32>,
    pub body_sid: Option<u32>,
    /// Edit units the metadata says the picture has.
    pub duration: Option<u64>,
}

pub(crate) fn is_picture(k: &Ul, track_number: Option<u32>) -> bool {
    if !is_essence_element(k) {
        return false;
    }
    match track_number {
        Some(t) if t != 0 => k.0[12..16] == t.to_be_bytes(),
        _ => k.0[8..12] == [0x0d, 0x01, 0x03, 0x01] && matches!(k.0[12], 0x05 | 0x15),
    }
}

/// Locates every picture frame.
pub(crate) fn frames<R: Read + Seek>(
    r: &mut Reader<R>,
    parts: &[Partition],
    complete: bool,
    segments: &[Segment],
    t: &Target,
) -> Frames {
    let sid = t
        .body_sid
        .or_else(|| parts.iter().map(|p| p.body_sid).find(|&s| s != 0))
        .unwrap_or(1);
    let st = stretches(r, parts, sid);
    let segs: Vec<&Segment> = segments
        .iter()
        .filter(|s| s.body_sid == sid || s.body_sid == 0)
        .collect();

    if !st.is_empty() && !segs.is_empty() {
        let indexed = if let Some(cbe) = segs.iter().find(|s| s.edit_unit_byte_count > 0) {
            from_cbe(r, &st, cbe.edit_unit_byte_count as u64, t)
        } else {
            from_vbe(r, &st, &segs, t)
        };
        match indexed {
            Ok(f) => return f,
            Err(why) => {
                let mut f = scan(r, parts, complete, sid, t);
                f.warnings.insert(
                    0,
                    format!("Index table not usable ({why}); frames were located by scanning."),
                );
                return f;
            }
        }
    }
    scan(r, parts, complete, sid, t)
}

/// Finds the picture element inside the edit unit starting at `unit`.
fn picture_in_unit<R: Read + Seek>(
    r: &mut Reader<R>,
    unit: u64,
    limit: u64,
    tn: Option<u32>,
) -> Result<Klv, String> {
    let mut off = unit;
    for _ in 0..MAX_ELEMENTS_PER_UNIT {
        if off >= limit {
            break;
        }
        let k = read_klv(r, off).map_err(|e| e.to_string())?;
        if is_picture(&k.key, tn) {
            return Ok(k);
        }
        off = k.end();
    }
    Err(format!(
        "no picture element in the edit unit at offset {unit}"
    ))
}

fn from_cbe<R: Read + Seek>(
    r: &mut Reader<R>,
    st: &[Stretch],
    eubc: u64,
    t: &Target,
) -> Result<Frames, String> {
    let total: u64 = st.iter().map(|s| s.file_end - s.file_start).sum();
    let available = total / eubc;
    let n = t.duration.unwrap_or(available).min(MAX_FRAMES);
    if n == 0 {
        return Err("empty".into());
    }
    let first_unit = to_file(st, 0).ok_or("no essence")?;
    let k0 = picture_in_unit(r, first_unit, first_unit + eubc, t.track_number)?;
    let delta = k0.value - first_unit;
    let mut f = Frames {
        first_key: Some(k0.key),
        ..Default::default()
    };
    for i in 0..n {
        let Some(unit) = i.checked_mul(eubc).and_then(|so| to_file(st, so)) else {
            f.truncated = true;
            break;
        };
        let value = unit + delta;
        if value + k0.len > r.len() {
            f.truncated = true;
            break;
        }
        f.ranges.push(ByteRange {
            offset: value,
            len: k0.len,
        });
    }
    // Spot-check the last frame's element header against the first.
    if let Some(last) = f.ranges.last() {
        let k = read_klv(r, last.offset - (k0.value - k0.offset)).map_err(|e| e.to_string())?;
        if k.key != k0.key || k.len != k0.len {
            return Err("constant edit units do not line up".into());
        }
    }
    Ok(f)
}

fn from_vbe<R: Read + Seek>(
    r: &mut Reader<R>,
    st: &[Stretch],
    segs: &[&Segment],
    t: &Target,
) -> Result<Frames, String> {
    let mut units: BTreeMap<i64, (i8, u64)> = BTreeMap::new();
    for s in segs {
        for (i, e) in s.entries.iter().enumerate() {
            units.insert(s.start + i as i64, *e);
        }
    }
    let n = units.len() as u64;
    if n == 0 || n > MAX_FRAMES {
        return Err("no index entries".into());
    }
    if units.keys().next() != Some(&0) || *units.keys().last().expect("non-empty") != n as i64 - 1 {
        return Err("index entries have gaps".into());
    }
    let coded: Vec<(i8, u64)> = units.into_values().collect();
    let mut f = Frames::default();
    let mut ranges = Vec::with_capacity(coded.len());
    for (i, &(_, so)) in coded.iter().enumerate() {
        let Some(unit) = to_file(st, so) else {
            f.truncated = true;
            break;
        };
        let limit = coded
            .get(i + 1)
            .and_then(|&(_, next)| to_file(st, next))
            .unwrap_or(r.len());
        let k = match picture_in_unit(r, unit, limit, t.track_number) {
            Ok(k) => k,
            Err(_) if i + 1 == coded.len() || unit >= r.len() => {
                f.truncated = true;
                break;
            }
            Err(e) => return Err(e),
        };
        if k.end() > r.len() {
            f.truncated = true;
            break;
        }
        f.first_key.get_or_insert(k.key);
        ranges.push(ByteRange {
            offset: k.value,
            len: k.len,
        });
    }
    // Display order: display position d holds coded frame d + TemporalOffset[d].
    let reorder = coded.iter().any(|&(to, _)| to != 0) && ranges.len() == coded.len();
    if reorder {
        let mut display = Vec::with_capacity(ranges.len());
        for (d, &(to, _)) in coded.iter().enumerate() {
            let c = d as i64 + to as i64;
            match ranges.get(usize::try_from(c).unwrap_or(usize::MAX)) {
                Some(rg) => display.push(*rg),
                None => return Err("temporal offsets point outside the clip".into()),
            }
        }
        ranges = display;
    }
    f.ranges = ranges;
    Ok(f)
}

/// Walks every KLV from the first essence byte, collecting picture elements.
fn scan<R: Read + Seek>(
    r: &mut Reader<R>,
    parts: &[Partition],
    complete: bool,
    sid: u32,
    t: &Target,
) -> Frames {
    let mut f = Frames::default();
    let start = parts
        .iter()
        .find(|p| p.body_sid == sid)
        .or_else(|| parts.iter().find(|p| p.body_sid != 0))
        .map(|p| essence_start(r, p));
    let Some(mut off) = start else {
        f.warnings.push("No essence partition was found.".into());
        return f;
    };
    let footer = parts
        .iter()
        .find(|p| p.kind == Kind::Footer)
        .map_or(u64::MAX, |p| p.offset);
    let len = r.len();
    while off < len && off < footer {
        let k = match read_klv(r, off) {
            Ok(k) => k,
            Err(_) => {
                if off + 17 > len {
                    f.truncated = true;
                } else {
                    f.warnings.push(format!(
                        "The essence is damaged at offset {off}; frames after it are not listed."
                    ));
                }
                break;
            }
        };
        if super::klv::is_partition(&k.key) && k.key.0[13] == 0x04 {
            break;
        }
        if k.end() > len {
            f.truncated = true;
            break;
        }
        if is_picture(&k.key, t.track_number) {
            if f.ranges.len() as u64 >= MAX_FRAMES {
                break;
            }
            f.first_key.get_or_insert(k.key);
            f.ranges.push(ByteRange {
                offset: k.value,
                len: k.len,
            });
        }
        off = k.end();
    }
    if f.ranges.len() == 1 && t.duration.is_some_and(|d| d > 1) {
        f.clip_wrapped = true;
    }
    let _ = complete;
    f
}
