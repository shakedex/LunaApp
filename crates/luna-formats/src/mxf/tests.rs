//! Tests over synthetic MXF built in memory, plus a corruption harness.

use std::io::Cursor;

use super::klv::{FILL, INDEX_SEGMENT, PRIMER, RIP, Ul, ul};
use super::probe_source;
use crate::types::{ByteRange, Codec, FrameIndex, ProResProfile, Rational, SidecarKind};

const PICTURE_KEY: Ul = ul("060e2b34010201010d01030115011700");
const SOUND_KEY: Ul = ul("060e2b34010201010d01030116020300");
const ANC_KEY: Ul = ul("060e2b34010201010d01030117010201");

fn klv(key: Ul, v: &[u8]) -> Vec<u8> {
    let mut o = key.0.to_vec();
    o.push(0x84);
    o.extend((v.len() as u32).to_be_bytes());
    o.extend(v);
    o
}

fn set(key: &str, items: &[(u16, Vec<u8>)]) -> Vec<u8> {
    let mut v = Vec::new();
    for (tag, val) in items {
        v.extend(tag.to_be_bytes());
        v.extend((val.len() as u16).to_be_bytes());
        v.extend(val);
    }
    klv(ul(key), &v)
}

fn uid(n: u8) -> Vec<u8> {
    let mut u = vec![0xa0; 16];
    u[15] = n;
    u
}

fn umid(n: u8) -> Vec<u8> {
    let mut u = vec![0x06, 0x0a, 0x2b, 0x34, 0x01, 0x01, 0x01, 0x05];
    u.extend([0x01, 0x01, 0x0d, 0x43, 0x13, 0x00, 0x00, 0x00]);
    u.extend(vec![n; 16]);
    u
}

fn batch(uids: &[Vec<u8>]) -> Vec<u8> {
    let mut v = (uids.len() as u32).to_be_bytes().to_vec();
    v.extend(16u32.to_be_bytes());
    for u in uids {
        v.extend(u);
    }
    v
}

fn utf16(s: &str) -> Vec<u8> {
    s.encode_utf16().flat_map(u16::to_be_bytes).collect()
}

fn be32(x: u32) -> Vec<u8> {
    x.to_be_bytes().to_vec()
}

fn be64(x: i64) -> Vec<u8> {
    x.to_be_bytes().to_vec()
}

fn rational(n: i32, d: i32) -> Vec<u8> {
    let mut v = n.to_be_bytes().to_vec();
    v.extend(d.to_be_bytes());
    v
}

fn ul_bytes(hex: &str) -> Vec<u8> {
    ul(hex).0.to_vec()
}

#[allow(clippy::too_many_arguments)]
fn partition(
    kind: u8,
    status: u8,
    this: u64,
    prev: u64,
    footer: u64,
    hb: u64,
    ibc: u64,
    isid: u32,
    body_offset: u64,
    bsid: u32,
) -> Vec<u8> {
    let mut v = Vec::new();
    v.extend(1u16.to_be_bytes());
    v.extend(3u16.to_be_bytes());
    v.extend(1u32.to_be_bytes());
    for x in [this, prev, footer, hb, ibc] {
        v.extend(x.to_be_bytes());
    }
    v.extend(isid.to_be_bytes());
    v.extend(body_offset.to_be_bytes());
    v.extend(bsid.to_be_bytes());
    v.extend(ul_bytes("060e2b34040101010d01020101010900"));
    v.extend(0u32.to_be_bytes());
    v.extend(16u32.to_be_bytes());
    let mut key = ul("060e2b34020501010d01020101020400");
    key.0[13] = kind;
    key.0[14] = status;
    klv(key, &v)
}

