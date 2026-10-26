//! Whole-file tests over a movie built here and over moov-only fixtures cut
//! from the corpus (tests/fixtures/qt: the `moov` atom alone, padding dropped,
//! large ilst values emptied, embedded JPEGs reduced to their Exif segment),
//! plus the corruption harness.

use super::{Source, probe_source};
use crate::types::{
    ByteRange, Codec, FormatError, FrameIndex, Gamut, ProResProfile, Rational, Transfer,
};

fn atom(kind: &[u8; 4], body: &[u8]) -> Vec<u8> {
    let mut v = ((body.len() + 8) as u32).to_be_bytes().to_vec();
    v.extend_from_slice(kind);
    v.extend_from_slice(body);
    v
}

fn atoms(kind: &[u8; 4], children: &[Vec<u8>]) -> Vec<u8> {
    atom(kind, &children.concat())
}

fn be(vals: &[u32]) -> Vec<u8> {
    vals.iter().flat_map(|v| v.to_be_bytes()).collect()
}

fn tkhd(id: u32) -> Vec<u8> {
    let mut b = be(&[0, 0, 0, id]);
    b.resize(84, 0);
    atom(b"tkhd", &b)
}

fn mdhd(timescale: u32) -> Vec<u8> {
    atom(b"mdhd", &be(&[0, 0, 0, timescale, 0, 0]))
}

fn hdlr(kind: &[u8; 4]) -> Vec<u8> {
    let mut b = be(&[0]);
    b.extend_from_slice(b"mhlr");
    b.extend_from_slice(kind);
    b.extend_from_slice(&[0; 13]);
    atom(b"hdlr", &b)
}

fn stbl(entry: Vec<u8>, stts: (u32, u32), sizes: &[u32], chunk: u32) -> Vec<u8> {
    let mut stsd = be(&[0, 1]);
    stsd.extend(entry);
    let mut stsz = be(&[0, 0, sizes.len() as u32]);
    stsz.extend(be(sizes));
    atoms(
        b"stbl",
        &[
            atom(b"stsd", &stsd),
            atom(b"stts", &be(&[0, 1, stts.0, stts.1])),
            atom(b"stsz", &stsz),
            atom(b"stsc", &be(&[0, 1, 1, sizes.len() as u32, 1])),
            atom(b"stco", &be(&[0, 1, chunk])),
        ],
    )
}

fn video_entry() -> Vec<u8> {
    let mut b = vec![0u8; 24];
    b.extend_from_slice(&1920u16.to_be_bytes());
    b.extend_from_slice(&1080u16.to_be_bytes());
    b.resize(78, 0);
    b.extend(atom(b"colr", b"nclc\x00\x01\x00\x01\x00\x01"));
    b.extend(atom(b"fiel", &[1, 0]));
    b.extend(atom(b"pasp", &be(&[1, 1])));
    atom(b"apch", &b)
}

fn tmcd_entry() -> Vec<u8> {
    let mut b = vec![0u8; 12];
    b.extend(be(&[0, 24000, 1001]));
    b.extend_from_slice(&[24, 0]);
    b.extend(atom(b"name", b"\x00\x04\x00\x00A001"));
    atom(b"tmcd", &b)
}

fn udta_text(kind: &[u8; 4], s: &str) -> Vec<u8> {
    let mut b = (s.len() as u16).to_be_bytes().to_vec();
    b.extend_from_slice(&[0, 0]);
    b.extend_from_slice(s.as_bytes());
    atom(kind, &b)
}

const FRAME_SIZES: [u32; 3] = [100, 120, 90];

/// ftyp + mdat (three frames, one timecode sample) + moov.
fn movie() -> Vec<u8> {
    let ftyp = atom(b"ftyp", b"qt  \0\0\0\0qt  ");
    let frames_at = (ftyp.len() + 8) as u32;
    let total: u32 = FRAME_SIZES.iter().sum();
    let tc_at = frames_at + total;
    let mut mdat_body: Vec<u8> = (0..total).map(|i| i as u8).collect();
    mdat_body.extend_from_slice(&(86_400u32 + 5).to_be_bytes());
    let video = atoms(
        b"trak",
        &[
            tkhd(1),
            atoms(b"tref", &[atom(b"tmcd", &be(&[2]))]),
            atoms(
                b"mdia",
                &[
                    mdhd(24000),
                    hdlr(b"vide"),
                    atoms(
                        b"minf",
                        &[stbl(video_entry(), (3, 1001), &FRAME_SIZES, frames_at)],
                    ),
                ],
            ),
        ],
    );
    let tc = atoms(
        b"trak",
        &[
            tkhd(2),
            atoms(
                b"mdia",
                &[
                    mdhd(24000),
                    hdlr(b"tmcd"),
                    atoms(b"minf", &[stbl(tmcd_entry(), (1, 3003), &[4], tc_at)]),
                ],
            ),
        ],
    );
    let udta = atoms(
        b"udta",
        &[
            udta_text(b"\xA9mak", "Acme"),
            udta_text(b"\xA9mod", "Cam 1"),
            udta_text(b"\xA9day", "2024-05-06T07:08:09+0200"),
        ],
    );
    let mut mvhd = be(&[0, 3_000_000_000, 0, 600, 0]);
    mvhd.resize(100, 0);
    let moov = atoms(b"moov", &[atom(b"mvhd", &mvhd), video, tc, udta]);
    [ftyp, atom(b"mdat", &mdat_body), moov].concat()
}

