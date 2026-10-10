//! Committed fixtures (tiny x264 streams + ffmpeg reference planes) and a
//! hand-built I_PCM stream, so the decoder is checked without ffmpeg.

mod common;

use luna_h264::{DecoderConfig, Error, FrameKind, access_unit_kind, decode_access_unit};

/// (name, stream, reference planes, width, height)
type Fixture = (&'static str, &'static [u8], &'static [u8], u32, u32);

const FIXTURES: &[Fixture] = &[
    (
        "cavlc422p10 (2 slices, JVT matrices)",
        include_bytes!("fixtures/cavlc422p10.h264"),
        include_bytes!("fixtures/cavlc422p10.yuv"),
        96,
        64,
    ),
    (
        "cabac422p10 (2 slices)",
        include_bytes!("fixtures/cabac422p10.h264"),
        include_bytes!("fixtures/cabac422p10.yuv"),
        96,
        64,
    ),
    (
        "cabac420 (cropped)",
        include_bytes!("fixtures/cabac420.h264"),
        include_bytes!("fixtures/cabac420.yuv"),
        104,
        56,
    ),
];

fn samples(raw: &[u8], wide: bool) -> Vec<u16> {
    if wide {
        raw.as_chunks::<2>()
            .0
            .iter()
            .map(|&c| u16::from_le_bytes(c))
            .collect()
    } else {
        raw.iter().map(|&b| b as u16).collect()
    }
}

#[test]
fn fixtures_are_bit_exact() {
    for &(name, stream, yuv, w, h) in FIXTURES {
        let cfg = DecoderConfig::annex_b();
        assert_eq!(access_unit_kind(stream, &cfg), FrameKind::Idr, "{name}");
        let pic = decode_access_unit(stream, &cfg).unwrap_or_else(|e| panic!("{name}: {e}"));
        assert_eq!((pic.width, pic.height), (w, h), "{name}");
        let want = samples(yuv, pic.bit_depth > 8);
        let (bad, first) = common::compare(&pic, &want);
        assert_eq!(bad, [0; 3], "{name}: {first:?}");
    }
}

/// Annex B NAL payloads (with header byte) of a stream.
fn nal_units(stream: &[u8]) -> Vec<&[u8]> {
    let mut starts = Vec::new();
    let mut i = 0;
    while i + 3 <= stream.len() {
        if stream[i..i + 3] == [0, 0, 1] {
            starts.push(i + 3);
            i += 3;
        } else {
            i += 1;
        }
    }
    starts
        .iter()
        .enumerate()
        .map(|(k, &s)| {
            let mut e = starts.get(k + 1).map_or(stream.len(), |&n| n - 3);
            while e > s && stream[e - 1] == 0 {
                e -= 1;
            }
            &stream[s..e]
        })
        .collect()
}

#[test]
fn avcc_framing_decodes_the_same() {
    let (_, stream, yuv, _, _) = FIXTURES[1];
    let nals = nal_units(stream);
    let sps: Vec<&[u8]> = nals.iter().copied().filter(|n| n[0] & 0x1f == 7).collect();
    let pps: Vec<&[u8]> = nals.iter().copied().filter(|n| n[0] & 0x1f == 8).collect();
    let mut rec = vec![
        1,
        sps[0][1],
        sps[0][2],
        sps[0][3],
        0xff,
        0xe0 | sps.len() as u8,
    ];
    for s in &sps {
        rec.extend_from_slice(&(s.len() as u16).to_be_bytes());
        rec.extend_from_slice(s);
    }
    rec.push(pps.len() as u8);
    for p in &pps {
        rec.extend_from_slice(&(p.len() as u16).to_be_bytes());
        rec.extend_from_slice(p);
    }
    let mut sample = Vec::new();
    for n in nals.iter().filter(|n| matches!(n[0] & 0x1f, 1 | 5 | 6)) {
        sample.extend_from_slice(&(n.len() as u32).to_be_bytes());
        sample.extend_from_slice(n);
    }
    let cfg = DecoderConfig::from_avcc(&rec).unwrap();
    assert_eq!(access_unit_kind(&sample, &cfg), FrameKind::Idr);
    let pic = decode_access_unit(&sample, &cfg).unwrap();
    let (bad, _) = common::compare(&pic, &samples(yuv, true));
    assert_eq!(bad, [0; 3]);
    // Without its parameter sets the sample cannot be decoded.
    assert!(matches!(
        decode_access_unit(
            &sample,
            &DecoderConfig::from_avcc(&[1, 0, 0, 0, 0xff, 0xe0, 0]).unwrap()
        ),
        Err(Error::Corrupt(_))
    ));
}

/// MSB-first bit writer for building streams by hand.
#[derive(Default)]
struct Bits {
    bytes: Vec<u8>,
    n: usize,
}

impl Bits {
    fn u(&mut self, bits: u32, v: u32) {
        for k in (0..bits).rev() {
            if self.n.is_multiple_of(8) {
                self.bytes.push(0);
            }
            if (v >> k) & 1 == 1 {
                *self.bytes.last_mut().unwrap() |= 0x80 >> (self.n % 8);
            }
            self.n += 1;
        }
    }
    fn ue(&mut self, v: u32) {
        let x = v + 1;
        let len = 32 - x.leading_zeros();
        self.u(len - 1, 0);
        self.u(len, x);
    }
    fn se(&mut self, v: i32) {
        self.ue(if v > 0 {
            2 * v as u32 - 1
        } else {
            (-2 * v) as u32
        });
    }
    fn align_zero(&mut self) {
        while !self.n.is_multiple_of(8) {
            self.u(1, 0);
        }
    }
    fn trailing(&mut self) {
        self.u(1, 1);
        self.align_zero();
    }
    /// As an Annex B NAL unit with emulation prevention.
    fn nal(&self, header: u8, out: &mut Vec<u8>) {
        out.extend_from_slice(&[0, 0, 0, 1, header]);
        let mut zeros = 0;
        for &b in &self.bytes {
            if zeros >= 2 && b <= 3 {
                out.push(3);
                zeros = 0;
            }
            out.push(b);
            zeros = if b == 0 { zeros + 1 } else { 0 };
        }
    }
}

/// A 32×16 4:2:0 8-bit IDR picture: an I_PCM macroblock, then an
/// Intra_16x16 DC macroblock predicted from it (CAVLC, no deblocking).
fn pcm_stream(pcm: &[u8; 384], deblock: bool) -> Vec<u8> {
    let mut out = Vec::new();
    let mut s = Bits::default();
    s.u(8, 100); // High
    s.u(8, 0);
    s.u(8, 40);
    s.ue(0); // sps id
    s.ue(1); // 4:2:0
    s.ue(0);
    s.ue(0);
    s.u(1, 0);
    s.u(1, 0); // no matrices
    s.ue(0); // log2_max_frame_num - 4
    s.ue(2); // poc type 2
    s.ue(0); // max refs
    s.u(1, 0);
    s.ue(1); // width 2 MBs
    s.ue(0); // height 1 MB
    s.u(1, 1); // frame_mbs_only
    s.u(1, 1);
    s.u(1, 0); // no cropping
    s.u(1, 0); // no VUI
    s.trailing();
    s.nal(0x67, &mut out);
    let mut p = Bits::default();
    p.ue(0);
    p.ue(0);
    p.u(1, 0); // CAVLC
    p.u(1, 0);
    p.ue(0);
    p.ue(0);
    p.ue(0);
    p.u(1, 0);
    p.u(2, 0);
    p.se(0); // pic_init_qp 26
    p.se(0);
    p.se(0);
    p.u(1, 1); // deblocking_filter_control_present
    p.u(1, 0);
    p.u(1, 0);
    p.trailing();
    p.nal(0x68, &mut out);
    let mut sl = Bits::default();
    sl.ue(0); // first_mb
    sl.ue(7); // I
    sl.ue(0);
    sl.u(4, 0); // frame_num
    sl.ue(0); // idr_pic_id
    sl.u(1, 0);
    sl.u(1, 0); // dec_ref_pic_marking
    sl.se(0); // slice_qp_delta
    if deblock {
        sl.ue(0); // disable_deblocking_filter_idc: on
        sl.se(2); // alpha offset (div2)
        sl.se(1); // beta offset (div2)
    } else {
        sl.ue(1); // off
    }
    sl.ue(25); // I_PCM
    sl.align_zero();
    for &b in pcm {
        sl.u(8, b as u32);
    }
    sl.ue(3); // I_16x16_2_0_0: DC prediction, no coded blocks
    sl.ue(0); // chroma DC
    sl.se(0); // mb_qp_delta
    sl.u(6, 0b000011); // DC coeff_token: nC = 16 from the PCM neighbour, TotalCoeff 0
    sl.trailing();
    sl.nal(0x65, &mut out);
    out
}

#[test]
fn pcm_macroblock_and_its_neighbour() {
    let mut pcm = [0u8; 384];
    let mut x = 0x1234_5678u32;
    for v in pcm.iter_mut() {
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        *v = (x >> 24) as u8;
    }
    let stream = pcm_stream(&pcm, false);
    let pic = decode_access_unit(&stream, &DecoderConfig::annex_b()).unwrap();
    assert_eq!((pic.width, pic.height), (32, 16));
    for y in 0..16 {
        for xx in 0..16 {
            assert_eq!(pic.y[y * 32 + xx], pcm[y * 16 + xx] as u16);
        }
    }
    // Intra_16x16 DC from the left column of the PCM block only.
    let sum: u32 = (0..16).map(|y| pcm[y * 16 + 15] as u32).sum();
    let dc = ((sum + 8) >> 4) as u16;
    for y in 0..16 {
        for xx in 16..32 {
            assert_eq!(pic.y[y * 32 + xx], dc, "({xx},{y})");
        }
    }
    for (c, plane) in [&pic.cb, &pic.cr].into_iter().enumerate() {
        let off = 256 + c * 64;
        for y in 0..8 {
            for xx in 0..8 {
                assert_eq!(plane[y * 16 + xx], pcm[off + y * 8 + xx] as u16);
            }
            // Chroma DC: each 4-row band from the left neighbour's column.
        }
        for band in 0..2 {
            let s: u32 = (0..4)
                .map(|y| pcm[off + (band * 4 + y) * 8 + 7] as u32)
                .sum();
            let v = ((s + 2) >> 2) as u16;
            for y in band * 4..band * 4 + 4 {
                for xx in 8..16 {
                    assert_eq!(plane[y * 16 + xx], v, "chroma {c} ({xx},{y})");
                }
            }
        }
    }
}

#[test]
fn pcm_with_deblocking_matches_ffmpeg() {
    let Some(ff) = common::ffmpeg() else {
        eprintln!("ffmpeg not found; skipping");
        return;
    };
    let mut pcm = [0u8; 384];
    for (i, v) in pcm.iter_mut().enumerate() {
        *v = (100 + (i * 7) % 40) as u8;
    }
    let stream = pcm_stream(&pcm, true);
    let src = common::tmp("pcm_deblock.h264");
    std::fs::write(&src, &stream).unwrap();
    let want = common::reference(&ff, &src, "yuv420p", &common::tmp("pcm_deblock.yuv"));
    let pic = decode_access_unit(&stream, &DecoderConfig::annex_b()).unwrap();
    let (bad, first) = common::compare(&pic, &want);
    assert_eq!(bad, [0; 3], "{first:?}");
}
