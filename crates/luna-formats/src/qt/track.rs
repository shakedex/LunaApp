//! One `trak`: handler, timescale, sample description and sample tables.

use super::atom::{Atoms, child, path};
use super::bytes::{Reader, table_fits};
use crate::types::ByteRange;

#[derive(Debug, Default, Clone)]
pub(crate) struct Track {
    pub id: u32,
    pub handler: [u8; 4],
    pub timescale: u32,
    /// The first sample entry's format and its body (after the 8-byte header).
    pub entry_format: [u8; 4],
    pub entry: Vec<u8>,
    pub stts: Vec<(u32, u32)>,
    pub ctts: Vec<(u32, i64)>,
    pub sizes: SampleSizes,
    /// (first_chunk, samples_per_chunk), first_chunk 1-based.
    pub stsc: Vec<(u32, u32)>,
    pub chunk_offsets: Vec<u64>,
    pub edits: Vec<Edit>,
    /// Track IDs this track names in `tref`/`tmcd`.
    pub tmcd_refs: Vec<u32>,
    /// Table atoms that were present but unreadable.
    pub damaged: Vec<&'static str>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum SampleSizes {
    Fixed { size: u32, count: u32 },
    Table(Vec<u32>),
}

impl Default for SampleSizes {
    fn default() -> Self {
        SampleSizes::Fixed { size: 0, count: 0 }
    }
}

impl SampleSizes {
    pub(crate) fn count(&self) -> u32 {
        match self {
            SampleSizes::Fixed { count, .. } => *count,
            SampleSizes::Table(t) => t.len() as u32,
        }
    }

