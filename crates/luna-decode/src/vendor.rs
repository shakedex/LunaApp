//! Clip metadata for containers only a vendor decoder can read: Canon Cinema
//! RAW Light (our `crm` crate), Blackmagic RAW (Blackmagic's SDK), and R3D and
//! Nikon N-RAW (the R3D add-on). `luna-formats` handles everything else.

use luna_formats::{
    Audio, ClipFiles, ClipMeta, Codec, ColourHint, Container, EmbeddedLut, FormatError, FrameIndex,
    PictureLayout, Probe, Rational, Timecode,
};

use crate::r3d::{self, Value};
use crate::{DecodeError, braw_sdk, util};

/// Reads clip metadata through a vendor decoder. `Some` for `.CRM`, `.braw`,
/// `.R3D` and `.NEV` clips, `None` for every container `luna-formats` reads itself.
/// Never panics.
pub fn vendor_probe(clip: &ClipFiles) -> Option<Result<Probe, FormatError>> {
    let probe: fn(&ClipFiles) -> Result<Probe, FormatError> = match clip.container {
        Container::Crm => probe_crm,
        Container::Braw => probe_braw,
        Container::R3d | Container::Nev => probe_r3d,
        _ => return None,
    };
    Some(
        util::guard("metadata", || Ok(probe(clip)))
            .unwrap_or_else(|e| Err(FormatError::Corrupt(e.to_string()))),
    )
}

fn to_format(e: DecodeError) -> FormatError {
    match e {
        DecodeError::Unsupported(m) => FormatError::Unsupported(m),
        DecodeError::Failed(m) => FormatError::Corrupt(m),
    }
}

/// The 3D LUT a Blackmagic camera embedded in the clip: `size³` RGB rows of
/// little-endian `f32`, stored blue fastest (checked on a Rec.709 LUT from a
/// Pocket 6K Pro: full red only lands on red output in that order), reordered
/// here to `.cube` order. Applied whatever `post_3dlut_mode` says: the mode
/// only sets what post tools do by default.
fn braw_embedded_lut(md: &braw::Metadata) -> Option<EmbeddedLut> {
    let size = md.uint("post_3dlut_embedded_size")? as usize;
    let Some(braw::Value::Array(braw::Array::U8(data))) = md.get("post_3dlut_embedded_data") else {
        return None;
    };
    if size < 2 || data.len() != size * size * size * 3 * 4 {
        tracing::warn!(
            size,
            bytes = data.len(),
            "embedded LUT has the wrong size, ignored"
        );
        return None;
    }
    Some(EmbeddedLut {
        name: md
            .text("post_3dlut_embedded_name")
            .unwrap_or_else(|| "Embedded LUT".into()),
        size,
        data: blue_fastest_to_cube(data, size),
    })
}

/// Reorders `n³` 12-byte rows from blue-fastest to red-fastest.
fn blue_fastest_to_cube(data: &[u8], n: usize) -> Vec<u8> {
    let mut out = Vec::with_capacity(data.len());
    for b in 0..n {
        for g in 0..n {
            for r in 0..n {
                let i = ((r * n + g) * n + b) * 12;
                out.extend_from_slice(&data[i..i + 12]);
            }
        }
    }
    out
}

fn probe_of(codec: Codec, meta: ClipMeta, layout: PictureLayout) -> Probe {
    Probe {
        codec,
        meta,
        frames: FrameIndex::DecoderOwned,
        layout,
        codec_config: None,
        warnings: Vec::new(),
        embedded_lut: None,
    }
}

// ---------------------------------------------------------------- CRM

