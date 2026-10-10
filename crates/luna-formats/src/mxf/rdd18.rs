//! SMPTE RDD 18 acquisition metadata, carried in ST 436 ANC packets
//! (DID 0x43, SDID 0x05) by Sony, Canon and others.
//!
//! An ANC element holds packets; the RDD 18 payload is split across them,
//! each packet's user data starting with a sequence byte. Joined, the payload
//! is a run of KLV local sets with fixed 2-byte tags.

use super::klv::{Ul, ber};

#[derive(Debug, Default, Clone, PartialEq)]
pub(crate) struct Acquisition {
    pub iris_f: Option<f64>,
    pub iris_t: Option<f64>,
    /// Actual focal length in millimetres.
    pub focal_mm: Option<f64>,
    pub lens_attributes: Option<String>,
    pub exposure_index: Option<u32>,
    pub iso: Option<u32>,
    pub nd_setting: Option<u16>,
    /// Shutter angle in degrees.
    pub shutter_angle: Option<f64>,
    pub shutter_time: Option<(u32, u32)>,
    pub capture_fps: Option<(u32, u32)>,
    pub white_balance: Option<u16>,
    pub camera_attributes: Option<String>,
    pub capture_gamma: Option<Ul>,
    /// Sony private: gamma and primaries the look is built on.
    pub look_gamma: Option<Ul>,
    pub look_primaries: Option<Ul>,
    /// Canon private: model, serial, recording date.
    pub vendor_model: Option<String>,
    pub vendor_serial: Option<String>,
    pub vendor_transfer: Option<Ul>,
    pub vendor_primaries: Option<Ul>,
    pub vendor_date: Option<(u16, u8, u8, u8, u8, u8)>,
}

/// Joins the RDD 18 user data of an ST 436 ANC element value.
pub(crate) fn anc_payload(v: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    if v.len() < 2 {
        return out;
    }
    let n = u16::from_be_bytes([v[0], v[1]]) as usize;
    let mut i = 2usize;
    for _ in 0..n {
        // line u16, wrapping u8, coding u8, sample count u16, then an array.
        let Some(h) = v.get(i..i + 14) else { break };
        let count = u32::from_be_bytes(h[6..10].try_into().expect("4")) as usize;
        let size = u32::from_be_bytes(h[10..14].try_into().expect("4")) as usize;
        let Some(bytes) = count.checked_mul(size) else {
            break;
        };
        let Some(arr) = v.get(i + 14..i + 14 + bytes) else {
            break;
        };
        i += 14 + bytes;
        if arr.len() >= 4 && arr[0] == 0x43 && arr[1] == 0x05 {
            let dc = arr[2] as usize;
            if let Some(udw) = arr.get(3..3 + dc)
                && udw.len() > 1
            {
                out.extend_from_slice(&udw[1..]);
            }
        }
    }
    out
}

fn text(v: &[u8]) -> Option<String> {
    let s: String = String::from_utf8_lossy(v)
        .chars()
        .take_while(|&c| c != '\0')
        .collect();
    let s = s.trim();
    (!s.is_empty()).then(|| s.to_string())
}

fn be_u16(v: &[u8]) -> Option<u16> {
    v.get(..2).map(|b| u16::from_be_bytes([b[0], b[1]]))
}

fn be_u32(v: &[u8]) -> Option<u32> {
    v.get(..4)
        .map(|b| u32::from_be_bytes(b.try_into().expect("4")))
}

fn rational(v: &[u8]) -> Option<(u32, u32)> {
    Some((be_u32(v)?, be_u32(v.get(4..)?)?)).filter(|&(_, d)| d != 0)
}

/// RDD 18 iris: F = 2^(8 · (1 − X/65536)).
fn iris(v: &[u8]) -> Option<f64> {
    let x = be_u16(v)?;
    (x != 0 && x != 0xffff).then(|| 2f64.powf(8.0 * (1.0 - x as f64 / 65536.0)))
}

/// RDD 18 distance: 4-bit signed decimal exponent, 12-bit mantissa, metres.
fn distance_m(v: &[u8]) -> Option<f64> {
    let x = be_u16(v)?;
    if x == 0xffff {
        return None;
    }
    let e = ((x >> 12) as i8) << 4 >> 4;
    Some((x & 0x0fff) as f64 * 10f64.powi(e as i32))
}

/// Decodes an RDD 18 payload (see [`anc_payload`]).
pub(crate) fn parse(payload: &[u8]) -> Acquisition {
    let mut a = Acquisition::default();
    let mut i = 0usize;
    while i + 17 <= payload.len() {
        let key = &payload[i..i + 16];
        let Some((len, used)) = ber(&payload[i + 16..]) else {
            break;
        };
        let s = i + 16 + used;
        let Some(e) = usize::try_from(len).ok().and_then(|l| s.checked_add(l)) else {
            break;
        };
        let Some(set) = payload.get(s..e) else { break };
        if key[..4] == [0x06, 0x0e, 0x2b, 0x34] {
            items(set, &mut a);
        }
        i = e;
    }
    a
}

