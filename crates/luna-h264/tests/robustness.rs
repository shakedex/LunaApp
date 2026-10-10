//! Damaged input never panics: deterministic bit flips, truncations, byte
//! overwrites and splices of the fixtures, ~1000 iterations.

use luna_h264::{DecoderConfig, access_unit_kind, access_unit_kind_prefix, decode_access_unit};

const STREAMS: &[&[u8]] = &[
    include_bytes!("fixtures/cavlc422p10.h264"),
    include_bytes!("fixtures/cabac422p10.h264"),
    include_bytes!("fixtures/cabac420.h264"),
];

struct XorShift(u64);

impl XorShift {
    fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }
    fn below(&mut self, n: usize) -> usize {
        (self.next() % n.max(1) as u64) as usize
    }
}

fn mutate(rng: &mut XorShift, src: &[u8]) -> Vec<u8> {
    let mut d = src.to_vec();
    match rng.below(5) {
        0 => {
            for _ in 0..1 + rng.below(16) {
                let i = rng.below(d.len());
                d[i] ^= 1 << rng.below(8);
            }
        }
        1 => {
            let n = rng.below(d.len());
            d.truncate(n);
        }
        2 => {
            let at = rng.below(d.len());
            let len = 1 + rng.below(64);
            for k in at..(at + len).min(d.len()) {
                d[k] = rng.next() as u8;
            }
        }
        3 => {
            // Splice a chunk from elsewhere (possibly another stream).
            let other = STREAMS[rng.below(STREAMS.len())];
            let from = rng.below(other.len());
            let len = 1 + rng.below(200);
            let chunk = &other[from..(from + len).min(other.len())];
            let at = rng.below(d.len());
            d.splice(at..at, chunk.iter().copied());
        }
        _ => {
            // Header damage: flips in the first 64 bytes (SPS/PPS/slice header).
            for _ in 0..1 + rng.below(4) {
                let i = rng.below(d.len().min(64));
                d[i] ^= 1 << rng.below(8);
            }
        }
    }
    d
}

#[test]
fn damaged_streams_never_panic() {
    // LUNA_FUZZ_ITERS / LUNA_FUZZ_SEED widen the search locally.
    let iters = std::env::var("LUNA_FUZZ_ITERS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(1000);
    let seed = std::env::var("LUNA_FUZZ_SEED")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(0x9E37_79B9_7F4A_7C15u64);
    let mut rng = XorShift(seed);
    let cfg = DecoderConfig::annex_b();
    let (mut ok, mut err) = (0, 0);
    for i in 0..iters {
        let src = STREAMS[i % STREAMS.len()];
        let d = mutate(&mut rng, src);
        let _ = access_unit_kind(&d, &cfg);
        let _ = access_unit_kind_prefix(&d[..d.len() / 2], &cfg);
        match decode_access_unit(&d, &cfg) {
            Ok(p) => {
                ok += 1;
                assert_eq!(p.y.len(), (p.width * p.height) as usize);
                assert_eq!(p.cb.len(), (p.chroma_width() * p.chroma_height()) as usize);
            }
            Err(_) => err += 1,
        }
    }
    eprintln!("{ok} decoded, {err} rejected");
    assert!(err > 0);
}

#[test]
fn garbage_and_empty_input_are_errors() {
    let cfg = DecoderConfig::annex_b();
    assert!(decode_access_unit(&[], &cfg).is_err());
    assert!(decode_access_unit(&[0, 0, 1], &cfg).is_err());
    assert!(decode_access_unit(&[0xff; 100], &cfg).is_err());
    let mut rng = XorShift(7);
    for _ in 0..200 {
        let n = rng.below(300);
        let d: Vec<u8> = (0..n).map(|_| rng.next() as u8).collect();
        let _ = decode_access_unit(&d, &cfg);
        let _ = DecoderConfig::from_avcc(&d);
        if let Ok(c) = DecoderConfig::from_avcc(&d) {
            let _ = decode_access_unit(&d, &c);
        }
    }
}
