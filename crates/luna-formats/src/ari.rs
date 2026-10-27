//! ARRI frame-file sequences: `.ari` (ARRIRAW) and `.arx` (ARRIRAW HDE, as
//! Codex recorders write it). Every frame file starts with the 4 KiB ARRI
//! camera header (`ARRI` or `CDXA`, then 12 34 56 78); the clip's metadata is
//! read from the first frame's.
//!
//! Beyond the fields ALEXA Mini LF MXF headers share (`mxf::arri`), a frame
//! file's header carries per-frame values, matched against ARRI's `art-cmd`
//! export of an ALEXA Mini LF `.arx` clip: focus (mm) at 0x378, iris (T ×
//! 1000) at 0x384, timecode at 0x1a8 (BCD frames, seconds, minutes, hours) and
//! the recording date at 0x17c (BCD day, month, year, century, then
//! hundredths, seconds, minutes, hours).

use std::io::Read;

use crate::mxf::arri::parse_legacy;
use crate::mxf::display;
use crate::types::{
    ClipFiles, ClipMeta, Codec, ColourHint, FormatError, FrameIndex, Gamut, PictureLayout, Probe,
    Rational, Timecode, Transfer,
};

const HEADER_LEN: usize = 4096;

pub(crate) fn probe(clip: &ClipFiles) -> Result<Probe, FormatError> {
    let mut h = vec![0u8; HEADER_LEN];
    std::fs::File::open(&clip.main)
        .and_then(|mut f| f.read_exact(&mut h))
        .map_err(|e| FormatError::Io(format!("ARRI frame header: {e}")))?;
    let codec = match &h[..8] {
        [b'A', b'R', b'R', b'I', 0x12, 0x34, 0x56, 0x78] => Codec::ArriRaw,
        [b'C', b'D', b'X', b'A', 0x12, 0x34, 0x56, 0x78] => Codec::ArriRawHde,
        _ => return Err(FormatError::Corrupt("not an ARRI frame file".into())),
    };
    let u32_at = |o: usize| u32::from_le_bytes(h[o..o + 4].try_into().expect("4"));
    let some = |v: u32| (v != 0 && v != u32::MAX).then_some(v);
    let (width, height) = (u32_at(0x14), u32_at(0x18));
    let l = parse_legacy(&h);
    let fps = l.sensor_fps_1000.and_then(|f| Rational::reduced(f, 1000));
    let mut files = vec![clip.main.clone()];
    files.extend(clip.segments.iter().cloned());
    let meta = ClipMeta {
        recording_date: date(&h),
        start_timecode: fps.and_then(|r| timecode(&h, r)),
        // Project rate: the sensor rate (the only sample records both alike).
        frame_rate: fps,
        frame_count: Some(files.len() as u64),
        width: some(width),
        height: some(height),
        camera_make: Some("ARRI".into()),
        camera_model: l
            .camera_model
            .map(|m| m.strip_prefix("ARRI ").unwrap_or(&m).to_string()),
        camera_serial: l.camera_serial.map(|s| s.to_string()),
        lens: l.lens_model,
        focal_length: l.focal_um.and_then(|u| display::focal(u as f64 / 1000.0)),
        stop: some(u32_at(0x384)).and_then(|t| display::t_stop(t as f64 / 1000.0)),
        iso: l.exposure_index.map(|e| e.to_string()),
        shutter: l
            .shutter_angle_1000
            .and_then(|a| display::angle(a as f64 / 1000.0)),
        white_balance: l.white_balance.and_then(display::kelvin),
        nd: l
            .nd_1000
            .and_then(|n| display::nd_density(n as f64 / 1000.0)),
        sensor_fps: fps,
        audio: None,
        embedded_reel: l.reel_name,
        embedded_camera_id: l.camera_index,
        slate: Default::default(),
        // ALEV III cameras record ARRIRAW for LogC3 / AWG3.
        colour: ColourHint {
            transfer: Some(Transfer::LogC3),
            gamut: Some(Gamut::ArriWideGamut3),
        },
    };
    Ok(Probe {
        codec,
        meta,
        frames: FrameIndex::Files(files),
        layout: PictureLayout {
            stored_width: width,
            stored_height: height,
            raw_bits: Some(12),
            pixel_aspect: None,
            rotation: 0,
        },
        codec_config: None,
        warnings: Vec::new(),
    })
}

fn bcd(b: u8) -> Option<u32> {
    let (hi, lo) = (b >> 4, b & 15);
    (hi < 10 && lo < 10).then_some(hi as u32 * 10 + lo as u32)
}

fn timecode(h: &[u8], rate: Rational) -> Option<Timecode> {
    let [f, s, m, hh] = [0, 1, 2, 3].map(|i| bcd(h[0x1a8 + i]));
    let (f, s, m, hh) = (f?, s?, m?, hh?);
    let base = rate.as_f64().round() as u32;
    (base > 0 && f < base && s < 60 && m < 60 && hh < 24).then(|| Timecode {
        frames: ((hh * 60 + m) * 60 + s) as u64 * base as u64 + f as u64,
        base,
        drop_frame: false,
    })
}

fn date(h: &[u8]) -> Option<String> {
    let [d, mo, y, c] = [0, 1, 2, 3].map(|i| bcd(h[0x17c + i]));
    let [_, s, mi, hh] = [0, 1, 2, 3].map(|i| bcd(h[0x180 + i]));
    let year = c? * 100 + y?;
    let (d, mo) = (d?, mo?);
    if !(1..=31).contains(&d) || !(1..=12).contains(&mo) || year < 2000 {
        return None;
    }
    Some(match (hh, mi, s) {
        (Some(hh), Some(mi), Some(s)) if hh < 24 && mi < 60 && s < 60 => {
            display::stamp((year as u16, mo as u8, d as u8, hh as u8, mi as u8, s as u8))
        }
        _ => format!("{year:04}-{mo:02}-{d:02}"),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn header_timecode_and_date() {
        let mut h = vec![0xffu8; HEADER_LEN];
        h[0x1a8..0x1ac].copy_from_slice(&[0x11, 0x27, 0x48, 0x03]);
        h[0x17c..0x184].copy_from_slice(&[0x25, 0x09, 0x19, 0x20, 0x00, 0x00, 0x06, 0x13]);
        let tc = timecode(&h, Rational { num: 24, den: 1 }).unwrap();
        assert_eq!(
            (tc.frames, tc.base),
            ((3 * 3600 + 48 * 60 + 27) * 24 + 11, 24)
        );
        assert_eq!(date(&h).as_deref(), Some("2019-09-25 13:06:00"));
        h[0x1a8] = 0x99; // 99 frames at 24 fps is not a timecode
        assert!(timecode(&h, Rational { num: 24, den: 1 }).is_none());
        assert!(date(&[0xff; HEADER_LEN]).is_none());
    }
}