fn items(set: &[u8], a: &mut Acquisition) {
    let mut q = 0usize;
    while q + 4 <= set.len() {
        let tag = u16::from_be_bytes([set[q], set[q + 1]]);
        let l = u16::from_be_bytes([set[q + 2], set[q + 3]]) as usize;
        let Some(v) = set.get(q + 4..q + 4 + l) else {
            break;
        };
        match tag {
            0x8000 => a.iris_f = iris(v),
            0x8008 => a.iris_t = iris(v),
            0x8005 => a.focal_mm = distance_m(v).map(|m| m * 1000.0),
            0x8007 => a.lens_attributes = text(v),
            0x8103 => a.nd_setting = be_u16(v),
            0x8106 => a.capture_fps = rational(v),
            0x8108 => a.shutter_angle = be_u32(v).map(|x| x as f64 / 60.0),
            0x8109 => a.shutter_time = rational(v),
            0x810b => a.iso = be_u16(v).map(u32::from),
            0x810e => a.white_balance = be_u16(v),
            0x8114 => a.camera_attributes = text(v),
            0x8115 => a.exposure_index = be_u16(v).map(u32::from),
            0x8119 if a.exposure_index.is_none() => a.exposure_index = be_u32(v),
            0x3210 => a.capture_gamma = Ul::from_slice(v),
            0xe111 => a.look_gamma = Ul::from_slice(v),
            0xe112 => a.look_primaries = Ul::from_slice(v),
            0xe211 => a.vendor_transfer = Ul::from_slice(v),
            0xe212 => a.vendor_primaries = Ul::from_slice(v),
            0xe227 => a.vendor_date = canon_date(v),
            0xe228 => a.vendor_model = text(v),
            0xe229 => a.vendor_serial = text(v),
            _ => {}
        }
        q += 4 + l;
    }
}

/// Canon's recording date item: `?? ?? ?? ?? YYYY MM DD hh mm ss …`.
fn canon_date(v: &[u8]) -> Option<(u16, u8, u8, u8, u8, u8)> {
    let d = v.get(4..11)?;
    super::metadata::timestamp(&[d[0], d[1], d[2], d[3], d[4], d[5], d[6], 0])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn iris_and_distance_encodings() {
        // Values from a BURANO clip whose XML sidecar states F1.84 / T1.92 /
        // 50 mm / 2.924 m.
        assert!((iris(&[0xe3, 0xe4]).unwrap() - 1.84).abs() < 0.01);
        assert!((iris(&[0xe1, 0xd6]).unwrap() - 1.92).abs() < 0.01);
        assert!((distance_m(&[0xd0, 0x32]).unwrap() - 0.050).abs() < 1e-9);
        assert!((distance_m(&[0xdb, 0x6c]).unwrap() - 2.924).abs() < 1e-9);
    }

    #[test]
    fn packets_join_and_sets_decode() {
        // One camera-unit set split across two ANC packets.
        let set_key = [
            0x06, 0x0e, 0x2b, 0x34, 0x02, 0x53, 0x01, 0x01, 0x0c, 0x02, 0x01, 0x01, 0x02, 0x01,
            0x00, 0x00,
        ];
        let mut payload = set_key.to_vec();
        let items = [
            0x81, 0x0e, 0x00, 0x02, 0x15, 0xe0, 0x81, 0x15, 0x00, 0x02, 0x03, 0x20,
        ];
        payload.push(items.len() as u8);
        payload.extend(items);
        let (p1, p2) = payload.split_at(10);
        let mut anc = vec![0x00, 0x02];
        for (seq, part) in [(1u8, p1), (2u8, p2)] {
            let mut arr = vec![0x43, 0x05, (part.len() + 1) as u8, seq];
            arr.extend(part);
            anc.extend([0x00, 0x0b, 0x04, 0x04, 0x00, arr.len() as u8]);
            anc.extend((arr.len() as u32).to_be_bytes());
            anc.extend(1u32.to_be_bytes());
            anc.extend(arr);
        }
        let a = parse(&anc_payload(&anc));
        assert_eq!(a.white_balance, Some(5600));
        assert_eq!(a.exposure_index, Some(800));
    }

    #[test]
    fn garbage_does_not_panic() {
        let mut x = 0x1234_5678u32;
        for n in 0..300 {
            let v: Vec<u8> = (0..n)
                .map(|_| {
                    x ^= x << 13;
                    x ^= x >> 17;
                    x ^= x << 5;
                    x as u8
                })
                .collect();
            let _ = parse(&anc_payload(&v));
            let _ = parse(&v);
        }
    }
}
