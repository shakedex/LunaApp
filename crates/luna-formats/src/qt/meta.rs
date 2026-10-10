//! Movie-level metadata: `meta`/`keys`/`ilst` (both the `mdta` key form and
//! the `mdir` four-character form), QuickTime `udta` text atoms (`©mak` …) and
//! 3GPP `udta` strings (`manu`, `modl`).

use super::atom::{Atoms, child};
use super::bytes::{Reader, clean_text};

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Value {
    Text(String),
    Int(i64),
    Float(f64),
    Bytes(Vec<u8>),
}

impl Value {
    pub(crate) fn text(&self) -> Option<&str> {
        match self {
            Value::Text(s) if !s.trim().is_empty() => Some(s.trim()),
            _ => None,
        }
    }

    pub(crate) fn int(&self) -> Option<i64> {
        match self {
            Value::Int(i) => Some(*i),
            Value::Text(s) => s.trim().parse().ok(),
            _ => None,
        }
    }
}

/// Metadata items in file order. Keys are the `mdta` key strings
/// (`com.arri.camera.CameraModel`) or the atom type as text (`©mak`, `manu`).
#[derive(Debug, Default, Clone)]
pub(crate) struct Items(pub Vec<(String, Value)>);

impl Items {
    pub(crate) fn get(&self, key: &str) -> Option<&Value> {
        self.0.iter().find(|(k, _)| k == key).map(|(_, v)| v)
    }

    pub(crate) fn text(&self, key: &str) -> Option<String> {
        self.get(key).and_then(Value::text).map(str::to_string)
    }

    pub(crate) fn int(&self, key: &str) -> Option<i64> {
        self.get(key).and_then(Value::int)
    }

    pub(crate) fn has_prefix(&self, prefix: &str) -> bool {
        self.0.iter().any(|(k, _)| k.starts_with(prefix))
    }
}

/// Values larger than this (LUTs, cover art) are kept as an empty marker.
const MAX_VALUE: usize = 1 << 20;

fn fourcc_key(k: &[u8; 4]) -> String {
    k.iter()
        .map(|&b| if b == 0xA9 { '©' } else { b as char })
        .collect()
}

/// A `meta` atom body. QuickTime's has no version/flags; ISO's has 4 bytes.
pub(crate) fn read_meta(meta: &[u8], out: &mut Items) {
    let body = if meta.get(4..8) == Some(b"hdlr") {
        meta
    } else {
        meta.get(4..).unwrap_or(&[])
    };
    let mut keys: Vec<String> = Vec::new();
    if let Some(k) = child(body, b"keys") {
        let mut r = Reader::new(k);
        if r.skip(4).is_some()
            && let Some(n) = r.u32()
        {
            for _ in 0..n {
                let Some(size) = r.u32() else { break };
                let Some(entry) = (size as usize).checked_sub(4).and_then(|s| r.take(s)) else {
                    break;
                };
                // namespace (4 bytes) + key text
                keys.push(String::from_utf8_lossy(entry.get(4..).unwrap_or(&[])).into_owned());
            }
        }
    }
    let Some(ilst) = child(body, b"ilst") else {
        return;
    };
    for item in Atoms::new(ilst) {
        let index = u32::from_be_bytes(item.kind) as usize;
        let key = if item.kind[0] == 0 && index >= 1 && index <= keys.len() {
            keys[index - 1].clone()
        } else {
            fourcc_key(&item.kind)
        };
        if let Some(data) = child(item.body, b"data")
            && let Some(v) = data_value(data)
        {
            out.0.push((key, v));
        }
    }
}

/// An `ilst` `data` atom: type indicator, locale, value.
fn data_value(data: &[u8]) -> Option<Value> {
    let mut r = Reader::new(data);
    let ty = r.u32()? & 0x00FF_FFFF;
    r.skip(4)?;
    let v = r.rest();
    if v.len() > MAX_VALUE {
        return Some(Value::Bytes(Vec::new()));
    }
    Some(match ty {
        1 => Value::Text(
            String::from_utf8_lossy(v)
                .trim_end_matches('\0')
                .to_string(),
        ),
        2 => Value::Text(utf16be(v)),
        21 | 22 => {
            if !matches!(v.len(), 1 | 2 | 3 | 4 | 8) {
                return Some(Value::Bytes(v.to_vec()));
            }
            let mut n: i64 = 0;
            for &b in v {
                n = (n << 8) | b as i64;
            }
            if ty == 21 && v.len() < 8 && v[0] & 0x80 != 0 {
                n -= 1 << (8 * v.len());
            }
            Value::Int(n)
        }
        23 if v.len() == 4 => Value::Float(f32::from_be_bytes([v[0], v[1], v[2], v[3]]) as f64),
        24 if v.len() == 8 => {
            let mut a = [0u8; 8];
            a.copy_from_slice(v);
            Value::Float(f64::from_be_bytes(a))
        }
        _ => Value::Bytes(v.to_vec()),
    })
}

