//! Sample descriptions: the video entry (codec, size, colour, aspect) and the
//! timecode entry.

use super::atom::{Atoms, child};
use super::bytes::{Reader, clean_text};
use crate::types::{Codec, ColourHint, Gamut, ProResProfile, Rational, Transfer};

pub(crate) fn codec(fourcc: &[u8; 4]) -> Codec {
    match fourcc {
        b"apco" => Codec::ProRes(ProResProfile::Proxy),
        b"apcs" => Codec::ProRes(ProResProfile::Lt),
        b"apcn" => Codec::ProRes(ProResProfile::Standard),
        b"apch" => Codec::ProRes(ProResProfile::Hq),
        b"ap4h" => Codec::ProRes(ProResProfile::P4444),
        b"ap4x" => Codec::ProRes(ProResProfile::P4444Xq),
        b"aprn" | b"aprh" => Codec::ProResRaw,
        b"avc1" | b"avc3" => Codec::H264,
        b"hvc1" | b"hev1" => Codec::Hevc,
        other => Codec::Other(String::from_utf8_lossy(other).trim().to_string()),
    }
}

/// One sound sample description: channels, rate, and bit depth when the
/// format is uncompressed PCM (a compressed format's sample size is nominal).
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub(crate) struct SoundEntry {
    pub channels: u32,
    pub sample_rate: Option<u32>,
    pub bits: Option<u32>,
}