fn probe_crm(clip: &ClipFiles) -> Result<Probe, FormatError> {
    let path = util::path_str(&clip.main).map_err(to_format)?;
    let c = crm::Clip::open(path).map_err(|e| match e {
        crm::Error::Io(e) => FormatError::Io(e.to_string()),
        crm::Error::Unsupported(m) => FormatError::Unsupported(m),
        other => FormatError::Corrupt(other.to_string()),
    })?;
    let m = c.metadata();
    let fm = c.frame_metadata(0).ok().unwrap_or_default();
    let active = c.active();
    let (scale, dur) = c.frame_rate();
    let frame_rate = rational(scale, dur);
    let start_timecode = c.index().tmcd.map(|t| Timecode {
        frames: t.start as u64,
        base: t.frames_per_second.max(1),
        drop_frame: t.flags & 1 != 0,
    });
    let meta = ClipMeta {
        slate: Default::default(),
        recording_date: m.capture_time.as_deref().map(exif_date),
        start_timecode,
        frame_rate,
        frame_count: Some(c.frame_count() as u64),
        width: Some(active.width),
        height: Some(active.height),
        camera_make: m.make.clone(),
        camera_model: m.model.clone(),
        camera_serial: m.body_serial.clone(),
        lens: m.lens_model.clone(),
        focal_length: fm
            .focal_length
            .or(m.focal_length)
            .and_then(|(n, d)| ratio(n, d))
            .map(|v| format!("{} mm", trim_num(v, 1))),
        stop: fm
            .f_number
            .or(m.f_number)
            .and_then(|(n, d)| ratio(n, d))
            .map(|v| format!("f/{}", trim_num(v, 1))),
        iso: fm.iso.or(m.iso).map(|v| v.to_string()),
        shutter: crm_shutter(&fm)
            .or_else(|| m.exposure_time.and_then(|(n, d)| exposure_time(n, d))),
        white_balance: fm.color_temperature.map(|k| format!("{k} K")),
        nd: None,
        sensor_fps: None,
        audio: None,
        embedded_reel: None,
        embedded_camera_id: None,
        // The file doesn't say which gamma/gamut the camera monitored in;
        // the frames are raw anyway.
        colour: ColourHint::default(),
    };
    let cmp = c.codec();
    let layout = PictureLayout {
        rotation: 0,
        stored_width: cmp.width,
        stored_height: cmp.height,
        raw_bits: u8::try_from(cmp.bits).ok(),
        pixel_aspect: None,
    };
    Ok(probe_of(Codec::CinemaRawLight, meta, layout))
}

fn crm_shutter(fm: &crm::FrameMetadata) -> Option<String> {
    let (n, d) = fm.shutter?;
    match fm.shutter_mode? {
        crm::metadata::ShutterMode::Angle => ratio(n, d).map(|v| format!("{}°", trim_num(v, 1))),
        crm::metadata::ShutterMode::Speed => exposure_time(n, d),
    }
}

// ---------------------------------------------------------------- BRAW

fn probe_braw(clip: &ClipFiles) -> Result<Probe, FormatError> {
    let sdk = braw_sdk::sdk().map_err(to_format)?;
    let c = sdk.open(&clip.main).map_err(|e| match e {
        braw::Error::Io(m) => FormatError::Io(m),
        braw::Error::SdkNotFound(m) | braw::Error::SdkLoad(m) => FormatError::Unsupported(m),
        other => FormatError::Corrupt(other.to_string()),
    })?;
    let info = c.info().clone();
    let f0 = c.frame_info(0).unwrap_or_default();
    let frame_rate = info.frame_rate.map(|r| Rational {
        num: r.num,
        den: r.den,
    });
    let start_timecode = info
        .start_timecode
        .as_deref()
        .and_then(|t| parse_timecode(t, frame_rate));
    let meta = ClipMeta {
        slate: Default::default(),
        recording_date: info.date_recorded.as_deref().map(exif_date),
        start_timecode,
        frame_rate,
        frame_count: Some(info.frame_count),
        width: Some(info.width),
        height: Some(info.height),
        camera_make: info.manufacturer.clone(),
        camera_model: info.camera_type.clone(),
        // The SDK's camera id is a per-install UUID, not the body serial.
        camera_serial: info.camera_id.clone().filter(|id| !looks_like_uuid(id)),
        lens: info.lens_type.clone(),
        focal_length: f0.focal_length.as_deref().map(spaced_mm),
        stop: f0.aperture.as_deref().map(braw_stop),
        iso: f0.iso.map(|v| v.to_string()),
        shutter: f0.shutter.clone(),
        white_balance: f0.white_balance_kelvin.map(|k| format!("{k} K")),
        nd: f0.internal_nd.clone().filter(|nd| !is_zero(nd)),
        sensor_fps: f0.sensor_rate.and_then(|r| rational(r.num, r.den)),
        audio: None,
        embedded_reel: info.reel_name.clone(),
        embedded_camera_id: info.camera_number.clone(),
        colour: braw_sdk::HINT,
    };
    let layout = PictureLayout {
        rotation: 0,
        stored_width: info.width,
        stored_height: info.height,
        raw_bits: None,
        pixel_aspect: None,
    };
    let mut probe = probe_of(Codec::Braw, meta, layout);
    probe.embedded_lut = braw_embedded_lut(c.raw_metadata());
    Ok(probe)
}

