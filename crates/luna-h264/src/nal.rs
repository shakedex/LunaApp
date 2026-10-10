//! NAL units from Annex B byte streams and from length-prefixed (AVCC)
//! samples, and the `avcC` decoder configuration record (ISO/IEC 14496-15).

use crate::Error;

pub(crate) const NAL_SLICE: u8 = 1;
pub(crate) const NAL_IDR: u8 = 5;
pub(crate) const NAL_SPS: u8 = 7;
pub(crate) const NAL_PPS: u8 = 8;

#[derive(Debug, Clone, Copy)]
pub(crate) struct Nal<'a> {
    pub ref_idc: u8,
    pub kind: u8,
    /// The bytes after the one-byte NAL header, still escaped.
    pub payload: &'a [u8],
}

fn nal(bytes: &[u8]) -> Option<Nal<'_>> {
    let (&h, payload) = bytes.split_first()?;
    if h & 0x80 != 0 {
        return None;
    }
    Some(Nal {
        ref_idc: (h >> 5) & 3,
        kind: h & 0x1f,
        payload,
    })
}

/// NAL units of an Annex B stream (start codes `00 00 01` / `00 00 00 01`).
/// Trailing zero bytes of each unit are dropped.
pub(crate) fn annex_b(data: &[u8]) -> Vec<Nal<'_>> {
    let mut starts = Vec::new();
    let mut i = 0;
    while i + 2 < data.len() {
        if data[i] == 0 && data[i + 1] == 0 && data[i + 2] == 1 {
            starts.push(i + 3);
            i += 3;
        } else {
            i += 1;
        }
    }
    let mut out = Vec::with_capacity(starts.len());
    for (k, &s) in starts.iter().enumerate() {
        let mut e = starts.get(k + 1).map_or(data.len(), |&n| n - 3);
        while e > s && data[e - 1] == 0 {
            e -= 1;
        }
        if let Some(n) = nal(&data[s..e]) {
            out.push(n);
        }
    }
    out
}

/// NAL units of a length-prefixed sample (`len_size` bytes, big-endian).
pub(crate) fn length_prefixed(data: &[u8], len_size: u8) -> Result<Vec<Nal<'_>>, Error> {
    let n = len_size as usize;
    let mut out = Vec::new();
    let mut i = 0;
    while i < data.len() {
        let Some(head) = data.get(i..i + n) else {
            return Err(Error::corrupt("a NAL length is cut off"));
        };
        let len = head.iter().fold(0usize, |a, &b| (a << 8) | b as usize);
        i += n;
        let Some(body) = data.get(i..i.saturating_add(len)) else {
            return Err(Error::corrupt("a NAL unit runs past the sample"));
        };
        i += len;
        if let Some(u) = nal(body) {
            out.push(u);
        }
    }
    Ok(out)
}

/// Like [`length_prefixed`] for the first bytes of a sample: the last NAL
/// unit may be cut short, and a cut-off length ends the list.
pub(crate) fn length_prefixed_lenient(data: &[u8], len_size: u8) -> Vec<Nal<'_>> {
    let n = len_size as usize;
    let mut out = Vec::new();
    let mut i = 0;
    while let Some(head) = data.get(i..i + n) {
        let len = head.iter().fold(0usize, |a, &b| (a << 8) | b as usize);
        i += n;
        let end = i.saturating_add(len).min(data.len());
        if let Some(u) = data.get(i..end).and_then(nal) {
            out.push(u);
        }
        i = i.saturating_add(len);
    }
    out
}

/// The parts of an `avcC` record a decoder needs.
pub(crate) struct AvcC<'a> {
    pub len_size: u8,
    pub sps: Vec<&'a [u8]>,
    pub pps: Vec<&'a [u8]>,
}

pub(crate) fn avcc(b: &[u8]) -> Result<AvcC<'_>, Error> {
    let bad = || Error::corrupt("the avcC record is damaged");
    if b.len() < 7 || b[0] != 1 {
        return Err(bad());
    }
    let len_size = (b[4] & 3) + 1;
    if len_size == 3 {
        return Err(bad());
    }
    let mut i = 5;
    let take_sets = |count: usize, i: &mut usize| -> Result<Vec<&[u8]>, Error> {
        let mut v = Vec::with_capacity(count);
        for _ in 0..count {
            let l = b.get(*i..*i + 2).ok_or_else(bad)?;
            let l = u16::from_be_bytes([l[0], l[1]]) as usize;
            *i += 2;
            v.push(b.get(*i..*i + l).ok_or_else(bad)?);
            *i += l;
        }
        Ok(v)
    };
    let nsps = (b[i] & 0x1f) as usize;
    i += 1;
    let sps = take_sets(nsps, &mut i)?;
    let npps = *b.get(i).ok_or_else(bad)? as usize;
    i += 1;
    let pps = take_sets(npps, &mut i)?;
    Ok(AvcC { len_size, sps, pps })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_annex_b() {
        let s = [
            0, 0, 0, 1, 0x67, 1, 2, 0, 0, 1, 0x68, 3, 0, 0, 0, 0, 1, 0x65, 0, 0, 3, 1,
        ];
        let v = annex_b(&s);
        assert_eq!(v.len(), 3);
        assert_eq!((v[0].kind, v[0].payload), (7, &[1u8, 2][..]));
        assert_eq!((v[1].kind, v[1].payload), (8, &[3u8][..]));
        assert_eq!((v[2].kind, v[2].ref_idc), (5, 3));
        assert_eq!(v[2].payload, &[0, 0, 3, 1]);
    }

    #[test]
    fn splits_length_prefixed() {
        let s = [0, 0, 0, 2, 0x65, 9, 0, 0, 0, 1, 0x06];
        let v = length_prefixed(&s, 4).unwrap();
        assert_eq!(v.len(), 2);
        assert_eq!(v[0].payload, &[9]);
        assert!(length_prefixed(&[0, 0, 0, 9, 1], 4).is_err());
        assert!(length_prefixed(&[0, 0], 4).is_err());
        let cut = length_prefixed_lenient(&s[..7], 4);
        assert_eq!(cut.len(), 1);
        assert_eq!(cut[0].payload, &[9]);
        assert_eq!(length_prefixed_lenient(&s[..5], 4)[0].payload, &[] as &[u8]);
    }

    #[test]
    fn parses_avcc() {
        let rec = [
            1, 122, 0, 52, 0xff, 0xe1, 0, 2, 0x67, 7, 1, 0, 3, 0x68, 1, 2,
        ];
        let a = avcc(&rec).unwrap();
        assert_eq!(a.len_size, 4);
        assert_eq!(a.sps, vec![&[0x67, 7][..]]);
        assert_eq!(a.pps, vec![&[0x68, 1, 2][..]]);
        assert!(avcc(&rec[..10]).is_err());
        assert!(avcc(&[]).is_err());
    }
}
