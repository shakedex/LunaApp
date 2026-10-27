//! ARRI metadata in MXF.
//!
//! - ALEXA 35 generation: JSON documents in header metadata (Slate Info,
//!   Camera Device, Lens Device, …) and a per-frame data element of nested
//!   KLV sets (0x0c posture, 0x0d lens, 0x0e exposure).
//! - ALEXA Mini LF generation: the 4 KiB ARRIRAW-style camera header
//!   (little-endian, `ARRI` + 0x12345678) inside a vendor header set.

use std::collections::HashMap;

use super::klv::{Ul, ber, ul};
use super::metadata::Metadata;

const JSON_SET: [u8; 13] = [
    0x06, 0x0e, 0x2b, 0x34, 0x02, 0x53, 0x01, 0x01, 0x0e, 0x17, 0x01, 0x02, 0x10,
];
const JSON_NAME: Ul = ul("060e2b340101010e0e17010210010000");
const JSON_DATA: Ul = ul("060e2b340101010e0e17010210030000");

/// The ARRI JSON documents by name ("Slate Info", "Camera Device", …).
pub(crate) fn json_documents(md: &Metadata) -> HashMap<String, serde_json::Value> {
    let mut out = HashMap::new();
    for s in md.with_key(&JSON_SET) {
        let item = |u: &Ul| s.items.iter().find(|i| i.ul.is_some_and(|x| x.is(u)));
        let Some(name) = item(&JSON_NAME).map(|i| utf16(&i.value)) else {
            continue;
        };
        let Some(data) = item(&JSON_DATA) else {
            continue;
        };
        if let Ok(v) = serde_json::from_slice::<serde_json::Value>(&data.value) {
            out.insert(name, v);
        }
    }
    out
}

fn utf16(v: &[u8]) -> String {
    let u: Vec<u16> = v
        .as_chunks::<2>()
        .0
        .iter()
        .map(|c| u16::from_be_bytes([c[0], c[1]]))
        .take_while(|&x| x != 0)
        .collect();
    String::from_utf16_lossy(&u).trim().to_string()
}

/// A string field of a JSON document, if present and non-empty.
pub(crate) fn json_str(
    docs: &HashMap<String, serde_json::Value>,
    doc: &str,
    field: &str,
) -> Option<String> {
    let v = docs.get(doc)?.get(field)?;
    let s = match v {
        serde_json::Value::String(s) => s.trim().to_string(),
        serde_json::Value::Number(n) => n.to_string(),
        _ => return None,
    };
    (!s.is_empty()).then_some(s)
}

/// Per-frame values from the ALEXA 35 frame metadata element.
#[derive(Debug, Default, Clone, PartialEq)]
pub(crate) struct FrameMeta {
    pub focus_mm: Option<u32>,
    pub iris_t1000: Option<u32>,
    pub focal_um: Option<u32>,
    pub exposure_time: Option<(u32, u32)>,
    pub exposure_index: Option<u32>,
    pub white_balance: Option<u16>,
    /// Green-magenta tint (ARRI CC units, -16..+16).
    pub tint: Option<f32>,
    pub sensor_fps: Option<(u32, u32)>,
    pub nd: Option<f32>,
}

