//! DJI (Ronin 4D): the `djmd` timed-metadata track holds one protocol-buffer
//! message per frame. Without DJI's schema, only fields whose meaning is
//! unambiguous from structure and content are read (see `read`).

use super::display;
use crate::types::{ClipMeta, Transfer};

#[derive(Debug, Clone, Copy, PartialEq)]
enum Wire<'a> {
    Varint(u64),
    Fixed64([u8; 8]),
    Fixed32([u8; 4]),
    Bytes(&'a [u8]),
}

fn varint(b: &[u8], p: &mut usize) -> Option<u64> {
    let mut v = 0u64;
    for shift in (0..64).step_by(7) {
        let byte = *b.get(*p)?;
        *p += 1;
        v |= ((byte & 0x7F) as u64) << shift;
        if byte & 0x80 == 0 {
            return Some(v);
        }
    }
    None
}

/// The fields of one message, in order; stops at the first malformed field.
fn fields(b: &[u8]) -> Vec<(u64, Wire<'_>)> {
    let mut out = Vec::new();
    let mut p = 0;
    while p < b.len() {
        let Some(key) = varint(b, &mut p) else { break };
        let v = match key & 7 {
            0 => match varint(b, &mut p) {
                Some(v) => Wire::Varint(v),
                None => break,
            },
            1 => match b.get(p..p + 8) {
                Some(s) => {
                    p += 8;
                    let mut a = [0u8; 8];
                    a.copy_from_slice(s);
                    Wire::Fixed64(a)
                }
                None => break,
            },
            2 => {
                let Some(len) = varint(b, &mut p).and_then(|l| usize::try_from(l).ok()) else {
                    break;
                };
                match p.checked_add(len).and_then(|e| b.get(p..e)) {
                    Some(s) => {
                        p += len;
                        Wire::Bytes(s)
                    }
                    None => break,
                }
            }
            5 => match b.get(p..p + 4) {
                Some(s) => {
                    p += 4;
                    Wire::Fixed32([s[0], s[1], s[2], s[3]])
                }
                None => break,
            },
            _ => break,
        };
        out.push((key >> 3, v));
    }
    out
}

/// The first field at a path of field numbers.
fn at<'a>(msg: &'a [u8], path: &[u64]) -> Option<Wire<'a>> {
    let (&last, parents) = path.split_last()?;
    let mut cur = msg;
    for &n in parents {
        cur = fields(cur).into_iter().find_map(|(k, v)| match v {
            Wire::Bytes(b) if k == n => Some(b),
            _ => None,
        })?;
    }
    fields(cur)
        .into_iter()
        .find(|&(k, _)| k == last)
        .map(|(_, v)| v)
}

fn text(msg: &[u8], path: &[u64]) -> Option<String> {
    match at(msg, path)? {
        Wire::Bytes(b) => super::bytes::clean_text(b),
        _ => None,
    }
}

fn float(msg: &[u8], path: &[u64]) -> Option<f32> {
    match at(msg, path)? {
        Wire::Fixed32(a) => Some(f32::from_le_bytes(a)).filter(|v| v.is_finite()),
        _ => None,
    }
}

fn uint(msg: &[u8], path: &[u64]) -> Option<u64> {
    match at(msg, path)? {
        Wire::Varint(v) => Some(v),
        _ => None,
    }
}

/// Fields read from the first `djmd` message (Ronin 4D layout):
/// - 2.2.1.4 camera model ("DJI …"), make taken from its first word
/// - 2.2.2.1.4 lens name
/// - 2.2.11.1 log curve name ("D-Log")
/// - 3.2.3.1 ISO (float), 3.2.13.1 white balance in kelvin
///
/// Shutter, iris, ND, serials and clip identity are present but their meaning
/// can't be pinned without DJI's schema, so they are left out.
pub(crate) fn read(sample: &[u8]) -> Option<ClipMeta> {
    let model = text(sample, &[2, 2, 1, 4]).filter(|m| m.starts_with("DJI"));
    let model = model?;
    let mut m = ClipMeta {
        camera_make: model.split_whitespace().next().map(str::to_string),
        camera_model: Some(model),
        lens: text(sample, &[2, 2, 2, 1, 4]),
        iso: float(sample, &[3, 2, 3, 1])
            .filter(|&v| v >= 1.0 && v.fract() == 0.0)
            .map(|v| (v as u32).to_string()),
        white_balance: uint(sample, &[3, 2, 13, 1])
            .filter(|&k| (1000..=20000).contains(&k))
            .and_then(|k| display::kelvin(k as i64)),
        ..ClipMeta::default()
    };
    if let Some(curve) = text(sample, &[2, 2, 11, 1]) {
        let c = curve.to_ascii_uppercase().replace(['-', ' ', '_'], "");
        if c == "DLOG" {
            m.colour.transfer = Some(Transfer::DLog);
        }
    }
    Some(m)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn len_field(n: u64, body: &[u8]) -> Vec<u8> {
        let mut v = vec![((n << 3) | 2) as u8, body.len() as u8];
        v.extend_from_slice(body);
        v
    }

    fn f32_field(n: u64, x: f32) -> Vec<u8> {
        let mut v = vec![((n << 3) | 5) as u8];
        v.extend_from_slice(&x.to_le_bytes());
        v
    }

    fn sample() -> Vec<u8> {
        let model = len_field(1, &len_field(4, b"DJI HG4180"));
        let lens = len_field(2, &len_field(1, &len_field(4, b"DJI_DL_35mm_F2.8_ND_ASPH")));
        let curve = len_field(11, &len_field(1, b"D-Log"));
        let mut static_ = model;
        static_.extend(lens);
        static_.extend(curve);
        let video = len_field(2, &len_field(2, &static_));
        let mut cam = len_field(3, &f32_field(1, 800.0));
        cam.extend(len_field(13, &[0x08, 0xE0, 0x2B]));
        let frame = len_field(3, &len_field(2, &cam));
        let mut s = len_field(1, &len_field(1, b"dvtm_HG4181.proto"));
        s.extend(video);
        s.extend(frame);
        s
    }

    #[test]
    fn reads_the_unambiguous_fields() {
        let m = read(&sample()).unwrap();
        assert_eq!(m.camera_make.as_deref(), Some("DJI"));
        assert_eq!(m.camera_model.as_deref(), Some("DJI HG4180"));
        assert_eq!(m.lens.as_deref(), Some("DJI_DL_35mm_F2.8_ND_ASPH"));
        assert_eq!(m.iso.as_deref(), Some("800"));
        assert_eq!(m.white_balance.as_deref(), Some("5600 K"));
        assert_eq!(m.colour.transfer, Some(Transfer::DLog));
        assert_eq!(m.colour.gamut, None);
    }

    #[test]
    fn truncations_never_panic() {
        let s = sample();
        for cut in 0..s.len() {
            let _ = read(&s[..cut]);
        }
        assert!(read(&[0xFF; 32]).is_none());
    }
}
