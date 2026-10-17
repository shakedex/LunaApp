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
/// - 2.2.3.1 camera serial (DJI's 14-character format, beside the model)
/// - 3.2.3.1 ISO (float), 3.2.13.1 white balance in kelvin
/// - 3.2.8.1 shutter angle in degrees (float; 180.0, confirmed as an angle)
/// - 3.2.9 ND: 9.1 = 1 when in use, 9.2 the filter factor (256.0 = 8 stops),
///   shown as optical density
/// - 3.2.10.1 iris as an f-number (float; 4.4 on the F2.8 prime)
/// - focal length from a prime lens's name ("DJI_DL_35mm_…")
pub(crate) fn read(sample: &[u8]) -> Option<ClipMeta> {
    let model = text(sample, &[2, 2, 1, 4]).filter(|m| m.starts_with("DJI"));
    let model = model?;
    let mut m = ClipMeta {
        camera_make: model.split_whitespace().next().map(str::to_string),
        camera_model: Some(model),
        camera_serial: text(sample, &[2, 2, 3, 1]).filter(|s| {
            s.len() == 14
                && s.bytes()
                    .all(|b| b.is_ascii_uppercase() || b.is_ascii_digit())
        }),
        lens: text(sample, &[2, 2, 2, 1, 4]),
        iso: float(sample, &[3, 2, 3, 1])
            .filter(|&v| v >= 1.0 && v.fract() == 0.0)
            .map(|v| (v as u32).to_string()),
        stop: float(sample, &[3, 2, 10, 1])
            .filter(|&v| (0.7..=64.0).contains(&v))
            .and_then(|v| display::f_number(v as f64)),
        shutter: float(sample, &[3, 2, 8, 1])
            .filter(|&v| v > 0.0 && v <= 360.0)
            .and_then(|v| display::shutter_angle(v as f64)),
        nd: nd_density(uint(sample, &[3, 2, 9, 1]), float(sample, &[3, 2, 9, 2])),
        white_balance: uint(sample, &[3, 2, 13, 1])
            .filter(|&k| (1000..=20000).contains(&k))
            .and_then(|k| display::kelvin(k as i64)),
        ..ClipMeta::default()
    };
    m.focal_length = m.lens.as_deref().and_then(prime_focal_length);
    if let Some(curve) = text(sample, &[2, 2, 11, 1]) {
        let c = curve.to_ascii_uppercase().replace(['-', ' ', '_'], "");
        if c == "DLOG" {
            m.colour.transfer = Some(Transfer::DLog);
        }
    }
    Some(m)
}

/// The ND filter factor as optical density ("2.4" for 256); none when the
/// filter is out or the factor is implausible.
fn nd_density(on: Option<u64>, factor: Option<f32>) -> Option<String> {
    let f = factor.filter(|&f| f > 1.0 && f <= 4096.0)?;
    if on != Some(1) {
        return None;
    }
    Some(display::decimal((f as f64).log10(), 1))
}

/// "DJI_DL_35mm_F2.8_ND_ASPH" → "35 mm": the one `<n>mm` token of a prime's
/// name (zooms name two focal lengths, so they get none).
fn prime_focal_length(lens: &str) -> Option<String> {
    let mut found = lens.split(['_', ' ']).filter_map(|t| {
        let n = t.strip_suffix("mm").or_else(|| t.strip_suffix("MM"))?;
        n.parse::<f64>().ok().filter(|&v| v > 0.0)
    });
    let mm = found.next()?;
    if found.next().is_some() {
        return None;
    }
    display::focal_length(mm)
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
        let serial = len_field(3, &len_field(1, b"4CVLJ62008001A"));
        let mut static_ = model;
        static_.extend(lens);
        static_.extend(curve);
        static_.extend(serial);
        let video = len_field(2, &len_field(2, &static_));
        let mut cam = len_field(3, &f32_field(1, 800.0));
        cam.extend(len_field(13, &[0x08, 0xE0, 0x2B]));
        cam.extend(len_field(10, &f32_field(1, 4.399_991_5)));
        cam.extend(len_field(8, &f32_field(1, 180.0)));
        let mut nd = vec![0x08, 0x01];
        nd.extend(f32_field(2, 256.0));
        cam.extend(len_field(9, &nd));
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
        assert_eq!(m.camera_serial.as_deref(), Some("4CVLJ62008001A"));
        assert_eq!(m.focal_length.as_deref(), Some("35 mm"));
        assert_eq!(m.stop.as_deref(), Some("f/4.4"));
        assert_eq!(m.shutter.as_deref(), Some("180°"));
        assert_eq!(m.nd.as_deref(), Some("2.4"));
        assert_eq!(m.iso.as_deref(), Some("800"));
        assert_eq!(m.white_balance.as_deref(), Some("5600 K"));
        assert_eq!(m.colour.transfer, Some(Transfer::DLog));
        assert_eq!(m.colour.gamut, None);
    }

    #[test]
    fn nd_only_when_in_use() {
        assert_eq!(nd_density(Some(1), Some(4.0)).as_deref(), Some("0.6"));
        assert_eq!(nd_density(Some(0), Some(256.0)), None);
        assert_eq!(nd_density(Some(1), Some(1.0)), None);
        assert_eq!(nd_density(Some(1), None), None);
    }

    #[test]
    fn focal_length_only_for_primes() {
        assert_eq!(
            prime_focal_length("DJI_DL_35mm_F2.8_ND_ASPH").as_deref(),
            Some("35 mm")
        );
        assert_eq!(prime_focal_length("DJI_DL_18-55mm_zoom"), None);
        assert_eq!(prime_focal_length("24mm_70mm"), None);
        assert_eq!(prime_focal_length("Unknown lens"), None);
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