/// The header metadata of a small ProRes 4444 clip.
fn header_metadata(frames: i64) -> Vec<u8> {
    let mut primer = Vec::new();
    let entries: &[(u16, &str)] = &[
        (0x3c01, "060e2b34010101020520070102010000"),
        (0x3c02, "060e2b34010101020520070103010000"),
        (0x8001, "060e2b34010101020401060100000000"), // PictureEssenceCoding under a dynamic tag
    ];
    primer.extend((entries.len() as u32).to_be_bytes());
    primer.extend(18u32.to_be_bytes());
    for (tag, hex) in entries {
        primer.extend(tag.to_be_bytes());
        primer.extend(ul_bytes(hex));
    }
    let mut m = klv(PRIMER, &primer);
    // Identification
    m.extend(set(
        "060e2b34025301010d01010101013000",
        &[
            (0x3c0a, uid(1)),
            (0x3c01, utf16("TestCo")),
            (0x3c02, utf16("Cam X")),
            (0x3c06, vec![0x07, 0xe9, 3, 4, 5, 6, 7, 0]),
        ],
    ));
    // Material package: timecode track + picture track.
    m.extend(set(
        "060e2b34025301010d01010101013600",
        &[
            (0x3c0a, uid(2)),
            (0x4401, umid(1)),
            (0x4405, vec![0x07, 0xe9, 3, 4, 10, 11, 12, 0]),
            (0x4403, batch(&[uid(3), uid(6)])),
        ],
    ));
    // Timecode track → sequence → timecode component.
    m.extend(set(
        "060e2b34025301010d01010101013b00",
        &[
            (0x3c0a, uid(3)),
            (0x4801, be32(1)),
            (0x4b01, rational(24, 1)),
            (0x4803, uid(4)),
        ],
    ));
    m.extend(set(
        "060e2b34025301010d01010101010f00",
        &[
            (0x3c0a, uid(4)),
            (0x0201, ul_bytes("060e2b34040101010103020101000000")),
            (0x0202, be64(frames)),
            (0x1001, batch(&[uid(5)])),
        ],
    ));
    m.extend(set(
        "060e2b34025301010d01010101011400",
        &[
            (0x3c0a, uid(5)),
            (0x1501, be64(1000)),
            (0x1502, vec![0, 24]),
            (0x1503, vec![0]),
        ],
    ));
    // Picture track → sequence → source clip into the file package.
    m.extend(set(
        "060e2b34025301010d01010101013b00",
        &[
            (0x3c0a, uid(6)),
            (0x4801, be32(2)),
            (0x4b01, rational(24, 1)),
            (0x4803, uid(7)),
        ],
    ));
    m.extend(set(
        "060e2b34025301010d01010101010f00",
        &[
            (0x3c0a, uid(7)),
            (0x0201, ul_bytes("060e2b34040101010103020201000000")),
            (0x0202, be64(frames)),
            (0x1001, batch(&[uid(8)])),
        ],
    ));
    m.extend(set(
        "060e2b34025301010d01010101011100",
        &[
            (0x3c0a, uid(8)),
            (0x0202, be64(frames)),
            (0x1101, umid(2)),
            (0x1102, be32(2)),
        ],
    ));
    // File package with one picture track and a CDCI descriptor.
    m.extend(set(
        "060e2b34025301010d01010101013700",
        &[
            (0x3c0a, uid(9)),
            (0x4401, umid(2)),
            (0x4403, batch(&[uid(10), uid(13)])),
            (0x4701, uid(12)),
        ],
    ));
    m.extend(set(
        "060e2b34025301010d01010101013b00",
        &[
            (0x3c0a, uid(10)),
            (0x4801, be32(2)),
            (0x4804, be32(0x15011700)),
            (0x4b01, rational(24, 1)),
            (0x4803, uid(11)),
        ],
    ));
    m.extend(set(
        "060e2b34025301010d01010101010f00",
        &[
            (0x3c0a, uid(11)),
            (0x0201, ul_bytes("060e2b34040101010103020201000000")),
            (0x0202, be64(frames)),
        ],
    ));
    m.extend(set(
        "060e2b34025301010d01010101013b00",
        &[
            (0x3c0a, uid(13)),
            (0x4801, be32(3)),
            (0x4804, be32(0x17010201)),
            (0x4b01, rational(24, 1)),
            (0x4803, uid(14)),
        ],
    ));
    m.extend(set(
        "060e2b34025301010d01010101010f00",
        &[
            (0x3c0a, uid(14)),
            (0x0201, ul_bytes("060e2b34040101010103020203000000")),
            (0x0202, be64(frames)),
        ],
    ));
    m.extend(set(
        "060e2b34025301010d01010101012800",
        &[
            (0x3c0a, uid(12)),
            (0x3006, be32(2)),
            (0x3001, rational(24, 1)),
            (0x3004, ul_bytes("060e2b34040101020d010301021c0100")),
            (0x3203, be32(64)),
            (0x3202, be32(36)),
            (0x3209, be32(64)),
            (0x3208, be32(32)),
            (0x320e, rational(4, 1)),
            (0x8001, ul_bytes("060e2b340401010d0401020203060500")),
            (0x3210, ul_bytes("060e2b34040101010401010101020000")),
        ],
    ));
    m.extend(set(
        "060e2b34025301010d01010101012300",
        &[
            (0x3c0a, uid(15)),
            (0x2701, umid(2)),
            (0x3f06, be32(2)),
            (0x3f07, be32(1)),
        ],
    ));
    m
}

