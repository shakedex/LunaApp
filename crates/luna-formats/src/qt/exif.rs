//! Just enough Exif (TIFF IFD0 + Exif sub-IFD) to read the camera fields that
//! Canon (`uuid`/CNTH thumbnail) and Panasonic (`PANA` preview) embed.

use super::bytes::clean_text;
use super::display;
use crate::types::ClipMeta;

/// Finds the first "Exif\0\0" APP1 payload in a blob and returns the TIFF
/// bytes after it, bounded by the APP1 segment length when one precedes it.
pub(crate) fn find_tiff(blob: &[u8]) -> Option<&[u8]> {
    const SIG: &[u8] = b"Exif\0\0";
    let at = blob.windows(SIG.len()).position(|w| w == SIG)?;
    let start = at + SIG.len();
    let end = if at >= 4 && blob[at - 4] == 0xFF && blob[at - 3] == 0xE1 {
        let seg = u16::from_be_bytes([blob[at - 2], blob[at - 1]]) as usize;
        (at - 2 + seg).min(blob.len())
    } else {
        blob.len()
    };
    blob.get(start..end.max(start))
}

#[derive(Clone, Copy)]
struct Tiff<'a> {
    b: &'a [u8],
    le: bool,
}

#[derive(Debug, Clone, Copy)]
struct Entry {
    tag: u16,
    ty: u16,
    count: u32,
    /// Offset of the value in the TIFF buffer (inline values point into the entry).
    at: usize,
}

const MAX_ENTRIES: usize = 512;

impl<'a> Tiff<'a> {
    fn u16(&self, at: usize) -> Option<u16> {
        let s = self.b.get(at..at + 2)?;
        Some(if self.le {
            u16::from_le_bytes([s[0], s[1]])
        } else {
            u16::from_be_bytes([s[0], s[1]])
        })
    }

    fn u32(&self, at: usize) -> Option<u32> {
        let s = self.b.get(at..at + 4)?;
        let a = [s[0], s[1], s[2], s[3]];
        Some(if self.le {
            u32::from_le_bytes(a)
        } else {
            u32::from_be_bytes(a)
        })
    }

    fn ifd(&self, at: usize) -> Vec<Entry> {
        let Some(n) = self.u16(at) else {
            return Vec::new();
        };
        let mut out = Vec::new();
        for i in 0..(n as usize).min(MAX_ENTRIES) {
            let e = at + 2 + 12 * i;
            let (Some(tag), Some(ty), Some(count)) =
                (self.u16(e), self.u16(e + 2), self.u32(e + 4))
            else {
                break;
            };
            let unit = match ty {
                1 | 2 | 6 | 7 => 1,
                3 | 8 => 2,
                4 | 9 | 11 => 4,
                5 | 10 | 12 => 8,
                _ => continue,
            };
            let Some(size) = (count as usize).checked_mul(unit) else {
                continue;
            };
            let at = if size <= 4 {
                e + 8
            } else {
                match self.u32(e + 8) {
                    Some(o) => o as usize,
                    None => continue,
                }
            };
            if at.checked_add(size).is_none_or(|end| end > self.b.len()) {
                continue;
            }
            out.push(Entry { tag, ty, count, at });
        }
        out
    }

    fn ascii(&self, e: &Entry) -> Option<String> {
        (e.ty == 2)
            .then(|| self.b.get(e.at..e.at + e.count as usize))
            .flatten()
            .and_then(clean_text)
    }

    fn rational(&self, e: &Entry) -> Option<(u32, u32)> {
        (e.ty == 5 && e.count >= 1).then_some((self.u32(e.at)?, self.u32(e.at + 4)?))
    }

    fn uint(&self, e: &Entry) -> Option<u32> {
        match e.ty {
            3 => self.u16(e.at).map(u32::from),
            4 => self.u32(e.at),
            _ => None,
        }
    }
}

