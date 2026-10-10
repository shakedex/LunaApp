//! Corruption harness: truncated, bit-flipped and garbage frames must come
//! back as `Err` (or a picture) and never panic. Deterministic xorshift seed.

fn fixture(name: &str) -> Vec<u8> {
    std::fs::read(format!(
        "{}/tests/fixtures/{name}.prores",
        env!("CARGO_MANIFEST_DIR")
    ))
    .unwrap()
}

struct XorShift(u64);

impl XorShift {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }

    fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }
}

const FIXTURES: [&str; 6] = [
    "proxy",
    "hq",
    "interlaced_tff",
    "interlaced_bff",
    "p4444_alpha",
    "p4444xq",
];

/// Decodes at a random downscale and thread count; only panics matter.
fn exercise(rng: &mut XorShift, data: &[u8]) -> bool {
    let ds = [1u8, 2, 4, 8][rng.below(4)];
    let threads = 1 + rng.below(3);
    luna_prores::decode_with_threads(data, ds, threads).is_ok()
}

#[test]
fn truncated_frames_are_rejected() {
    let mut rng = XorShift(0x9E37_79B9_7F4A_7C15);
    for name in FIXTURES {
        let frame = fixture(name);
        for _ in 0..200 {
            let cut = rng.below(frame.len());
            let ok = exercise(&mut rng, &frame[..cut]);
            assert!(!ok, "{name}: truncated to {cut} bytes decoded");
        }
    }
}

#[test]
fn bit_flips_never_panic() {
    let mut rng = XorShift(0xD1B5_4A32_D192_ED03);
    for name in FIXTURES {
        let frame = fixture(name);
        for _ in 0..200 {
            let mut data = frame.clone();
            for _ in 0..1 + rng.below(8) {
                let at = rng.below(data.len());
                data[at] ^= 1 << rng.below(8);
            }
            exercise(&mut rng, &data);
        }
    }
}

#[test]
fn header_field_flips_never_panic() {
    // Concentrate on the frame, picture and first slice headers.
    let mut rng = XorShift(0x2545_F491_4F6C_DD1D);
    for name in FIXTURES {
        let frame = fixture(name);
        for _ in 0..200 {
            let mut data = frame.clone();
            let at = rng.below(200.min(data.len()));
            data[at] = rng.next() as u8;
            exercise(&mut rng, &data);
        }
    }
}

#[test]
fn garbage_never_panics() {
    let mut rng = XorShift(0x0123_4567_89AB_CDEF);
    let template = fixture("hq");
    for _ in 0..300 {
        let len = rng.below(4096);
        let mut data: Vec<u8> = (0..len).map(|_| rng.next() as u8).collect();
        // Half the time keep a plausible container so parsing gets deeper.
        if len >= 8 && rng.below(2) == 0 {
            data[..4].copy_from_slice(&(len as u32).to_be_bytes());
            data[4..8].copy_from_slice(b"icpf");
            let keep = rng.below(template.len().min(len) - 7);
            data[8..8 + keep].copy_from_slice(&template[8..8 + keep]);
        }
        exercise(&mut rng, &data);
    }
}

#[test]
fn bad_downscale_is_unsupported() {
    assert!(matches!(
        luna_prores::decode(&fixture("hq"), 3),
        Err(luna_prores::Error::Unsupported(_))
    ));
}