/// RDD 18 camera-unit set in one ANC packet: WB 3200 K, EI 1600.
fn anc_value() -> Vec<u8> {
    let items = [
        0x81, 0x0e, 0x00, 0x02, 0x0c, 0x80, 0x81, 0x15, 0x00, 0x02, 0x06, 0x40,
    ];
    let mut payload = ul_bytes("060e2b34025301010c02010102010000");
    payload.push(items.len() as u8);
    payload.extend(items);
    let mut arr = vec![0x43, 0x05, (payload.len() + 1) as u8, 1];
    arr.extend(&payload);
    let mut v = vec![0x00, 0x01, 0x00, 0x0b, 0x04, 0x04, 0x00, arr.len() as u8];
    v.extend((arr.len() as u32).to_be_bytes());
    v.extend(1u32.to_be_bytes());
    v.extend(arr);
    v
}

struct Built {
    bytes: Vec<u8>,
    frames: Vec<ByteRange>,
}

#[derive(Clone, Copy)]
struct Opts {
    frames: usize,
    index: bool,
    rip: bool,
    footer: bool,
    /// Constant-size frames with a CBE index.
    cbe: bool,
    /// Durations written in the header; a recording cut off by power loss has none.
    durations: bool,
}

impl Default for Opts {
    fn default() -> Self {
        Opts {
            frames: 5,
            index: true,
            rip: true,
            footer: true,
            cbe: false,
            durations: true,
        }
    }
}

fn build(o: Opts) -> Built {
    let md = header_metadata(if o.durations { o.frames as i64 } else { 0 });
    let header_pack = partition(0x02, 0x04, 0, 0, 0, md.len() as u64, 0, 0, 0, 0);
    let mut out = header_pack;
    out.extend(&md);

    // Body: one content package per frame (picture, sound, ANC on frame 0).
    let body_at = out.len() as u64;
    out.extend(partition(0x03, 0x04, body_at, 0, 0, 0, 0, 0, 0, 1));
    let essence_start = out.len() as u64;
    let mut frames = Vec::new();
    let mut units = Vec::new();
    for i in 0..o.frames {
        units.push(out.len() as u64 - essence_start);
        let size = if o.cbe { 100 } else { 100 + 37 * i };
        let payload: Vec<u8> = (0..size).map(|b| (b + i) as u8).collect();
        let at = out.len() as u64;
        out.extend(klv(PICTURE_KEY, &payload));
        frames.push(ByteRange {
            offset: at + 21,
            len: size as u64,
        });
        out.extend(klv(SOUND_KEY, &[0u8; 24]));
        let anc = if i == 0 || o.cbe {
            anc_value()
        } else {
            vec![0, 0]
        };
        let anc = if o.cbe {
            anc
        } else {
            let mut a = anc;
            a.resize(a.len() + i, 0);
            a
        };
        out.extend(klv(ANC_KEY, &anc));
    }
    let eubc = units.get(1).copied().unwrap_or(0);

    let mut index_at = 0;
    if o.index {
        index_at = out.len() as u64;
        let mut seg = Vec::new();
        seg.extend(0x3c0au16.to_be_bytes());
        seg.extend(16u16.to_be_bytes());
        seg.extend(uid(99));
        let mut push = |tag: u16, v: Vec<u8>| {
            seg.extend(tag.to_be_bytes());
            seg.extend((v.len() as u16).to_be_bytes());
            seg.extend(v);
        };
        push(0x3f0b, rational(24, 1));
        push(0x3f0c, be64(0));
        push(0x3f0d, be64(o.frames as i64));
        push(0x3f05, be32(if o.cbe { eubc as u32 } else { 0 }));
        push(0x3f06, be32(2));
        push(0x3f07, be32(1));
        if !o.cbe {
            let mut e = (o.frames as u32).to_be_bytes().to_vec();
            e.extend(11u32.to_be_bytes());
            for so in &units {
                e.extend([0u8, 0, 0x80]);
                e.extend(so.to_be_bytes());
            }
            push(0x3f0a, e);
        }
        let seg = klv(INDEX_SEGMENT, &seg);
        out.extend(partition(
            0x03,
            0x04,
            index_at,
            body_at,
            0,
            0,
            seg.len() as u64,
            2,
            0,
            0,
        ));
        out.extend(seg);
    }
    if o.footer {
        let footer_at = out.len() as u64;
        let prev = if o.index { index_at } else { body_at };
        out.extend(partition(
            0x04, 0x04, footer_at, prev, footer_at, 0, 0, 0, 0, 0,
        ));
        // Point the header at the footer.
        let mut h = partition(0x02, 0x04, 0, 0, footer_at, md.len() as u64, 0, 0, 0, 0);
        h.truncate(out.len().min(h.len()));
        out[..h.len()].copy_from_slice(&h);
        if o.rip {
            let mut entries = Vec::new();
            for (sid, at) in [(0u32, 0u64), (1, body_at), (0, index_at), (0, footer_at)] {
                if at == 0 && sid == 0 && entries.len() > 1 {
                    continue;
                }
                entries.extend(sid.to_be_bytes());
                entries.extend(at.to_be_bytes());
            }
            let total = 16 + 5 + entries.len() + 4;
            entries.extend((total as u32).to_be_bytes());
            out.extend(klv(RIP, &entries));
        }
    }
    Built { bytes: out, frames }
}