/// "f2.8" → "f/2.8"; T-stops and anything else pass through.
fn braw_stop(s: &str) -> String {
    match s.strip_prefix('f').or_else(|| s.strip_prefix('F')) {
        Some(rest) if rest.parse::<f64>().is_ok() => format!("f/{rest}"),
        _ => s.to_string(),
    }
}

/// "33mm" → "33 mm".
fn spaced_mm(s: &str) -> String {
    match s.strip_suffix("mm") {
        Some(n) if !n.ends_with(' ') => format!("{n} mm"),
        _ => s.to_string(),
    }
}

fn is_zero(s: &str) -> bool {
    s.trim().parse::<f64>().map(|v| v == 0.0).unwrap_or(false)
}

// ---------------------------------------------------------------- R3D

fn probe_r3d(clip: &ClipFiles) -> Result<Probe, FormatError> {
    let addon = r3d::addon().map_err(to_format)?;
    let c = addon.open(&clip.main).map_err(to_format)?;
    let info = c.info().map_err(to_format)?;
    let md = c.metadata().map_err(to_format)?;
    let text = |k: &str| {
        md.iter()
            .find(|(key, _)| key == k)
            .and_then(|(_, v)| match v {
                Value::Text(s) if !s.trim().is_empty() => Some(s.trim().to_string()),
                _ => None,
            })
    };
    let int = |k: &str| {
        md.iter()
            .find(|(key, _)| key == k)
            .and_then(|(_, v)| match v {
                Value::Int(i) => Some(*i),
                _ => None,
            })
    };
    let float = |k: &str| {
        md.iter()
            .find(|(key, _)| key == k)
            .and_then(|(_, v)| match v {
                Value::Float(f) if f.is_finite() => Some(*f),
                Value::Int(i) => Some(*i as f32),
                _ => None,
            })
    };
    let frame_rate = match (int("framerate_numerator"), int("framerate_denominator")) {
        (Some(n), Some(d)) => rational(n, d),
        _ => fps_rational(info.video_fps),
    };
    let tc_rate = fps_rational(info.timecode_fps).or(frame_rate);
    let start_timecode = r3d_start_timecode(
        int("user_timecode_preference"),
        r3d::Info::text(&info.start_absolute_timecode),
        r3d::Info::text(&info.start_edge_timecode),
    )
    .and_then(|t| parse_timecode(&t, tc_rate));
    let recording_date = text("local_date").map(|d| {
        let date = compact_date(&d);
        match text("local_time").map(|t| compact_time(&t)) {
            Some(t) => format!("{date} {t}"),
            None => date,
        }
    });
    // RED's SDK also reads Nikon's N-RAW and R3D NE; `redcode` says which.
    let (codec, make) = match text("redcode").as_deref() {
        Some("N-RAW") => (Codec::NRaw, "Nikon"),
        Some("R3D NE") => (Codec::R3dNe, "Nikon"),
        _ => (Codec::Redcode, "RED"),
    };
    let camera_model = text("camera_model").map(|m| match m.get(..6) {
        // Nikon writes "NIKON Z6_3"; the make is its own field.
        Some(p) if make == "Nikon" && p.eq_ignore_ascii_case("nikon ") => m[6..].to_string(),
        _ => m,
    });
    let lens = match (text("lens_brand"), text("lens_name")) {
        (Some(b), Some(n)) if !n.starts_with(&b) => Some(format!("{b} {n}")),
        (_, Some(n)) => Some(n),
        (b, None) => b,
    };
    let meta = ClipMeta {
        slate: Default::default(),
        recording_date,
        start_timecode,
        frame_rate,
        frame_count: Some(info.frame_count),
        width: Some(info.width),
        height: Some(info.height),
        camera_make: Some(make.into()),
        camera_model,
        camera_serial: text("camera_pin").or_else(|| text("camera_serial_number")),
        lens,
        focal_length: float("lens_focal_length")
            .filter(|v| *v > 0.0)
            .map(|v| format!("{} mm", trim_num(v as f64, 1))),
        stop: r3d_stop(int("lens_aperture"), int("lens_aperture_units")),
        iso: int("iso").map(|v| v.to_string()),
        shutter: float("shutter_degrees")
            .filter(|v| *v > 0.0)
            .map(|v| format!("{}°", trim_num(v as f64, 1))),
        white_balance: float("white_balance_kelvin")
            .filter(|v| *v > 0.0)
            .map(|v| format!("{} K", v.round() as u32)),
        nd: float("nd_stops")
            .filter(|v| *v > 0.0)
            .map(|v| trim_num(v as f64 * 0.3, 1)),
        // RED's record rate is the rate the sensor ran at.
        sensor_fps: match (
            int("record_framerate_numerator"),
            int("record_framerate_denominator"),
        ) {
            (Some(n), Some(d)) => rational(n, d),
            _ => None,
        },
        audio: r3d_audio(
            int("channel_mask"),
            int("samplerate"),
            int("sample_size"),
            text("wav_filename").is_some(),
        ),
        embedded_reel: text("reel_id_full"),
        embedded_camera_id: text("camera_id"),
        colour: r3d::HINT,
    };
    let pixel_aspect = float("pixel_aspect_ratio")
        .filter(|p| *p > 0.0 && (p - 1.0).abs() > 1e-3)
        .and_then(|p| rational((p * 1000.0).round() as u32, 1000));
    let layout = PictureLayout {
        rotation: 0,
        stored_width: info.width,
        stored_height: info.height,
        raw_bits: None,
        pixel_aspect,
    };
    let mut probe = probe_of(codec, meta, layout);
    if info.video_tracks > 1 {
        probe
            .warnings
            .push("HDRx clip: thumbnails show the A track".into());
    }
    Ok(probe)
}