fn probe(bytes: &[u8]) -> Result<crate::types::Probe, FormatError> {
    let src: &dyn Source = &bytes;
    probe_source(src)
}

#[test]
fn built_movie_reads_completely() {
    let file = movie();
    let p = probe(&file).unwrap();
    assert_eq!(p.codec, Codec::ProRes(ProResProfile::Hq));
    assert_eq!(
        p.meta.frame_rate,
        Some(Rational {
            num: 24000,
            den: 1001
        })
    );
    assert_eq!(p.meta.frame_count, Some(3));
    assert_eq!((p.meta.width, p.meta.height), (Some(1920), Some(1080)));
    assert_eq!(
        p.meta.start_timecode.map(|t| t.to_string()).as_deref(),
        Some("01:00:00:05")
    );
    assert_eq!(p.meta.embedded_reel.as_deref(), Some("A001"));
    assert_eq!(p.meta.camera_make.as_deref(), Some("Acme"));
    assert_eq!(p.meta.camera_model.as_deref(), Some("Cam 1"));
    assert_eq!(
        p.meta.recording_date.as_deref(),
        Some("2024-05-06 07:08:09")
    );
    assert_eq!(p.meta.colour.transfer, Some(Transfer::Rec709));
    assert_eq!(p.meta.colour.gamut, Some(Gamut::Rec709));
    assert_eq!(p.layout.pixel_aspect, Some(Rational { num: 1, den: 1 }));
    assert_eq!(
        p.frames,
        FrameIndex::Ranges(vec![
            ByteRange {
                offset: 28,
                len: 100
            },
            ByteRange {
                offset: 128,
                len: 120
            },
            ByteRange {
                offset: 248,
                len: 90
            },
        ])
    );
    assert!(p.warnings.is_empty(), "{:?}", p.warnings);
    // The frame bytes really are where the index says.
    let FrameIndex::Ranges(r) = &p.frames else {
        unreachable!()
    };
    assert_eq!(file[r[1].offset as usize], 100);
}

#[test]
fn mvhd_date_when_nothing_else_says() {
    let mut file = movie();
    let at = file.windows(4).position(|w| w == b"\xA9day").unwrap();
    file[at..at + 4].copy_from_slice(b"\xA9xyz");
    let p = probe(&file).unwrap();
    // 3_000_000_000 s after 1904-01-01
    assert_eq!(
        p.meta.recording_date.as_deref(),
        Some("1999-01-24 05:20:00")
    );
}

#[test]
fn frames_past_the_end_are_dropped_with_a_warning() {
    let mut file = movie();
    let sizes = be(&FRAME_SIZES);
    let at = file
        .windows(12)
        .position(|w| w == sizes.as_slice())
        .unwrap();
    file[at + 8..at + 12].copy_from_slice(&1_000_000u32.to_be_bytes());
    let p = probe(&file).unwrap();
    let FrameIndex::Ranges(r) = &p.frames else {
        unreachable!()
    };
    assert_eq!(r.len(), 2);
    assert_eq!(p.meta.frame_count, Some(3));
    assert!(p.warnings.iter().any(|w| w.contains("1 of 3 frames")));
}

#[test]
fn moov_cut_off_is_corrupt() {
    let file = movie();
    assert!(matches!(
        probe(&file[..file.len() - 10]),
        Err(FormatError::Corrupt(_))
    ));
}

#[test]
fn no_moov_is_corrupt_and_crm_is_unsupported() {
    let only_mdat = atom(b"mdat", &[0; 16]);
    assert!(matches!(probe(&only_mdat), Err(FormatError::Corrupt(_))));
    let mut crm = atom(b"ftyp", b"crx \0\0\0\x01crx isom");
    crm.extend(atom(b"moov", &[]));
    assert!(matches!(probe(&crm), Err(FormatError::Unsupported(_))));
}

mod fixtures {
    use super::*;