/// Camera fields from an Exif TIFF block.
pub(crate) fn read(tiff: &[u8]) -> ClipMeta {
    let mut m = ClipMeta::default();
    let le = match tiff.get(0..4) {
        Some(b"II*\0") => true,
        Some(b"MM\0*") => false,
        _ => return m,
    };
    let t = Tiff { b: tiff, le };
    let Some(ifd0) = t.u32(4) else { return m };
    let mut date_time = None;
    let mut exif_ifd = None;
    for e in t.ifd(ifd0 as usize) {
        match e.tag {
            0x010F => m.camera_make = t.ascii(&e),
            0x0110 => m.camera_model = t.ascii(&e),
            0x0132 => date_time = t.ascii(&e).and_then(|s| display::exif_local(&s)),
            0x8769 => exif_ifd = t.uint(&e),
            _ => {}
        }
    }
    let mut original = None;
    if let Some(at) = exif_ifd {
        for e in t.ifd(at as usize) {
            match e.tag {
                0x829A => {
                    m.shutter = t
                        .rational(&e)
                        .and_then(|(n, d)| display::exposure_time(n, d))
                }
                0x829D => {
                    m.stop = t
                        .rational(&e)
                        .filter(|&(_, d)| d != 0)
                        .and_then(|(n, d)| display::f_number(n as f64 / d as f64))
                }
                0x8827 => m.iso = t.uint(&e).filter(|&v| v > 0).map(|v| v.to_string()),
                0x9003 => original = t.ascii(&e).and_then(|s| display::exif_local(&s)),
                0x920A => {
                    m.focal_length = t
                        .rational(&e)
                        .filter(|&(_, d)| d != 0)
                        .and_then(|(n, d)| display::focal_length(n as f64 / d as f64))
                }
                0xA431 => m.camera_serial = t.ascii(&e),
                0xA434 => m.lens = t.ascii(&e),
                _ => {}
            }
        }
    }
    m.recording_date = original.or(date_time);
    m
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A little-endian TIFF with IFD0 {Make, ExifIFD} and Exif {ISO, FNumber}.
    fn sample() -> Vec<u8> {
        let mut b = b"II*\0".to_vec();
        b.extend_from_slice(&8u32.to_le_bytes());
        // IFD0 at 8: 2 entries
        b.extend_from_slice(&2u16.to_le_bytes());
        b.extend_from_slice(&0x010Fu16.to_le_bytes());
        b.extend_from_slice(&2u16.to_le_bytes());
        b.extend_from_slice(&4u32.to_le_bytes());
        b.extend_from_slice(b"DJI\0");
        b.extend_from_slice(&0x8769u16.to_le_bytes());
        b.extend_from_slice(&4u16.to_le_bytes());
        b.extend_from_slice(&1u32.to_le_bytes());
        b.extend_from_slice(&38u32.to_le_bytes());
        b.extend_from_slice(&0u32.to_le_bytes());
        // Exif IFD at 38: 2 entries, rational data at 38+2+24+4 = 68
        b.extend_from_slice(&2u16.to_le_bytes());
        b.extend_from_slice(&0x8827u16.to_le_bytes());
        b.extend_from_slice(&3u16.to_le_bytes());
        b.extend_from_slice(&1u32.to_le_bytes());
        b.extend_from_slice(&[0x40, 0x06, 0, 0]);
        b.extend_from_slice(&0x829Du16.to_le_bytes());
        b.extend_from_slice(&5u16.to_le_bytes());
        b.extend_from_slice(&1u32.to_le_bytes());
        b.extend_from_slice(&68u32.to_le_bytes());
        b.extend_from_slice(&0u32.to_le_bytes());
        b.extend_from_slice(&56u32.to_le_bytes());
        b.extend_from_slice(&10u32.to_le_bytes());
        b
    }

    #[test]
    fn reads_ifd0_and_exif_ifd() {
        let m = read(&sample());
        assert_eq!(m.camera_make.as_deref(), Some("DJI"));
        assert_eq!(m.iso.as_deref(), Some("1600"));
        assert_eq!(m.stop.as_deref(), Some("f/5.6"));
    }

    #[test]
    fn finds_tiff_in_app1() {
        let mut jpeg = vec![0xFF, 0xD8, 0xFF, 0xE1];
        let tiff = sample();
        jpeg.extend_from_slice(&((tiff.len() + 8) as u16).to_be_bytes());
        jpeg.extend_from_slice(b"Exif\0\0");
        jpeg.extend_from_slice(&tiff);
        jpeg.extend_from_slice(&[0xFF, 0xD9, 1, 2, 3]);
        assert_eq!(find_tiff(&jpeg), Some(tiff.as_slice()));
    }

    #[test]
    fn hostile_offsets_are_ignored() {
        let mut b = sample();
        // Point the Exif IFD past the end.
        b[30..34].copy_from_slice(&0xFFFF_FFF0u32.to_le_bytes());
        let m = read(&b);
        assert_eq!(m.camera_make.as_deref(), Some("DJI"));
        assert_eq!(m.iso, None);
        assert_eq!(read(b"II*\0\xFF\xFF\xFF\xFF"), ClipMeta::default());
    }
}