/// RED's `lens_aperture` is the stop × 10 (0xFFFF = fully closed);
/// `lens_aperture_units` 1 means a T-stop, anything else an f-stop.
fn r3d_stop(aperture: Option<u32>, units: Option<u32>) -> Option<String> {
    let a = aperture.filter(|a| *a > 0 && *a != 0xFFFF)?;
    let prefix = if units == Some(1) { "T" } else { "f/" };
    Some(format!("{prefix}{}", trim_num(f64::from(a) / 10.0, 1)))
}

/// The start timecode on the track the camera was set to show
/// (`user_timecode_preference`: 0 = edge / run record, 1 = absolute / external).
/// Absolute when the preference is missing or unknown; the other track when the
/// preferred one is empty.
fn r3d_start_timecode(
    preference: Option<u32>,
    absolute: Option<String>,
    edge: Option<String>,
) -> Option<String> {
    match preference {
        Some(0) => edge.or(absolute),
        _ => absolute.or(edge),
    }
}

/// Embedded audio from RED's audio keys, which exist only when the clip has
/// embedded audio (`channel_mask` bit n = channel n+1). Without them the clip
/// has no embedded sound, unless the camera recorded a separate WAV
/// (`wav_filename`): then the sound exists elsewhere, so it's unknown here.
fn r3d_audio(
    channel_mask: Option<u32>,
    sample_rate: Option<u32>,
    bits: Option<u32>,
    separate_wav: bool,
) -> Option<Audio> {
    match channel_mask {
        Some(mask) => Some(Audio {
            channels: mask.count_ones(),
            sample_rate,
            bits,
        }),
        None if separate_wav => None,
        None => Some(Audio {
            channels: 0,
            sample_rate: None,
            bits: None,
        }),
    }
}

// ---------------------------------------------------------------- helpers

fn rational(num: u32, den: u32) -> Option<Rational> {
    if num == 0 || den == 0 {
        return None;
    }
    let (mut a, mut b) = (num, den);
    while b != 0 {
        (a, b) = (b, a % b);
    }
    Some(Rational {
        num: num / a,
        den: den / a,
    })
}