fn run(bytes: &[u8]) -> Result<crate::types::Probe, crate::types::FormatError> {
    probe_source(Cursor::new(bytes.to_vec()), &[])
}

#[test]
fn reads_structure_and_frames_through_the_index() {
    let b = build(Opts::default());
    let p = run(&b.bytes).unwrap();
    assert_eq!(p.codec, Codec::ProRes(ProResProfile::P4444));
    assert_eq!(p.meta.frame_rate, Some(Rational { num: 24, den: 1 }));
    assert_eq!(p.meta.frame_count, Some(5));
    assert_eq!(p.meta.start_timecode.unwrap().to_string(), "00:00:41:16");
    assert_eq!((p.meta.width, p.meta.height), (Some(64), Some(32)));
    assert_eq!(p.meta.camera_make.as_deref(), Some("TestCo"));
    assert_eq!(p.meta.camera_model.as_deref(), Some("Cam X"));
    assert_eq!(
        p.meta.recording_date.as_deref(),
        Some("2025-03-04 10:11:12")
    );
    assert_eq!(p.meta.colour.transfer, Some(crate::types::Transfer::Rec709));
    assert_eq!(p.meta.white_balance.as_deref(), Some("3200 K"));
    assert_eq!(p.meta.iso.as_deref(), Some("1600"));
    assert_eq!((p.layout.stored_width, p.layout.stored_height), (64, 36));
    assert_eq!(p.layout.pixel_aspect, Some(Rational { num: 2, den: 1 }));
    assert_eq!(p.frames, FrameIndex::Ranges(b.frames));
    assert!(p.warnings.is_empty(), "{:?}", p.warnings);
}

#[test]
fn scans_when_there_is_no_index() {
    let b = build(Opts {
        index: false,
        ..Default::default()
    });
    let p = run(&b.bytes).unwrap();
    assert_eq!(p.frames, FrameIndex::Ranges(b.frames));
}

#[test]
fn follows_the_footer_chain_without_a_rip() {
    let b = build(Opts {
        rip: false,
        ..Default::default()
    });
    let p = run(&b.bytes).unwrap();
    assert_eq!(p.frames, FrameIndex::Ranges(b.frames));
    assert!(p.warnings.is_empty(), "{:?}", p.warnings);
}

#[test]
fn constant_edit_units() {
    let b = build(Opts {
        cbe: true,
        ..Default::default()
    });
    let p = run(&b.bytes).unwrap();
    assert_eq!(p.frames, FrameIndex::Ranges(b.frames));
}

#[test]
fn a_cut_short_file_lists_the_complete_frames() {
    let b = build(Opts {
        index: false,
        footer: false,
        ..Default::default()
    });
    let last = b.frames[4];
    let cut = &b.bytes[..(last.offset + last.len / 2) as usize];
    let p = run(cut).unwrap();
    assert_eq!(p.frames, FrameIndex::Ranges(b.frames[..4].to_vec()));
    assert_eq!(p.meta.frame_count, Some(5));
    assert!(
        p.warnings.iter().any(|w| w.contains("4 of 5")),
        "{:?}",
        p.warnings
    );
}