fn utf16be(b: &[u8]) -> String {
    let u: Vec<u16> = b
        .as_chunks::<2>()
        .0
        .iter()
        .map(|&c| u16::from_be_bytes(c))
        .collect();
    String::from_utf16_lossy(&u)
        .trim_end_matches('\0')
        .to_string()
}

/// `udta` children: `©xxx` QuickTime text (u16 length, u16 language, text),
/// 3GPP strings (version/flags, u16 language, NUL-terminated UTF-8), and an
/// ISO `meta` (handled by [`read_meta`]). Other atoms are left to vendors.
pub(crate) fn read_udta(udta: &[u8], out: &mut Items) {
    for a in Atoms::new(udta) {
        match &a.kind {
            b"meta" => read_meta(a.body, out),
            [0xA9, ..] => {
                let mut r = Reader::new(a.body);
                if let Some(len) = r.u16()
                    && r.skip(2).is_some()
                    && let Some(t) = r.take(len as usize).and_then(clean_text)
                {
                    out.0.push((fourcc_key(&a.kind), Value::Text(t)));
                }
            }
            b"manu" | b"modl" | b"titl" | b"auth" => {
                if let Some(t) = a.body.get(6..).and_then(clean_text) {
                    out.0.push((fourcc_key(&a.kind), Value::Text(t)));
                }
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn atom(kind: &[u8; 4], body: &[u8]) -> Vec<u8> {
        let mut v = ((body.len() + 8) as u32).to_be_bytes().to_vec();
        v.extend_from_slice(kind);
        v.extend_from_slice(body);
        v
    }

    fn data(ty: u32, v: &[u8]) -> Vec<u8> {
        let mut b = ty.to_be_bytes().to_vec();
        b.extend_from_slice(&[0; 4]);
        b.extend_from_slice(v);
        atom(b"data", &b)
    }

    fn key(k: &str) -> Vec<u8> {
        let mut v = ((k.len() + 8) as u32).to_be_bytes().to_vec();
        v.extend_from_slice(b"mdta");
        v.extend_from_slice(k.as_bytes());
        v
    }

    #[test]
    fn mdta_keys_map_to_ilst_values() {
        let mut keys = vec![0, 0, 0, 0, 0, 0, 0, 2];
        keys.extend(key("com.arri.camera.CameraModel"));
        keys.extend(key("com.arri.camera.ExposureIndexAsa"));
        let mut ilst = atom(&[0, 0, 0, 1], &data(1, b"ARRI ALEXA Mini"));
        ilst.extend(atom(&[0, 0, 0, 2], &data(21, &[0, 0, 3, 0x20])));
        ilst.extend(atom(&[0, 0, 0, 9], &data(1, b"orphan")));
        let mut meta = atom(b"hdlr", &[0; 24]);
        meta.extend(atom(b"keys", &keys));
        meta.extend(atom(b"ilst", &ilst));
        let mut items = Items::default();
        read_meta(&meta, &mut items);
        assert_eq!(
            items.text("com.arri.camera.CameraModel").as_deref(),
            Some("ARRI ALEXA Mini")
        );
        assert_eq!(items.int("com.arri.camera.ExposureIndexAsa"), Some(800));
        assert!(items.has_prefix("com.arri.camera."));
    }

    #[test]
    fn signed_ints_and_floats() {
        assert_eq!(data_value(&data(21, &[0xFF])[8..]), Some(Value::Int(-1)));
        assert_eq!(data_value(&data(22, &[0xFF])[8..]), Some(Value::Int(255)));
        assert_eq!(
            data_value(&data(23, &1.5f32.to_be_bytes())[8..]),
            Some(Value::Float(1.5))
        );
    }

    #[test]
    fn udta_text_forms() {
        let mut udta = atom(b"\xA9mak", b"\x00\x05\x55\xC4Canon");
        udta.extend(atom(b"modl", b"\x00\x00\x00\x00\x15\xC7Canon EOS C50\x00"));
        let mut items = Items::default();
        read_udta(&udta, &mut items);
        assert_eq!(items.text("©mak").as_deref(), Some("Canon"));
        assert_eq!(items.text("modl").as_deref(), Some("Canon EOS C50"));
    }
}