/// A float rate as an exact fraction, snapping NTSC rates to ×1000/1001.
fn fps_rational(fps: f32) -> Option<Rational> {
    if !fps.is_finite() || fps <= 0.0 {
        return None;
    }
    let fps = fps as f64;
    let ntsc = (fps * 1.001).round();
    if fps.fract() != 0.0 && (ntsc * 1000.0 / 1001.0 - fps).abs() < 0.001 {
        return rational(ntsc as u32 * 1000, 1001);
    }
    rational((fps * 1000.0).round() as u32, 1000)
}

fn ratio(n: u32, d: u32) -> Option<f64> {
    (d != 0).then(|| n as f64 / d as f64)
}

/// A number with at most `places` decimals, trailing zeros trimmed.
fn trim_num(v: f64, places: usize) -> String {
    let s = format!("{v:.places$}");
    if s.contains('.') {
        s.trim_end_matches('0').trim_end_matches('.').to_string()
    } else {
        s
    }
}

/// An exposure time as "1/50" (or "2.5 s" when longer than a second).
fn exposure_time(n: u32, d: u32) -> Option<String> {
    let v = ratio(n, d)?;
    if v <= 0.0 {
        return None;
    }
    if v >= 1.0 {
        return Some(format!("{} s", trim_num(v, 1)));
    }
    Some(format!("1/{}", trim_num(1.0 / v, 0)))
}

/// "YYYY:MM:DD[ HH:MM:SS]" → "YYYY-MM-DD[ HH:MM:SS]".
fn exif_date(s: &str) -> String {
    let s = s.trim();
    let (date, time) = s.split_once(' ').unwrap_or((s, ""));
    let date = date.replace(':', "-");
    if time.is_empty() {
        date
    } else {
        format!("{date} {time}")
    }
}

/// "20250819" → "2025-08-19".
fn compact_date(s: &str) -> String {
    let s = s.trim();
    if s.len() == 8 && s.bytes().all(|b| b.is_ascii_digit()) {
        format!("{}-{}-{}", &s[..4], &s[4..6], &s[6..])
    } else {
        s.to_string()
    }
}

/// "142530" → "14:25:30".
fn compact_time(s: &str) -> String {
    let s = s.trim();
    if s.len() == 6 && s.bytes().all(|b| b.is_ascii_digit()) {
        format!("{}:{}:{}", &s[..2], &s[2..4], &s[4..])
    } else {
        s.to_string()
    }
}