#[test]
fn a_cut_off_recording_counts_its_complete_frames() {
    let b = build(Opts {
        index: false,
        footer: false,
        durations: false,
        ..Default::default()
    });
    let last = b.frames[4];
    let cut = &b.bytes[..(last.offset + last.len / 2) as usize];
    let p = run(cut).unwrap();
    assert_eq!(p.frames, FrameIndex::Ranges(b.frames[..4].to_vec()));
    assert_eq!(p.meta.frame_count, Some(4));
    assert!(
        p.warnings.iter().any(|w| w.contains("after 4 frames")),
        "{:?}",
        p.warnings
    );
}

#[test]
fn sidecar_values_fill_gaps() {
    let b = build(Opts::default());
    let xml = r#"<NonRealTimeMeta><Device manufacturer="Sony" modelName="MPC-2610" serialNo="42"/></NonRealTimeMeta>"#;
    let p = probe_source(
        Cursor::new(b.bytes),
        &[(SidecarKind::SonyXml, xml.to_string())],
    )
    .unwrap();
    assert_eq!(p.meta.camera_model.as_deref(), Some("BURANO"));
    assert_eq!(p.meta.camera_serial.as_deref(), Some("42"));
    let p = probe_source(
        Cursor::new(build(Opts::default()).bytes),
        &[(SidecarKind::SonyXml, "garbage".into())],
    )
    .unwrap();
    assert!(p.warnings.iter().any(|w| w.contains("Sony XML")));
}

#[test]
fn not_mxf_is_an_error() {
    assert!(run(&[]).is_err());
    assert!(run(&[0u8; 4096]).is_err());
    assert!(run(&klv(FILL, &[0; 64])).is_err());
}

/// Deterministic xorshift for the corruption harness.
struct XorShift(u64);

impl XorShift {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }

    fn below(&mut self, n: usize) -> usize {
        (self.next() % n.max(1) as u64) as usize
    }
}

/// Byte flips and truncations must never panic or hang; they may fail.
pub(crate) fn hammer(seed: u64, original: &[u8], rounds: usize) {
    let mut rng = XorShift(seed);
    for round in 0..rounds {
        let mut v = original.to_vec();
        match round % 3 {
            0 => {
                let n = 1 + rng.below(8);
                for _ in 0..n {
                    let i = rng.below(v.len());
                    v[i] ^= 1 << rng.below(8);
                }
            }
            1 => {
                let i = rng.below(v.len());
                v[i] = [0x00, 0xff, 0x80, 0x7f][rng.below(4)];
                let cut = rng.below(v.len()) + 1;
                if rng.below(2) == 0 {
                    v.truncate(cut);
                }
            }
            _ if v.len() > 9 => {
                // Hostile lengths: overwrite a BER length byte run.
                let i = rng.below(v.len() - 9);
                v[i] = 0x88;
                for b in &mut v[i + 1..i + 9] {
                    *b = 0xff;
                }
            }
            _ => {}
        }
        let _ = probe_source(Cursor::new(v), &[]);
    }
}

#[test]
fn corruption_harness_synthetic() {
    for (seed, o) in [
        (1, Opts::default()),
        (
            2,
            Opts {
                index: false,
                ..Default::default()
            },
        ),
        (
            3,
            Opts {
                cbe: true,
                ..Default::default()
            },
        ),
        (
            4,
            Opts {
                rip: false,
                ..Default::default()
            },
        ),
    ] {
        hammer(seed, &build(o).bytes, 1500);
    }
}

#[test]
fn every_truncation_of_the_synthetic_clip() {
    let b = build(Opts::default()).bytes;
    for n in 0..b.len() {
        let _ = run(&b[..n]);
    }
}

macro_rules! fixture {
    ($name:literal) => {
        include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/mxf/",
            $name
        ))
        .as_slice()
    };
}

const HEADERS: &[&[u8]] = &[
    fixture!("alexa35_prores_header.bin"),
    fixture!("alexa35_arriraw_header.bin"),
    fixture!("minilf_prores_header.bin"),
    fixture!("burano_xocn_header.bin"),
    fixture!("venice2_xocn_header.bin"),
    fixture!("fx6_xavc_header.bin"),
    fixture!("c50_xfavc_header.bin"),
];