    fn get(&self, i: usize) -> Option<u32> {
        match self {
            SampleSizes::Fixed { size, count } => (i < *count as usize).then_some(*size),
            SampleSizes::Table(t) => t.get(i).copied(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Edit {
    pub media_time: i64,
    pub rate_is_one: bool,
}

/// A clip with more coded frames than this is not something a camera wrote
/// (over 46 hours at 60 fps); refusing it bounds the frame-index allocation.
pub(crate) const MAX_SAMPLES: u32 = 10_000_000;

pub(crate) fn parse_track(trak: &[u8]) -> Track {
    let mut t = Track::default();
    if let Some(tkhd) = child(trak, b"tkhd") {
        let mut r = Reader::new(tkhd);
        if let Some(v) = r.u8() {
            let skip = if v == 1 { 3 + 16 } else { 3 + 8 };
            t.id = r.skip(skip).and_then(|_| r.u32()).unwrap_or(0);
        }
    }
    if let Some(tref) = child(trak, b"tref")
        && let Some(tm) = child(tref, b"tmcd")
    {
        t.tmcd_refs = tm
            .as_chunks::<4>()
            .0
            .iter()
            .map(|&c| u32::from_be_bytes(c))
            .collect();
    }
    if let Some(elst) = path(trak, &[b"edts", b"elst"]) {
        match parse_elst(elst) {
            Some(e) => t.edits = e,
            None => t.damaged.push("elst"),
        }
    }
    let Some(mdia) = child(trak, b"mdia") else {
        return t;
    };
    if let Some(mdhd) = child(mdia, b"mdhd") {
        let mut r = Reader::new(mdhd);
        if let Some(v) = r.u8() {
            let skip = if v == 1 { 3 + 16 } else { 3 + 8 };
            t.timescale = r.skip(skip).and_then(|_| r.u32()).unwrap_or(0);
        }
    }
    if let Some(hdlr) = child(mdia, b"hdlr") {
        let mut r = Reader::new(hdlr);
        if let Some(k) = r.skip(8).and_then(|_| r.fourcc()) {
            t.handler = k;
        }
    }
    let Some(stbl) = path(mdia, &[b"minf", b"stbl"]) else {
        return t;
    };
    for a in Atoms::new(stbl) {
        match &a.kind {
            b"stsd" => {
                if parse_stsd(a.body, &mut t).is_none() {
                    t.damaged.push("stsd");
                }
            }
            b"stts" => match parse_pairs(a.body) {
                Some(v) => t.stts = v,
                None => t.damaged.push("stts"),
            },
            b"ctts" => match parse_ctts(a.body) {
                Some(v) => t.ctts = v,
                None => t.damaged.push("ctts"),
            },
            b"stsz" => match parse_stsz(a.body) {
                Some(v) => t.sizes = v,
                None => t.damaged.push("stsz"),
            },
            b"stsc" => match parse_stsc(a.body) {
                Some(v) => t.stsc = v,
                None => t.damaged.push("stsc"),
            },
            b"stco" => match parse_offsets(a.body, false) {
                Some(v) => t.chunk_offsets = v,
                None => t.damaged.push("stco"),
            },
            b"co64" => match parse_offsets(a.body, true) {
                Some(v) => t.chunk_offsets = v,
                None => t.damaged.push("co64"),
            },
            _ => {}
        }
    }
    t
}

fn parse_stsd(b: &[u8], t: &mut Track) -> Option<()> {
    let mut r = Reader::new(b);
    r.skip(4)?;
    if r.u32()? == 0 {
        return None;
    }
    let size = r.u32()? as usize;
    let format = r.fourcc()?;
    let body = r.take(size.checked_sub(8)?)?;
    t.entry_format = format;
    t.entry = body.to_vec();
    Some(())
}

fn parse_pairs(b: &[u8]) -> Option<Vec<(u32, u32)>> {
    let mut r = Reader::new(b);
    r.skip(4)?;
    let n = r.u32()?;
    if !table_fits(&r, n, 8) {
        return None;
    }
    (0..n).map(|_| Some((r.u32()?, r.u32()?))).collect()
}

fn parse_ctts(b: &[u8]) -> Option<Vec<(u32, i64)>> {
    let mut r = Reader::new(b);
    // Version 0 is unsigned by the letter of ISO-BMFF, but writers put
    // negative offsets there too, so both versions are read signed.
    r.skip(4)?;
    let n = r.u32()?;
    if !table_fits(&r, n, 8) {
        return None;
    }
    (0..n).map(|_| Some((r.u32()?, r.i32()? as i64))).collect()
}

fn parse_stsz(b: &[u8]) -> Option<SampleSizes> {
    let mut r = Reader::new(b);
    r.skip(4)?;
    let size = r.u32()?;
    let count = r.u32()?;
    if count > MAX_SAMPLES {
        return None;
    }
    if size != 0 {
        return Some(SampleSizes::Fixed { size, count });
    }
    if !table_fits(&r, count, 4) {
        return None;
    }
    (0..count)
        .map(|_| r.u32())
        .collect::<Option<Vec<_>>>()
        .map(SampleSizes::Table)
}

fn parse_stsc(b: &[u8]) -> Option<Vec<(u32, u32)>> {
    let mut r = Reader::new(b);
    r.skip(4)?;
    let n = r.u32()?;
    if !table_fits(&r, n, 12) {
        return None;
    }
    (0..n)
        .map(|_| {
            let first = r.u32()?;
            let per = r.u32()?;
            r.skip(4)?;
            Some((first, per))
        })
        .collect()
}

fn parse_offsets(b: &[u8], wide: bool) -> Option<Vec<u64>> {
    let mut r = Reader::new(b);
    r.skip(4)?;
    let n = r.u32()?;
    if !table_fits(&r, n, if wide { 8 } else { 4 }) {
        return None;
    }
    (0..n)
        .map(|_| {
            if wide {
                r.u64()
            } else {
                r.u32().map(u64::from)
            }
        })
        .collect()
}

fn parse_elst(b: &[u8]) -> Option<Vec<Edit>> {
    let mut r = Reader::new(b);
    let version = r.u8()?;
    r.skip(3)?;
    let n = r.u32()?;
    if !table_fits(&r, n, if version == 1 { 20 } else { 12 }) {
        return None;
    }
    (0..n)
        .map(|_| {
            let media_time = if version == 1 {
                r.skip(8)?;
                r.u64()? as i64
            } else {
                r.skip(4)?;
                r.i32()? as i64
            };
            let rate = r.u32()?;
            Some(Edit {
                media_time,
                rate_is_one: rate == 0x0001_0000,
            })
        })
        .collect()
}

/// Byte ranges of every sample, in decode (file table) order. `Err` names
/// what is inconsistent.
pub(crate) fn sample_ranges(t: &Track) -> Result<Vec<ByteRange>, String> {
    let n = t.sizes.count() as usize;
    if n == 0 {
        return Ok(Vec::new());
    }
    if t.stsc.is_empty() || t.chunk_offsets.is_empty() {
        return Err("the sample-to-chunk or chunk-offset table is missing".into());
    }
    let chunks = t.chunk_offsets.len();
    let mut out = Vec::with_capacity(n.min(1 << 20));
    for (i, &(first, per)) in t.stsc.iter().enumerate() {
        let start = (first as usize)
            .checked_sub(1)
            .ok_or("the sample-to-chunk table starts at chunk 0")?;
        let end = match t.stsc.get(i + 1) {
            Some(&(next, _)) => (next as usize).saturating_sub(1).min(chunks),
            None => chunks,
        };
        if start > end || (i > 0 && first <= t.stsc[i - 1].0) {
            return Err("the sample-to-chunk table is out of order".into());
        }
        for &chunk_off in &t.chunk_offsets[start..end] {
            let mut off = chunk_off;
            for _ in 0..per {
                if out.len() == n {
                    break;
                }
                let len = t.sizes.get(out.len()).unwrap_or(0) as u64;
                out.push(ByteRange { offset: off, len });
                off = off.saturating_add(len);
            }
            if out.len() == n {
                return Ok(out);
            }
        }
    }
    Err(format!(
        "the chunk tables place {} of {} samples",
        out.len(),
        n
    ))
}

/// The byte range of the first sample, without building the whole index.
pub(crate) fn first_sample(t: &Track) -> Option<ByteRange> {
    let len = t.sizes.get(0)? as u64;
    let offset = *t.chunk_offsets.first()?;
    (t.stsc.first()?.0 == 1).then_some(ByteRange { offset, len })
}

/// Composition time of every sample, from `stts` + `ctts`. `None` without
/// `ctts` or when the tables don't cover all `n` samples.
fn composition_times(t: &Track, n: usize) -> Option<Vec<i64>> {
    if t.ctts.is_empty() {
        return None;
    }
    let mut dts = Vec::with_capacity(n);
    let mut clock = 0i64;
    for &(count, delta) in &t.stts {
        for _ in 0..count {
            if dts.len() == n {
                break;
            }
            dts.push(clock);
            clock = clock.saturating_add(delta as i64);
        }
    }
    let mut cts = Vec::with_capacity(n);
    for &(count, off) in &t.ctts {
        for _ in 0..count {
            if cts.len() == n {
                break;
            }
            cts.push(dts.get(cts.len())?.saturating_add(off));
        }
    }
    (dts.len() == n && cts.len() == n).then_some(cts)
}

/// Sample decode order → display order. `None` when there is no reordering
/// table (then decode order is display order).
pub(crate) fn display_order(t: &Track, n: usize) -> Option<Vec<usize>> {
    let cts = composition_times(t, n)?;
    let mut order: Vec<usize> = (0..n).collect();
    order.sort_by_key(|&i| cts[i]);
    Some(order)
}

/// The earliest composition time, which a single edit commonly starts at to
/// line the first displayed frame up with time zero (B-frame streams).
pub(crate) fn first_cts(t: &Track) -> i64 {
    composition_times(t, t.sizes.count() as usize)
        .and_then(|c| c.into_iter().min())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn track(sizes: SampleSizes, stsc: Vec<(u32, u32)>, offsets: Vec<u64>) -> Track {
        Track {
            sizes,
            stsc,
            chunk_offsets: offsets,
            ..Track::default()
        }
    }

    #[test]
    fn ranges_follow_chunks() {
        let t = track(
            SampleSizes::Table(vec![10, 20, 30, 40, 50]),
            vec![(1, 2), (3, 1)],
            vec![1000, 2000, 3000],
        );
        let r = sample_ranges(&t).unwrap();
        let got: Vec<_> = r.iter().map(|b| (b.offset, b.len)).collect();
        assert_eq!(
            got,
            vec![(1000, 10), (1010, 20), (2000, 30), (2030, 40), (3000, 50)]
        );
    }

    #[test]
    fn ranges_with_fixed_size() {
        let t = track(
            SampleSizes::Fixed { size: 4, count: 3 },
            vec![(1, 3)],
            vec![64],
        );
        let got: Vec<_> = sample_ranges(&t)
            .unwrap()
            .iter()
            .map(|b| b.offset)
            .collect();
        assert_eq!(got, vec![64, 68, 72]);
    }

    #[test]
    fn too_few_chunks_is_an_error() {
        let t = track(SampleSizes::Table(vec![1, 2, 3]), vec![(1, 1)], vec![0, 1]);
        assert!(sample_ranges(&t).is_err());
    }

    #[test]
    fn huge_samples_per_chunk_is_bounded_by_sample_count() {
        let t = track(
            SampleSizes::Fixed { size: 1, count: 2 },
            vec![(1, u32::MAX)],
            vec![0],
        );
        assert_eq!(sample_ranges(&t).unwrap().len(), 2);
    }

    #[test]
    fn display_order_from_ctts() {
        let mut t = Track {
            stts: vec![(4, 10)],
            ctts: vec![(1, 10), (1, 30), (1, 0), (1, 0)],
            ..Track::default()
        };
        // dts 0,10,20,30 → cts 10,40,20,30
        assert_eq!(display_order(&t, 4), Some(vec![0, 2, 3, 1]));
        t.sizes = SampleSizes::Fixed { size: 1, count: 4 };
        assert_eq!(first_cts(&t), 10);
        t.ctts.clear();
        assert_eq!(display_order(&t, 4), None);
    }

    #[test]
    fn stsz_lying_count_is_rejected() {
        let mut b = vec![0u8; 4];
        b.extend_from_slice(&0u32.to_be_bytes());
        b.extend_from_slice(&1000u32.to_be_bytes());
        b.extend_from_slice(&[0; 8]);
        assert!(parse_stsz(&b).is_none());
    }
}