/// "HH:MM:SS:FF" (';' before frames for drop-frame) at `rate`.
fn parse_timecode(s: &str, rate: Option<Rational>) -> Option<Timecode> {
    let s = s.trim();
    let drop_frame = s.contains(';');
    let parts: Vec<u64> = s
        .split([':', ';', '.'])
        .map(|p| p.parse().ok())
        .collect::<Option<_>>()?;
    let [hh, mm, ss, ff] = parts[..] else {
        return None;
    };
    let base = rate.map(|r| r.as_f64().round() as u32).filter(|&b| b > 0)?;
    // Rates above 30 often count frame pairs in the timecode's frame field;
    // trust the field when it fits the rate.
    if ff >= base as u64 || mm >= 60 || ss >= 60 {
        return None;
    }
    let b = base as u64;
    let nominal = (hh * 3600 + mm * 60 + ss) * b + ff;
    let frames = if drop_frame && (base == 30 || base == 60) {
        let drop = if base == 30 { 2 } else { 4 };
        let total_min = hh * 60 + mm;
        nominal - drop * (total_min - total_min / 10)
    } else {
        nominal
    };
    Some(Timecode {
        frames,
        base,
        drop_frame,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn files(container: Container) -> ClipFiles {
        ClipFiles {
            container,
            main: PathBuf::from("Z:/missing/clip"),
            segments: vec![],
            sidecars: vec![],
        }
    }

    #[test]
    fn only_vendor_containers() {
        assert!(vendor_probe(&files(Container::Mxf)).is_none());
        assert!(vendor_probe(&files(Container::QuickTime)).is_none());
        assert!(vendor_probe(&files(Container::AriFrame)).is_none());
        for c in [
            Container::Crm,
            Container::Braw,
            Container::R3d,
            Container::Nev,
        ] {
            assert!(matches!(vendor_probe(&files(c)), Some(Err(_))), "{c:?}");
        }
    }

    #[test]
    fn r3d_start_timecode_follows_the_cameras_timecode_preference() {
        let abs = Some("15:43:20:20".to_string());
        let edge = Some("03:33:58:06".to_string());
        // RMD_USER_TIMECODE_PREFERENCE: 0 = edge / run record, 1 = absolute / external.
        assert_eq!(r3d_start_timecode(Some(0), abs.clone(), edge.clone()), edge);
        assert_eq!(r3d_start_timecode(Some(1), abs.clone(), edge.clone()), abs);
        // Missing or unknown preference: absolute, which RED guarantees to exist.
        assert_eq!(r3d_start_timecode(None, abs.clone(), edge.clone()), abs);
        assert_eq!(r3d_start_timecode(Some(7), abs.clone(), edge.clone()), abs);
        // The preferred track is empty: fall back to the other one.
        assert_eq!(r3d_start_timecode(Some(0), abs.clone(), None), abs);
    }

    #[test]
    fn r3d_stop_is_tenths_with_units() {
        assert_eq!(r3d_stop(Some(25), Some(0)).as_deref(), Some("f/2.5"));
        assert_eq!(r3d_stop(Some(56), None).as_deref(), Some("f/5.6"));
        assert_eq!(r3d_stop(Some(20), Some(1)).as_deref(), Some("T2"));
        assert_eq!(r3d_stop(Some(0xFFFF), Some(0)), None);
        assert_eq!(r3d_stop(Some(0), Some(0)), None);
        assert_eq!(r3d_stop(None, Some(0)), None);
    }

    #[test]
    fn r3d_audio_from_embedded_audio_keys() {
        // channel_mask bit n = channel n+1 enabled.
        let a = r3d_audio(Some(0b0011), Some(48000), Some(24), false).unwrap();
        assert_eq!(
            (a.channels, a.sample_rate, a.bits),
            (2, Some(48000), Some(24))
        );
        assert_eq!(
            r3d_audio(Some(0b1111), None, None, false).unwrap().channels,
            4
        );
        // RED writes these keys only when the clip has embedded audio.
        assert_eq!(r3d_audio(None, None, None, false).unwrap().channels, 0);
        // No embedded audio but the camera recorded a separate WAV: unknown, not "None".
        assert_eq!(r3d_audio(None, None, None, true), None);
    }

    #[test]
    fn timecodes_parse() {
        let r24 = Some(Rational { num: 24, den: 1 });
        let tc = parse_timecode("09:15:35:00", r24).unwrap();
        assert_eq!(tc.frames, (9 * 3600 + 15 * 60 + 35) * 24);
        assert_eq!(tc.to_string(), "09:15:35:00");
        let ntsc = Some(Rational {
            num: 30000,
            den: 1001,
        });
        let df = parse_timecode("01:10:00;02", ntsc).unwrap();
        assert!(df.drop_frame);
        assert_eq!(df.to_string(), "01:10:00;02");
        assert_eq!(parse_timecode("garbage", r24), None);
        assert_eq!(parse_timecode("00:00:00:30", r24), None);
        assert_eq!(parse_timecode("00:00:00:00", None), None);
    }

    #[test]
    fn rates() {
        assert_eq!(
            fps_rational(23.976),
            Some(Rational {
                num: 24000,
                den: 1001
            })
        );
        assert_eq!(fps_rational(25.0), Some(Rational { num: 25, den: 1 }));
        assert_eq!(fps_rational(0.0), None);
        assert_eq!(rational(50, 1), Some(Rational { num: 50, den: 1 }));
        assert_eq!(
            rational(24000, 1001),
            Some(Rational {
                num: 24000,
                den: 1001
            })
        );
        assert_eq!(rational(1000, 40), Some(Rational { num: 25, den: 1 }));
    }

    #[test]
    fn text_formats() {
        assert_eq!(exif_date("2023:09:15"), "2023-09-15");
        assert_eq!(exif_date("2024:10:03 18:07:57"), "2024-10-03 18:07:57");
        assert_eq!(compact_date("20250819"), "2025-08-19");
        assert_eq!(compact_time("142530"), "14:25:30");
        assert_eq!(exposure_time(1, 50).as_deref(), Some("1/50"));
        assert_eq!(exposure_time(10, 480).as_deref(), Some("1/48"));
        assert_eq!(exposure_time(5, 2).as_deref(), Some("2.5 s"));
        assert_eq!(trim_num(2.80, 1), "2.8");
        assert_eq!(trim_num(4.0, 1), "4");
        assert_eq!(braw_stop("f2.8"), "f/2.8");
        assert_eq!(braw_stop("T2.1"), "T2.1");
        assert_eq!(spaced_mm("33mm"), "33 mm");
        assert!(is_zero("0"));
        assert!(!is_zero("0.6"));
    }
}

/// `1b4dda73-ef34-4cfc-909f-93eecfd8d0f4`-shaped.
fn looks_like_uuid(s: &str) -> bool {
    let parts: Vec<&str> = s.split('-').collect();
    parts.len() == 5
        && parts.iter().map(|p| p.len()).eq([8, 4, 4, 4, 12])
        && parts
            .iter()
            .all(|p| p.chars().all(|c| c.is_ascii_hexdigit()))
}

#[cfg(test)]
mod uuid_tests {
    #[test]
    fn uuid_shape() {
        assert!(super::looks_like_uuid(
            "1b4dda73-ef34-4cfc-909f-93eecfd8d0f4"
        ));
        assert!(!super::looks_like_uuid("273102000359"));
        assert!(!super::looks_like_uuid("1b4dda73-ef34-4cfc-909f"));
    }
}

#[cfg(test)]
mod braw_lut_tests {
    use super::*;
    use braw::{Array, Metadata, Value as BValue};

    fn table_bytes(values: &[f32]) -> Vec<u8> {
        values.iter().flat_map(|v| v.to_le_bytes()).collect()
    }

    fn md(name: &str, size: u16, data: Vec<u8>) -> Metadata {
        Metadata(vec![
            (
                "post_3dlut_embedded_name".into(),
                BValue::String(name.into()),
            ),
            ("post_3dlut_embedded_size".into(), BValue::U16(size)),
            (
                "post_3dlut_embedded_data".into(),
                BValue::Array(Array::U8(data)),
            ),
        ])
    }

    #[test]
    fn embedded_lut_from_braw_bytes() {
        let values: Vec<f32> = (0..24).map(|i| i as f32 / 23.0).collect();
        let lut = braw_embedded_lut(&md("Show.cube", 2, table_bytes(&values))).unwrap();
        assert_eq!(lut.name, "Show.cube");
        assert_eq!(lut.size, 2);
        assert_eq!(lut.data.len(), values.len() * 4);
    }

    #[test]
    fn blackmagic_blue_fastest_rows_become_red_fastest() {
        // Blackmagic stores blue fastest: row index = (r·n + g)·n + b. A
        // table whose output names its input (r, g, b) must come out in .cube
        // order, where row index = (b·n + g)·n + r.
        let n = 2;
        let mut bm = Vec::new();
        for r in 0..n {
            for g in 0..n {
                for b in 0..n {
                    bm.extend([r as f32, g as f32, b as f32]);
                }
            }
        }
        let lut = braw_embedded_lut(&md("Show.cube", n as u16, table_bytes(&bm))).unwrap();
        let rows: Vec<f32> = lut
            .data
            .chunks(4)
            .map(|b| f32::from_le_bytes(b.try_into().unwrap()))
            .collect();
        let mut cube = Vec::new();
        for b in 0..n {
            for g in 0..n {
                for r in 0..n {
                    cube.extend([r as f32, g as f32, b as f32]);
                }
            }
        }
        assert_eq!(rows, cube);
    }

    #[test]
    fn wrong_length_embedded_lut_is_ignored() {
        assert!(braw_embedded_lut(&md("Show.cube", 2, vec![0; 95])).is_none());
    }

    #[test]
    fn no_embedded_lut_is_none() {
        assert!(braw_embedded_lut(&Metadata(vec![])).is_none());
    }
}