#[test]
fn corruption_harness_real_headers() {
    for (i, h) in HEADERS.iter().enumerate() {
        hammer(100 + i as u64, h, 600);
    }
}

#[test]
fn rdd18_from_real_anc() {
    use super::rdd18::{anc_payload, parse};
    let v2 = parse(&anc_payload(fixture!("venice2_anc.bin")));
    assert_eq!(
        v2.camera_attributes.as_deref(),
        Some("MPC-3626 0010119 Version3.00")
    );
    assert_eq!(v2.exposure_index, Some(1250));
    assert_eq!(v2.iso, Some(800));
    assert_eq!(v2.white_balance, Some(3600));
    assert_eq!(v2.nd_setting, Some(2));
    assert_eq!(v2.shutter_angle, Some(180.0));
    assert_eq!(v2.lens_attributes.as_deref(), Some("7085.2501"));
    assert!((v2.focal_mm.unwrap() - 85.0).abs() < 1e-9);
    assert!((v2.iris_t.unwrap() - 2.83).abs() < 0.01);

    // BURANO: the same values its XML sidecar states.
    let b = parse(&anc_payload(fixture!("burano_anc.bin")));
    assert!((b.iris_f.unwrap() - 1.84).abs() < 0.01);
    assert!((b.iris_t.unwrap() - 1.92).abs() < 0.01);
    assert!((b.focal_mm.unwrap() - 50.0).abs() < 1e-9);
    assert_eq!(b.nd_setting, Some(4));
    assert_eq!(b.white_balance, Some(3600));

    let fx6 = parse(&anc_payload(fixture!("fx6_anc.bin")));
    assert_eq!(
        fx6.camera_attributes.as_deref(),
        Some("ILME-FX6V 4000254 Version1.00")
    );
    assert_eq!(fx6.exposure_index, Some(12800));
    assert_eq!(fx6.lens_attributes.as_deref(), Some("Unknown"));

    let c50 = parse(&anc_payload(fixture!("c50_anc.bin")));
    assert_eq!(c50.vendor_model.as_deref(), Some("Canon EOS C50"));
    assert_eq!(c50.vendor_serial.as_deref(), Some("123456789012"));
    assert_eq!(c50.vendor_date, Some((2025, 1, 7, 22, 40, 21)));
    assert_eq!(c50.capture_fps, Some((25000, 1000)));
}

#[test]
fn arri_frame_meta_from_real_element() {
    let f = super::arri::frame_meta(fixture!("alexa35_frame_meta.bin"));
    assert_eq!(f.exposure_index, Some(800));
    assert_eq!(f.white_balance, Some(5600));
    assert_eq!(f.exposure_time, Some((1, 50)));
    assert_eq!(f.sensor_fps, Some((24000, 1000)));
    assert_eq!(f.focal_um, Some(44978));
    assert_eq!(f.iris_t1000, Some(5190));
    assert_eq!(f.focus_mm, Some(14471));
    assert_eq!(f.nd, Some(1.2));
    assert_eq!(f.tint, Some(0.0));
}

#[test]
fn arri_legacy_header_from_mini_lf() {
    let slice = fixture!("minilf_prores_header.bin");
    let at = slice
        .windows(8)
        .position(|w| w == [b'A', b'R', b'R', b'I', 0x12, 0x34, 0x56, 0x78])
        .unwrap();
    let l = super::arri::parse_legacy(&slice[at..]);
    assert_eq!(l.camera_model.as_deref(), Some("ARRI ALEXA Mini LF"));
    assert_eq!(l.lens_model.as_deref(), Some("Zeiss CZ.2 70-200 01"));
    assert_eq!(l.exposure_index, Some(800));
    assert_eq!(l.white_balance, Some(5600));
    assert_eq!(l.exposure_us, Some(20000));
    assert_eq!(l.shutter_angle_1000, Some(172_800));
    assert_eq!(l.sensor_fps_1000, Some(24_000));
    assert_eq!(l.focal_um, Some(200_000));
    assert_eq!(l.camera_index.as_deref(), Some("F"));
    assert_eq!(l.reel_name.as_deref(), Some("F003MN99"));
    assert_eq!(l.clip_name.as_deref(), Some("F003C004_190925_MN99"));
}
