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
    let start_tc = fps.and_then(|r| timecode(&h, r));
    let (frame_count, warnings) = extent(&files, start_tc, fps);
    let meta = ClipMeta {
        recording_date: date(&h),
        start_timecode: start_tc,
        // Project rate: the sensor rate (the only sample records both alike).
        frame_rate: fps,
        frame_count: Some(frame_count),
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
        texture: None,
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
        warnings,
        embedded_lut: None,
    })
}

/// The clip's length in frames, from TC in to the last frame's own timecode (or
/// its file number), and a warning naming any frames missing on disk.
fn extent(
    files: &[std::path::PathBuf],
    start_tc: Option<Timecode>,
    fps: Option<Rational>,
) -> (u64, Vec<String>) {
    let on_disk = files.len() as u64;
    let numbers: Option<Vec<u64>> = files.iter().map(|f| frame_number(f)).collect();
    let end_tc = files.last().filter(|_| files.len() > 1).and_then(|f| {
        let mut h = vec![0u8; HEADER_LEN];
        std::fs::File::open(f)
            .and_then(|mut f| f.read_exact(&mut h))
            .ok()?;
        timecode(&h, fps?)
    });
    let span = match (start_tc, end_tc, &numbers) {
        (Some(a), Some(b), _) if b.frames >= a.frames && b.base == a.base => {
            b.frames - a.frames + 1
        }
        (_, _, Some(n)) if n.len() > 1 => n[n.len() - 1].saturating_sub(n[0]) + 1,
        _ => on_disk,
    }
    .max(on_disk);
    if span == on_disk {
        return (span, Vec::new());
    }
    let width = files
        .first()
        .and_then(|f| f.file_stem())
        .map(|s| {
            s.to_string_lossy()
                .bytes()
                .rev()
                .take_while(u8::is_ascii_digit)
                .count()
        })
        .unwrap_or(0);
    let mut ranges: Vec<String> = numbers
        .unwrap_or_default()
        .windows(2)
        .filter(|w| w[1] > w[0] + 1)
        .map(|w| match (w[0] + 1, w[1] - 1) {
            (a, b) if a == b => format!("{a:0width$}"),
            (a, b) => format!("{a:0width$}–{b:0width$}"),
        })
        .collect();
    if ranges.len() > 3 {
        let more = ranges.len() - 3;
        ranges.truncate(3);
        ranges.push(format!("{more} more"));
    }
    let missing = span - on_disk;
    let warning = if ranges.is_empty() {
        format!("Frames missing from the sequence: {missing} of {span}")
    } else {
        format!(
            "Frames missing from the sequence: {} ({missing} of {span})",
            ranges.join(", ")
        )
    };
    (span, vec![warning])
}

/// The frame number at the end of a frame file's name: `….0078471.ari` → 78471.
fn frame_number(path: &std::path::Path) -> Option<u64> {
    let stem = path.file_stem()?.to_string_lossy();
    let digits = stem.bytes().rev().take_while(u8::is_ascii_digit).count();
    stem[stem.len() - digits..].parse().ok()
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

    /// A 24 fps `.ari` frame whose header timecode is `frames`.
    fn frame(dir: &std::path::Path, number: u64, frames: u32) -> std::path::PathBuf {
        let mut h = vec![0u8; HEADER_LEN + 16];
        h[..8].copy_from_slice(b"ARRI4Vx");
        h[0x14..0x18].copy_from_slice(&16u32.to_le_bytes());
        h[0x18..0x1c].copy_from_slice(&16u32.to_le_bytes());
        h[0x1a0..0x1a4].copy_from_slice(&24_000u32.to_le_bytes());
        let bcd = |v: u32| (((v / 10) << 4) | (v % 10)) as u8;
        let (f, s) = (frames % 24, frames / 24);
        h[0x1a8..0x1ac].copy_from_slice(&[bcd(f), bcd(s % 60), bcd(s / 60 % 60), bcd(s / 3600)]);
        let path = dir.join(format!("A001C001_120101_R1AB.{number:07}.ari"));
        std::fs::write(&path, h).unwrap();
        path
    }

    fn sequence(name: &str, numbers: &[u64]) -> ClipFiles {
        let dir = std::env::temp_dir().join(format!("luna-ari-{}-{name}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let mut files: Vec<_> = numbers.iter().map(|&n| frame(&dir, n, n as u32)).collect();
        let main = files.remove(0);
        ClipFiles {
            container: crate::types::Container::AriFrame,
            main,
            segments: files,
            sidecars: vec![],
        }
    }

    #[test]
    fn whole_sequence_has_no_warning() {
        let p = probe(&sequence("whole", &[100, 101, 102, 103])).unwrap();
        assert_eq!(p.meta.frame_count, Some(4));
        assert!(p.warnings.is_empty(), "{:?}", p.warnings);
    }

    #[test]
    fn missing_frames_keep_the_span_and_are_named() {
        let p = probe(&sequence("gap", &[100, 101, 102, 110, 111, 115])).unwrap();
        // TC in 100, TC out 115: the take is 16 frames, 6 of them on disk.
        assert_eq!(p.meta.start_timecode.unwrap().frames, 100);
        assert_eq!(p.meta.frame_count, Some(16));
        assert_eq!(
            p.warnings,
            ["Frames missing from the sequence: 0000103–0000109, 0000112–0000114 (10 of 16)"]
        );
        let FrameIndex::Files(f) = &p.frames else {
            panic!("{:?}", p.frames)
        };
        assert_eq!(f.len(), 6);
    }

    #[test]
    fn one_missing_frame_is_named_alone() {
        let p = probe(&sequence("one", &[7, 9])).unwrap();
        assert_eq!(p.meta.frame_count, Some(3));
        assert_eq!(
            p.warnings,
            ["Frames missing from the sequence: 0000008 (1 of 3)"]
        );
    }

    #[test]
    fn many_gaps_list_three_and_count_the_rest() {
        let p = probe(&sequence("many", &[0, 2, 4, 6, 8, 10])).unwrap();
        assert_eq!(
            p.warnings,
            ["Frames missing from the sequence: 0000001, 0000003, 0000005, 2 more (5 of 11)"]
        );
    }
}