/// Parses the value of an ARRI frame metadata element (`…0d01030117010f00`).
pub(crate) fn frame_meta(v: &[u8]) -> FrameMeta {
    let mut items: HashMap<(u8, u8), Vec<u8>> = HashMap::new();
    walk(v, &mut |k, val| {
        // A set: 06 0e 2b 34 02 01 01 01 0e 17 01 02 ss 00 00 00
        if k[4] == 0x02 && k[8..12] == [0x0e, 0x17, 0x01, 0x02] {
            walk(val, &mut |ik, iv| {
                if ik[8..12] == [0x0e, 0x17, 0x01, 0x02] {
                    items.insert((ik[12], ik[13]), iv.to_vec());
                }
            });
        }
    });
    let u32_of = |s: u8, i: u8| {
        items
            .get(&(s, i))
            .and_then(|v| v.get(..4))
            .map(|b| u32::from_be_bytes(b.try_into().expect("4")))
    };
    let rat = |s: u8, i: u8| {
        let v = items.get(&(s, i))?.get(..8)?;
        let n = u32::from_be_bytes(v[..4].try_into().expect("4"));
        let d = u32::from_be_bytes(v[4..].try_into().expect("4"));
        (d != 0).then_some((n, d))
    };
    FrameMeta {
        focus_mm: u32_of(0x0d, 0x01),
        iris_t1000: u32_of(0x0d, 0x03),
        focal_um: u32_of(0x0d, 0x05),
        exposure_time: rat(0x0e, 0x01),
        exposure_index: u32_of(0x0e, 0x03),
        white_balance: items
            .get(&(0x0e, 0x05))
            .and_then(|v| v.get(..2))
            .map(|b| u16::from_be_bytes([b[0], b[1]])),
        tint: u32_of(0x0e, 0x06)
            .map(f32::from_bits)
            .filter(|f| f.is_finite()),
        sensor_fps: rat(0x0e, 0x07),
        nd: u32_of(0x0e, 0x08)
            .map(f32::from_bits)
            .filter(|f| f.is_finite()),
    }
}

fn walk(v: &[u8], f: &mut dyn FnMut(&[u8], &[u8])) {
    let mut i = 0usize;
    while i + 17 <= v.len() {
        let k = &v[i..i + 16];
        if k[..4] != [0x06, 0x0e, 0x2b, 0x34] {
            break;
        }
        let Some((len, used)) = ber(&v[i + 16..]) else {
            break;
        };
        let s = i + 16 + used;
        let Some(e) = usize::try_from(len).ok().and_then(|l| s.checked_add(l)) else {
            break;
        };
        let Some(val) = v.get(s..e) else { break };
        f(k, val);
        i = e;
    }
}

/// Fields of the ARRIRAW-style camera header carried by ALEXA Mini LF MXF.
#[derive(Debug, Default, Clone, PartialEq)]
pub(crate) struct LegacyHeader {
    pub white_balance: Option<u32>,
    pub exposure_index: Option<u32>,
    pub exposure_us: Option<u32>,
    /// Shutter angle × 1000.
    pub shutter_angle_1000: Option<u32>,
    /// Sensor frame rate × 1000.
    pub sensor_fps_1000: Option<u32>,
    pub camera_index: Option<String>,
    pub camera_model: Option<String>,
    pub focal_um: Option<u32>,
    pub lens_model: Option<String>,
    pub reel_name: Option<String>,
    pub clip_name: Option<String>,
    pub camera_serial: Option<u32>,
    /// ND density × 1000.
    pub nd_1000: Option<u16>,
}

const LEGACY_MAGIC: [u8; 8] = [b'A', b'R', b'R', b'I', 0x12, 0x34, 0x56, 0x78];

/// Finds and parses the ARRI legacy header in a header-metadata vendor set.
pub(crate) fn legacy_header(md: &Metadata) -> Option<LegacyHeader> {
    let blob = md
        .sets
        .iter()
        .filter(|s| s.key.0[5] == 0x13)
        .flat_map(|s| s.items.iter())
        .find(|i| i.value.starts_with(&LEGACY_MAGIC))?;
    Some(parse_legacy(&blob.value))
}