/// QuickTime sound description v0/v1/v2 and the ISO `AudioSampleEntry`
/// (which shares the v0 layout), from the body after the 8-byte header.
pub(crate) fn sound_entry(format: &[u8; 4], body: &[u8]) -> Option<SoundEntry> {
    let mut r = Reader::new(body);
    r.skip(6 + 2)?;
    let version = r.u16()?;
    r.skip(2 + 4)?;
    let (channels, sample_size, sample_rate, v2_bits) = if version == 2 {
        r.skip(2 + 2 + 2 + 2 + 4 + 4)?;
        let rate = f64::from_bits(r.u64()?);
        let channels = r.u32()?;
        r.skip(4)?;
        let bits = r.u32()?;
        let rate = (1.0..10_000_000.0)
            .contains(&rate)
            .then(|| rate.round() as u32);
        (channels, None, rate, Some(bits).filter(|&b| b > 0))
    } else {
        let channels = r.u16()? as u32;
        let size = r.u16()? as u32;
        r.skip(2 + 2)?;
        let rate = r.u32()? >> 16;
        (channels, Some(size), Some(rate).filter(|&r| r > 0), None)
    };
    let bits = match format {
        b"in24" => Some(24),
        b"in32" | b"fl32" => Some(32),
        b"fl64" => Some(64),
        b"lpcm" => v2_bits,
        b"twos" | b"sowt" | b"raw " | b"NONE" | b"ipcm" => v2_bits.or(sample_size),
        _ => None,
    }
    .filter(|&b| (8..=64).contains(&b));
    Some(SoundEntry {
        channels,
        sample_rate,
        bits,
    })
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub(crate) struct VideoEntry {
    pub width: u32,
    pub height: u32,
    /// Clean-aperture size when `clap` gives a whole-pixel one.
    pub clean: Option<(u32, u32)>,
    pub pixel_aspect: Option<Rational>,
    pub colour: ColourHint,
    pub fields: Option<u8>,
    /// The decoder configuration record (`avcC` / `hvcC` body), verbatim.
    pub config: Option<Vec<u8>>,
}

/// Bytes of the visual sample entry before its extension atoms
/// (after the 8-byte size/format header, which the caller strips).
const VISUAL_ENTRY_FIXED: usize = 78;

pub(crate) fn video_entry(body: &[u8]) -> Option<VideoEntry> {
    let mut r = Reader::new(body);
    r.skip(6 + 2 + 16)?;
    let width = r.u16()? as u32;
    let height = r.u16()? as u32;
    let mut v = VideoEntry {
        width,
        height,
        ..VideoEntry::default()
    };
    let ext = body.get(VISUAL_ENTRY_FIXED..).unwrap_or(&[]);
    for a in Atoms::new(ext) {
        match &a.kind {
            b"colr" => v.colour = colr(a.body),
            b"pasp" => {
                let mut r = Reader::new(a.body);
                if let (Some(h), Some(vs)) = (r.u32(), r.u32())
                    && h > 0
                    && vs > 0
                {
                    v.pixel_aspect = Some(reduce(h, vs));
                }
            }
            b"clap" => v.clean = clap(a.body),
            b"fiel" => v.fields = a.body.first().copied(),
            b"avcC" | b"hvcC" => v.config = Some(a.body.to_vec()),
            _ => {}
        }
    }
    Some(v)
}

fn clap(b: &[u8]) -> Option<(u32, u32)> {
    let mut r = Reader::new(b);
    let (wn, wd, hn, hd) = (r.u32()?, r.u32()?, r.u32()?, r.u32()?);
    if wd == 0 || hd == 0 || wn % wd != 0 || hn % hd != 0 {
        return None;
    }
    let (w, h) = (wn / wd, hn / hd);
    (w > 0 && h > 0).then_some((w, h))
}

/// `nclc` / `nclx`: only the codes that name something a viewer can act on.
fn colr(b: &[u8]) -> ColourHint {
    let mut r = Reader::new(b);
    let Some(kind) = r.fourcc() else {
        return ColourHint::default();
    };
    if &kind != b"nclc" && &kind != b"nclx" {
        return ColourHint::default();
    }
    let (Some(primaries), Some(transfer)) = (r.u16(), r.u16()) else {
        return ColourHint::default();
    };
    ColourHint {
        gamut: match primaries {
            1 => Some(Gamut::Rec709),
            9 => Some(Gamut::Rec2020),
            _ => None,
        },
        transfer: match transfer {
            1 | 6 | 14 | 15 => Some(Transfer::Rec709),
            8 => Some(Transfer::Linear),
            16 => Some(Transfer::Pq),
            18 => Some(Transfer::Hlg),
            _ => None,
        },
    }
}

pub(crate) fn reduce(num: u32, den: u32) -> Rational {
    let g = gcd(num, den).max(1);
    Rational {
        num: num / g,
        den: den / g,
    }
}

fn gcd(mut a: u32, mut b: u32) -> u32 {
    while b != 0 {
        (a, b) = (b, a % b);
    }
    a
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TimecodeEntry {
    pub drop_frame: bool,
    pub counter: bool,
    pub timescale: u32,
    pub frame_duration: u32,
    pub frames_per_second: u8,
    pub name: Option<String>,
}

pub(crate) fn timecode_entry(body: &[u8]) -> Option<TimecodeEntry> {
    let mut r = Reader::new(body);
    r.skip(6 + 2 + 4)?;
    let flags = r.u32()?;
    let timescale = r.u32()?;
    let frame_duration = r.u32()?;
    let frames_per_second = r.u8()?;
    r.skip(1)?;
    let name = child(r.rest(), b"name").and_then(|n| {
        let mut r = Reader::new(n);
        let len = r.u16()? as usize;
        r.skip(2)?;
        clean_text(r.take(len)?)
    });
    Some(TimecodeEntry {
        drop_frame: flags & 1 != 0,
        counter: flags & 8 != 0,
        timescale,
        frame_duration,
        frames_per_second,
        name,
    })
}

impl TimecodeEntry {
    /// The integer counting rate: the entry's frame count, or the nearest
    /// whole rate of timescale/duration when a writer left it zero.
    pub(crate) fn base(&self) -> Option<u32> {
        if self.frames_per_second > 0 {
            return Some(self.frames_per_second as u32);
        }
        if self.frame_duration == 0 {
            return None;
        }
        let r = (self.timescale as f64 / self.frame_duration as f64).round() as u32;
        (r > 0).then_some(r)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn visual_entry(w: u16, h: u16, ext: &[u8]) -> Vec<u8> {
        let mut b = vec![0u8; 24];
        b.extend_from_slice(&w.to_be_bytes());
        b.extend_from_slice(&h.to_be_bytes());
        b.resize(VISUAL_ENTRY_FIXED, 0);
        b.extend_from_slice(ext);
        b
    }

    fn atom(kind: &[u8; 4], body: &[u8]) -> Vec<u8> {
        let mut v = ((body.len() + 8) as u32).to_be_bytes().to_vec();
        v.extend_from_slice(kind);
        v.extend_from_slice(body);
        v
    }

    #[test]
    fn codec_table() {
        assert_eq!(codec(b"ap4x"), Codec::ProRes(ProResProfile::P4444Xq));
        assert_eq!(codec(b"apch"), Codec::ProRes(ProResProfile::Hq));
        assert_eq!(codec(b"aprh"), Codec::ProResRaw);
        assert_eq!(codec(b"hev1"), Codec::Hevc);
        assert_eq!(codec(b"mp4v"), Codec::Other("mp4v".into()));
    }

    #[test]
    fn reads_size_aspect_colour_fields() {
        let mut ext = atom(b"colr", b"nclx\x00\x09\x00\x12\x00\x09\x00");
        ext.extend(atom(b"pasp", &[0, 0, 0, 4, 0, 0, 0, 3]));
        ext.extend(atom(b"fiel", &[2, 9]));
        ext.extend(atom(
            b"clap",
            &[
                0, 0, 7, 128, 0, 0, 0, 1, 0, 0, 4, 56, 0, 0, 0, 1, 0, 0, 0, 0,
            ],
        ));
        ext.extend_from_slice(&[0, 0, 0, 0]);
        let v = video_entry(&visual_entry(1920, 1088, &ext)).unwrap();
        assert_eq!((v.width, v.height), (1920, 1088));
        assert_eq!(v.clean, Some((1920, 1080)));
        assert_eq!(v.pixel_aspect, Some(Rational { num: 4, den: 3 }));
        assert_eq!(v.colour.gamut, Some(Gamut::Rec2020));
        assert_eq!(v.colour.transfer, Some(Transfer::Hlg));
        assert_eq!(v.fields, Some(2));
    }

    #[test]
    fn keeps_the_decoder_configuration_record() {
        let ext = atom(b"avcC", &[1, 122, 0, 52, 0xff, 0xe0, 0]);
        let v = video_entry(&visual_entry(10, 10, &ext)).unwrap();
        assert_eq!(v.config, Some(vec![1, 122, 0, 52, 0xff, 0xe0, 0]));
        let v = video_entry(&visual_entry(10, 10, &[])).unwrap();
        assert_eq!(v.config, None);
    }

    #[test]
    fn unknown_colr_codes_say_nothing() {
        let ext = atom(b"colr", b"nclc\x00\x02\x00\x02\x00\x02");
        let v = video_entry(&visual_entry(10, 10, &ext)).unwrap();
        assert_eq!(v.colour, ColourHint::default());
    }

    #[test]
    fn timecode_entry_with_name() {
        let mut b = vec![0u8; 12];
        b.extend_from_slice(&3u32.to_be_bytes());
        b.extend_from_slice(&30000u32.to_be_bytes());
        b.extend_from_slice(&1001u32.to_be_bytes());
        b.extend_from_slice(&[30, 0]);
        b.extend(atom(b"name", b"\x00\x04\x00\x00A001"));
        let t = timecode_entry(&b).unwrap();
        assert!(t.drop_frame);
        assert!(!t.counter);
        assert_eq!(t.base(), Some(30));
        assert_eq!(t.name.as_deref(), Some("A001"));
    }

    #[test]
    fn timecode_base_falls_back_to_rate() {
        let t = TimecodeEntry {
            drop_frame: false,
            counter: false,
            timescale: 24000,
            frame_duration: 1001,
            frames_per_second: 0,
            name: None,
        };
        assert_eq!(t.base(), Some(24));
    }
}