    pub(super) const ALL: [(&str, &[u8]); 6] = [
        (
            "resolve_prores4444",
            include_bytes!("../../tests/fixtures/qt/resolve_prores4444.moov"),
        ),
        (
            "arri_alexa_mini",
            include_bytes!("../../tests/fixtures/qt/arri_alexa_mini.moov"),
        ),
        (
            "ronin4d_prores4444",
            include_bytes!("../../tests/fixtures/qt/ronin4d_prores4444.moov"),
        ),
        (
            "lumix_s5",
            include_bytes!("../../tests/fixtures/qt/lumix_s5.moov"),
        ),
        (
            "canon_c50",
            include_bytes!("../../tests/fixtures/qt/canon_c50.moov"),
        ),
        (
            "ronin4d_proresraw",
            include_bytes!("../../tests/fixtures/qt/ronin4d_proresraw.moov"),
        ),
    ];

    fn get(name: &str) -> crate::types::Probe {
        let bytes = ALL.iter().find(|(n, _)| *n == name).unwrap().1;
        probe(bytes).unwrap()
    }

    /// moov-only: the frames and the timecode sample lie outside the file.
    fn assert_moov_only(p: &crate::types::Probe) {
        assert_eq!(p.frames, FrameIndex::Ranges(Vec::new()));
        assert_eq!(p.meta.start_timecode, None);
        assert!(p.warnings.iter().any(|w| w.contains("outside the file")));
    }

    #[test]
    fn arri() {
        let p = get("arri_alexa_mini");
        assert_moov_only(&p);
        assert_eq!(p.codec, Codec::ProRes(ProResProfile::P4444Xq));
        let m = &p.meta;
        assert_eq!(m.camera_make.as_deref(), Some("ARRI"));
        assert_eq!(m.camera_model.as_deref(), Some("ARRI ALEXA Mini"));
        assert_eq!(m.camera_serial.as_deref(), Some("20017"));
        assert_eq!(m.lens.as_deref(), Some("Fujinon Alura AZ30-80 T2.8"));
        assert_eq!(m.iso.as_deref(), Some("800"));
        assert_eq!(m.shutter.as_deref(), Some("172.8°"));
        assert_eq!(m.white_balance.as_deref(), Some("5600 K"));
        assert_eq!(m.nd.as_deref(), Some("1.2"));
        assert_eq!(m.embedded_reel.as_deref(), Some("M001R00H"));
        assert_eq!(m.embedded_camera_id.as_deref(), Some("M"));
        assert_eq!(m.recording_date.as_deref(), Some("2016-12-07 15:13:41"));
        assert_eq!(m.frame_count, Some(253));
        assert_eq!(m.frame_rate, Some(Rational { num: 24, den: 1 }));
        assert_eq!((m.width, m.height), (Some(3840), Some(2160)));
        assert_eq!(m.colour.transfer, Some(Transfer::LogC3));
        assert_eq!(m.colour.gamut, Some(Gamut::ArriWideGamut3));
    }

    #[test]
    fn resolve_export() {
        let p = get("resolve_prores4444");
        assert_moov_only(&p);
        assert_eq!(p.codec, Codec::ProRes(ProResProfile::P4444));
        let m = &p.meta;
        assert_eq!(m.camera_make, None);
        assert_eq!(m.camera_model, None);
        assert_eq!(m.recording_date.as_deref(), Some("2024-09-19 20:23:02"));
        assert_eq!(m.frame_count, Some(560));
        assert_eq!(
            m.frame_rate,
            Some(Rational {
                num: 24000,
                den: 1001
            })
        );
        // The file's own colr says Rec.709, whatever its name says.
        assert_eq!(m.colour.transfer, Some(Transfer::Rec709));
    }

    #[test]
    fn lumix() {
        let p = get("lumix_s5");
        assert_moov_only(&p);
        assert_eq!(p.codec, Codec::H264);
        let m = &p.meta;
        assert_eq!(m.camera_make.as_deref(), Some("Panasonic"));
        assert_eq!(m.camera_model.as_deref(), Some("DC-S5"));
        assert_eq!(m.camera_serial.as_deref(), Some("WJ0LA001863"));
        assert_eq!(m.iso.as_deref(), Some("640"));
        assert_eq!(m.shutter.as_deref(), Some("1/60"));
        assert_eq!(m.stop, None);
        assert_eq!(m.recording_date.as_deref(), Some("2022-04-19 22:47:20"));
        assert_eq!(m.colour.transfer, Some(Transfer::VLog));
        assert_eq!(m.colour.gamut, Some(Gamut::VGamut));
    }

