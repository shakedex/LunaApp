//! Canon XF-HEVC/XF-AVC MP4 (Cinema EOS): a movie-level `uuid` box carries a
//! thumbnail JPEG whose Exif has make, model, date, exposure and lens; the
//! `cndm` timed-metadata track carries SMPTE RDD 18 acquisition metadata sets
//! per frame, of which the first frame's standard tags and Canon's private
//! transfer/primaries labels are read.

use super::display;
use crate::types::ClipMeta;

/// The `uuid` user type of Canon's metadata box.
pub(crate) const UUID: [u8; 16] = [
    0x85, 0xC0, 0xB6, 0x87, 0x82, 0x0F, 0x11, 0xE0, 0x81, 0x11, 0xF4, 0xCE, 0x46, 0x2B, 0x6A, 0x48,
];

pub(crate) fn read_uuid(payload: &[u8]) -> Option<ClipMeta> {
    let tiff = super::exif::find_tiff(payload)?;
    Some(super::exif::read(tiff))
}

/// RDD 18 metadata set keys share this 12-byte prefix (SMPTE UL, local set
/// with 2-byte tags and 2-byte lengths); byte 12 names the set.
const RDD18_SET_PREFIX: [u8; 12] = [
    0x06, 0x0E, 0x2B, 0x34, 0x02, 0x53, 0x01, 0x01, 0x0C, 0x02, 0x01, 0x01,
];
const CAMERA_UNIT_SET: u8 = 0x02;
/// Canon's private set: its transfer (0xE211) and primaries (0xE212) labels.
const CANON_SET: u8 = 0x7F;
const TAG_CANON_TRANSFER: u16 = 0xE211;
const TAG_CANON_PRIMARIES: u16 = 0xE212;

/// RDD 18 Camera Unit tags.
const TAG_SHUTTER_ANGLE: u16 = 0x8108;
const TAG_ISO: u16 = 0x810B;
const TAG_WHITE_BALANCE: u16 = 0x810E;

/// Reads the standard RDD 18 Camera Unit tags from one `cndm` sample
/// (white balance in kelvin, ISO, shutter angle in 1/60 degree units) and the
/// transfer and primaries labels from Canon's private set.
pub(crate) fn read_rdd18(sample: &[u8]) -> ClipMeta {
    let mut m = ClipMeta::default();
    let mut p = 0usize;
    let mut sets = 0;
    while let Some(start) = sample
        .get(p..)
        .and_then(|s| find(s, &RDD18_SET_PREFIX))
        .map(|i| p + i)
    {
        sets += 1;
        if sets > 64 {
            break;
        }
        let Some(&set) = sample.get(start + 12) else {
            break;
        };
        let Some((len, hdr)) = ber_len(sample.get(start + 16..).unwrap_or(&[])) else {
            break;
        };
        let body_start = start + 16 + hdr;
        let body_end = body_start.saturating_add(len).min(sample.len());
        if set == CAMERA_UNIT_SET || set == CANON_SET {
            let mut q = body_start;
            while q + 4 <= body_end {
                let tag = u16::from_be_bytes([sample[q], sample[q + 1]]);
                let l = u16::from_be_bytes([sample[q + 2], sample[q + 3]]) as usize;
                let Some(v) = sample
                    .get(q + 4..q + 4 + l)
                    .filter(|_| q + 4 + l <= body_end)
                else {
                    break;
                };
                let uint = || v.iter().fold(0u64, |a, &b| (a << 8) | b as u64);
                let label = || crate::mxf::klv::Ul::from_slice(v);
                match tag {
                    TAG_CANON_TRANSFER if set == CANON_SET => {
                        m.colour.transfer = label().as_ref().and_then(crate::mxf::labels::transfer)
                    }
                    TAG_CANON_PRIMARIES if set == CANON_SET => {
                        m.colour.gamut = label().as_ref().and_then(crate::mxf::labels::gamut)
                    }
                    _ if set != CAMERA_UNIT_SET => {}
                    TAG_WHITE_BALANCE if l == 2 => m.white_balance = display::kelvin(uint() as i64),
                    TAG_ISO if l == 2 => {
                        m.iso = Some(uint()).filter(|&v| v > 0).map(|v| v.to_string())
                    }
                    TAG_SHUTTER_ANGLE if l == 4 => {
                        m.shutter = display::shutter_angle(uint() as f64 / 60.0)
                    }
                    _ => {}
                }
                q += 4 + l;
            }
        }
        p = body_end.max(start + 16);
    }
    m
}