/// Offsets in the 4 KiB header (relative to `ARRI`), identified against an
/// ALEXA Mini LF clip whose JSON-era counterparts and clip name confirm them.
/// Serial (0x170) and ND (0x38a) were matched against ARRI's `art-cmd` export
/// of an ALEXA Mini LF ARRIRAW clip.
pub(crate) fn parse_legacy(h: &[u8]) -> LegacyHeader {
    let u32_at = |o: usize| {
        h.get(o..o + 4)
            .map(|b| u32::from_le_bytes(b.try_into().expect("4")))
            .filter(|&x| x != 0 && x != u32::MAX)
    };
    let str_at = |o: usize, n: usize| {
        let b = h.get(o..o + n)?;
        let s: String = b
            .iter()
            .take_while(|&&c| c != 0 && c != 0xff)
            .map(|&c| c as char)
            .collect();
        let s = s.trim().to_string();
        (!s.is_empty() && s.chars().all(|c| !c.is_control())).then_some(s)
    };
    LegacyHeader {
        white_balance: u32_at(0x5c),
        exposure_index: u32_at(0x74),
        exposure_us: u32_at(0x18c),
        shutter_angle_1000: u32_at(0x190),
        sensor_fps_1000: u32_at(0x1a0),
        camera_index: h
            .get(0x178)
            .filter(|c| c.is_ascii_uppercase())
            .map(|&c| (c as char).to_string()),
        camera_model: str_at(0x2f8, 32),
        focal_um: u32_at(0x37c),
        lens_model: str_at(0x398, 32),
        reel_name: str_at(0x4f8, 32),
        clip_name: str_at(0x698, 32),
        camera_serial: u32_at(0x170),
        nd_1000: h
            .get(0x38a..0x38c)
            .map(|b| u16::from_le_bytes([b[0], b[1]]))
            .filter(|&n| n != u16::MAX),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn klv(key: [u8; 16], v: &[u8]) -> Vec<u8> {
        let mut o = key.to_vec();
        o.push(0x83);
        o.extend(&(v.len() as u32).to_be_bytes()[1..]);
        o.extend(v);
        o
    }

    fn item_key(set: u8, item: u8) -> [u8; 16] {
        [
            0x06, 0x0e, 0x2b, 0x34, 0x01, 0x01, 0x01, 0x0e, 0x0e, 0x17, 0x01, 0x02, set, item, 0, 0,
        ]
    }

    #[test]
    fn frame_meta_sets() {
        let mut lens = klv(item_key(0x0d, 0x05), &44978u32.to_be_bytes());
        lens.extend(klv(item_key(0x0d, 0x03), &5190u32.to_be_bytes()));
        let mut expo = klv(item_key(0x0e, 0x01), &[0, 0, 0, 1, 0, 0, 0, 50]);
        expo.extend(klv(item_key(0x0e, 0x05), &5600u16.to_be_bytes()));
        expo.extend(klv(item_key(0x0e, 0x08), &1.2f32.to_bits().to_be_bytes()));
        expo.extend(klv(
            item_key(0x0e, 0x06),
            &(-3.0f32).to_bits().to_be_bytes(),
        ));
        let set = |s: u8| {
            [
                0x06, 0x0e, 0x2b, 0x34, 0x02, 0x01, 0x01, 0x01, 0x0e, 0x17, 0x01, 0x02, s, 0, 0, 0,
            ]
        };
        let mut v = klv(set(0x0d), &lens);
        v.extend(klv(set(0x0e), &expo));
        let m = frame_meta(&v);
        assert_eq!(m.focal_um, Some(44978));
        assert_eq!(m.iris_t1000, Some(5190));
        assert_eq!(m.exposure_time, Some((1, 50)));
        assert_eq!(m.white_balance, Some(5600));
        assert_eq!(m.nd, Some(1.2));
        assert_eq!(m.tint, Some(-3.0));
        assert_eq!(frame_meta(&v[..v.len() - 3]).tint, None);
    }

    #[test]
    fn legacy_header_tolerates_short_input() {
        let mut h = LEGACY_MAGIC.to_vec();
        h.resize(0x80, 0);
        h[0x74..0x78].copy_from_slice(&800u32.to_le_bytes());
        let l = parse_legacy(&h);
        assert_eq!(l.exposure_index, Some(800));
        assert_eq!(l.lens_model, None);
        assert_eq!((l.camera_serial, l.nd_1000), (None, None));
    }

    #[test]
    fn legacy_header_serial_and_nd() {
        let mut h = LEGACY_MAGIC.to_vec();
        h.resize(0x1000, 0xff);
        h[0x170..0x174].copy_from_slice(&30141u32.to_le_bytes());
        h[0x38a..0x38c].copy_from_slice(&600u16.to_le_bytes());
        let l = parse_legacy(&h);
        assert_eq!((l.camera_serial, l.nd_1000), (Some(30141), Some(600)));
    }
}