    #[test]
    fn canon() {
        let p = get("canon_c50");
        assert_moov_only(&p);
        assert_eq!(p.codec, Codec::Hevc);
        let m = &p.meta;
        assert_eq!(m.camera_make.as_deref(), Some("Canon"));
        assert_eq!(m.camera_model.as_deref(), Some("Canon EOS C50"));
        assert_eq!(m.lens.as_deref(), Some("RF24-105mm F2.8 L IS USM Z"));
        assert_eq!(m.focal_length.as_deref(), Some("24 mm"));
        assert_eq!(m.stop.as_deref(), Some("f/2.8"));
        assert_eq!(m.iso.as_deref(), Some("800"));
        assert_eq!(m.shutter.as_deref(), Some("1/50"));
        assert_eq!(m.recording_date.as_deref(), Some("2025-01-08 06:12:43"));
        // White balance comes from the cndm sample, which is not in a moov-only file.
        assert_eq!(m.white_balance, None);
        assert!(!p.warnings.iter().any(|w| w.contains("edit list")));
    }

    #[test]
    fn ronin_4d() {
        let p = get("ronin4d_prores4444");
        assert_moov_only(&p);
        assert_eq!(p.meta.camera_make, None);
        assert_eq!(p.meta.frame_rate, Some(Rational { num: 25, den: 1 }));
        let p = get("ronin4d_proresraw");
        assert_moov_only(&p);
        assert_eq!(p.codec, Codec::ProResRaw);
        assert_eq!(p.meta.frame_rate, Some(Rational { num: 60, den: 1 }));
        assert_eq!((p.meta.width, p.meta.height), (Some(8192), Some(4320)));
        // The DJI metadata sample is in mdat, which a moov-only file lacks.
        assert_eq!(p.meta.camera_model, None);
        assert!(p.warnings.iter().any(|w| w.contains("djmd")));
    }
}

/// Deterministic xorshift64*.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    fn below(&mut self, n: usize) -> usize {
        (self.next() % n.max(1) as u64) as usize
    }
}

/// Truncations and byte flips must give `Ok` or `Err`, never a panic, a hang,
/// or a runaway allocation.
#[test]
fn corruption_harness() {
    let built = movie();
    let mut inputs: Vec<(&str, &[u8])> = fixtures::ALL.to_vec();
    inputs.push(("built", &built));
    let mut rng = Rng(0x9E37_79B9_7F4A_7C15);
    for (name, original) in inputs {
        assert!(probe(original).is_ok(), "{name} must parse unmodified");
        for _ in 0..200 {
            let cut = rng.below(original.len());
            let _ = probe(&original[..cut]);
        }
        for _ in 0..400 {
            let mut b = original.to_vec();
            for _ in 0..1 + rng.below(8) {
                let i = rng.below(b.len());
                b[i] = match rng.below(4) {
                    0 => 0,
                    1 => 0xFF,
                    _ => rng.next() as u8,
                };
            }
            let _ = probe(&b);
        }
        // Flips concentrated in atom headers, where sizes and counts live.
        let headers: Vec<usize> = original
            .windows(4)
            .enumerate()
            .filter(|(_, w)| w.iter().all(|c| c.is_ascii_alphanumeric() || *c == b' '))
            .map(|(i, _)| i.saturating_sub(4))
            .collect();
        for _ in 0..300 {
            let mut b = original.to_vec();
            let at = headers[rng.below(headers.len())];
            for k in 0..4 {
                if let Some(x) = b.get_mut(at + k) {
                    *x = rng.next() as u8;
                }
            }
            let _ = probe(&b);
        }
    }
}

#[test]
fn one_tick_jitter_is_a_steady_rate() {
    use super::steady_delta;
    // iPhone 25p at timescale 600: one frame a tick long.
    assert_eq!(steady_delta(&[(231, 24), (1, 25), (122, 24)]), Some(24));
    // A tick added every 100 frames, and a short frame paired with a long one.
    let periodic = [
        (50, 24),
        (1, 25),
        (99, 24),
        (1, 25),
        (99, 24),
        (1, 25),
        (99, 24),
        (1, 25),
        (99, 24),
        (1, 25),
        (19, 24),
    ];
    assert_eq!(steady_delta(&periodic), Some(24));
    assert_eq!(
        steady_delta(&[(228, 24), (1, 23), (1, 25), (60, 24)]),
        Some(24)
    );
    // An odd-length last sample, as before.
    assert_eq!(steady_delta(&[(100, 24), (1, 7)]), Some(24));
    // Real changes of rate stay variable.
    assert_eq!(steady_delta(&[(100, 24), (100, 30)]), None);
    assert_eq!(steady_delta(&[(50, 24), (50, 25)]), None);
    // A tick is too coarse to ignore when frames are only a few ticks long.
    assert_eq!(steady_delta(&[(10, 1), (1, 2), (10, 1)]), None);
}

#[test]
fn stts_summary_names_the_irregular_samples() {
    let s = super::stts_summary(&[(100, 1001), (1, 2002), (50, 1001)], 24000);
    assert!(s.contains("min=41.708ms"), "{s}");
    assert!(s.contains("max=83.417ms"), "{s}");
    assert!(s.contains("changes at 100, 101"), "{s}");
    assert!(!s.contains("  "), "{s}");
}