fn find(hay: &[u8], needle: &[u8]) -> Option<usize> {
    hay.windows(needle.len()).position(|w| w == needle)
}

/// BER length: short form, or 0x8N followed by N bytes.
fn ber_len(b: &[u8]) -> Option<(usize, usize)> {
    let first = *b.first()?;
    if first < 0x80 {
        return Some((first as usize, 1));
    }
    let n = (first & 0x7F) as usize;
    if n == 0 || n > 8 {
        return None;
    }
    let bytes = b.get(1..1 + n)?;
    let v = bytes.iter().fold(0u64, |a, &x| (a << 8) | x as u64);
    Some((usize::try_from(v).ok()?, 1 + n))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{Gamut, Transfer};

    fn set(kind: u8, items: &[(u16, &[u8])]) -> Vec<u8> {
        let mut body = Vec::new();
        for (tag, v) in items {
            body.extend_from_slice(&tag.to_be_bytes());
            body.extend_from_slice(&(v.len() as u16).to_be_bytes());
            body.extend_from_slice(v);
        }
        let mut s = RDD18_SET_PREFIX.to_vec();
        s.extend_from_slice(&[kind, 0x01, 0x00, 0x00]);
        s.extend_from_slice(&[0x83, 0, 0, body.len() as u8]);
        s.extend(body);
        s
    }

    #[test]
    fn camera_unit_tags() {
        let mut sample = vec![1, 0, 0, 0];
        sample.extend(set(0x01, &[(0x810E, &[0x11, 0x11])]));
        sample.extend(set(
            CAMERA_UNIT_SET,
            &[
                (0x8106, &[0, 0, 0x61, 0xA8, 0, 0, 3, 0xE8]),
                (0x8108, &[0, 0, 0x2A, 0x30]),
                (0x810B, &[0x03, 0x20]),
                (0x810E, &[0x15, 0xE0]),
            ],
        ));
        let m = read_rdd18(&sample);
        assert_eq!(m.white_balance.as_deref(), Some("5600 K"));
        assert_eq!(m.iso.as_deref(), Some("800"));
        assert_eq!(m.shutter.as_deref(), Some("180°"));
    }

    #[test]
    fn canon_private_colour_labels() {
        // As written by an EOS C50 recording Canon Log 2 / Cinema Gamut.
        let transfer = [
            0x06, 0x0E, 0x2B, 0x34, 0x04, 0x01, 0x01, 0x0D, 0x0E, 0x15, 0x00, 0x01, 0x02, 0, 0, 0,
        ];
        let primaries = [
            0x06, 0x0E, 0x2B, 0x34, 0x04, 0x01, 0x01, 0x0D, 0x0E, 0x15, 0x00, 0x02, 0x02, 0, 0, 0,
        ];
        let sample = set(
            CANON_SET,
            &[
                (TAG_CANON_TRANSFER, &transfer),
                (TAG_CANON_PRIMARIES, &primaries),
            ],
        );
        let m = read_rdd18(&sample);
        assert_eq!(m.colour.transfer, Some(Transfer::CanonLog2));
        assert_eq!(m.colour.gamut, Some(Gamut::CinemaGamut));
        // The same tags outside Canon's set are not colour labels.
        let m = read_rdd18(&set(CAMERA_UNIT_SET, &[(TAG_CANON_TRANSFER, &transfer)]));
        assert_eq!(m.colour.transfer, None);
    }

    #[test]
    fn truncated_set_is_harmless() {
        let s = set(CAMERA_UNIT_SET, &[(0x810E, &[0x15, 0xE0])]);
        for cut in 0..s.len() {
            let _ = read_rdd18(&s[..cut]);
        }
    }
}
