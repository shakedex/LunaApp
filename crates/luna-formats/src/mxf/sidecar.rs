//! XML sidecars: Sony `…M01.XML` (NonRealTimeMeta) and Canon clip XML.

use crate::types::Timecode;

/// Largest sidecar we read; real ones are a few KiB.
pub(crate) const MAX_XML: u64 = 4 << 20;

#[derive(Debug, Default, Clone, PartialEq)]
pub(crate) struct SonyXml {
    pub manufacturer: Option<String>,
    pub model: Option<String>,
    pub serial: Option<String>,
    pub start_tc: Option<Timecode>,
    /// `CreationDate` as written (ISO 8601, possibly with `Z` or an offset).
    pub creation: Option<String>,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub iris_t: Option<f64>,
    pub iris_f: Option<f64>,
    pub focal_mm: Option<f64>,
    /// `LensAttributes`: the lens's identifier code (cine lenses record no name).
    pub lens: Option<String>,
    /// `<Lens modelName>`: the lens name, when the lens reports one.
    pub lens_model: Option<String>,
    pub exposure_index: Option<String>,
    pub iso: Option<String>,
    pub nd: Option<String>,
    pub shutter_angle: Option<f64>,
    pub white_balance: Option<String>,
    pub capture_gamma: Option<String>,
    pub capture_primaries: Option<String>,
    pub look_gamma: Option<String>,
    pub look_primaries: Option<String>,
    pub camera_attributes: Option<String>,
}

fn attr(n: roxmltree::Node, name: &str) -> Option<String> {
    n.attribute(name)
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
}

fn number_prefix(s: &str) -> Option<f64> {
    let end = s
        .find(|c: char| !(c.is_ascii_digit() || c == '.' || c == '-'))
        .unwrap_or(s.len());
    s[..end].parse().ok()
}

/// Sony LTC value "FFSSMMHH" (BCD digits, byte order frames first).
fn sony_ltc(value: &str, fps: u32) -> Option<Timecode> {
    if value.len() != 8 || fps == 0 {
        return None;
    }
    let byte = |i: usize| u8::from_str_radix(&value[2 * i..2 * i + 2], 16).ok();
    let (f, s, m, h) = (byte(0)?, byte(1)?, byte(2)?, byte(3)?);
    let bcd = |b: u8| (b >> 4) as u64 * 10 + (b & 0x0f) as u64;
    let drop = f & 0x40 != 0;
    let (ff, ss, mm, hh) = (bcd(f & 0x3f), bcd(s & 0x7f), bcd(m & 0x7f), bcd(h & 0x3f));
    if ff >= fps as u64 || ss > 59 || mm > 59 || hh > 23 {
        return None;
    }
    Some(timecode_from_hmsf(hh, mm, ss, ff, fps, drop))
}

/// Frame count of a displayed timecode, undoing drop-frame numbering.
pub(crate) fn timecode_from_hmsf(
    hh: u64,
    mm: u64,
    ss: u64,
    ff: u64,
    base: u32,
    drop: bool,
) -> Timecode {
    let b = base as u64;
    let mut frames = ((hh * 60 + mm) * 60 + ss) * b + ff;
    if drop && (base == 30 || base == 60) {
        let d = if base == 30 { 2 } else { 4 };
        let total_min = hh * 60 + mm;
        frames -= d * (total_min - total_min / 10);
    }
    Timecode {
        frames,
        base,
        drop_frame: drop,
    }
}

pub(crate) fn parse_sony(xml: &str) -> Option<SonyXml> {
    let doc = roxmltree::Document::parse(xml).ok()?;
    let root = doc.root_element();
    if root.tag_name().name() != "NonRealTimeMeta" {
        return None;
    }
    let mut x = SonyXml::default();
    for n in root.descendants().filter(|n| n.is_element()) {
        match n.tag_name().name() {
            "Device" => {
                x.manufacturer = attr(n, "manufacturer");
                x.model = attr(n, "modelName");
                x.serial = attr(n, "serialNo");
            }
            "LtcChangeTable" => {
                let fps: u32 = n
                    .attribute("tcFps")
                    .and_then(|s| s.trim().parse().ok())
                    .unwrap_or(0);
                x.start_tc = n
                    .children()
                    .filter(|c| c.has_tag_name("LtcChange"))
                    .find(|c| c.attribute("frameCount") == Some("0"))
                    .and_then(|c| c.attribute("value"))
                    .and_then(|v| sony_ltc(v, fps));
            }
            "CreationDate" => x.creation = attr(n, "value"),
            "Lens" => x.lens_model = attr(n, "modelName"),
            "VideoLayout" => {
                x.width = n.attribute("pixel").and_then(|s| s.parse().ok());
                x.height = n
                    .attribute("numOfVerticalLine")
                    .and_then(|s| s.parse().ok());
            }
            "Item" => {
                let Some(v) = attr(n, "value") else { continue };
                match n.attribute("name").unwrap_or("") {
                    "IrisTNumber" => x.iris_t = number_prefix(&v),
                    "IrisFNumber" => x.iris_f = number_prefix(&v),
                    "LensZoomActualFocalLength" => x.focal_mm = number_prefix(&v),
                    "LensAttributes" => x.lens = Some(v),
                    "ExposureIndexOfPhotoMeter" => x.exposure_index = Some(v),
                    "ISOSensitivity" => x.iso = Some(v),
                    "NeutralDensityFilterWheelSetting" => x.nd = Some(v),
                    "ShutterSpeed_Angle" => x.shutter_angle = number_prefix(&v),
                    "WhiteBalance" => x.white_balance = Some(v),
                    "CaptureGammaEquation" => x.capture_gamma = Some(v),
                    "CaptureColorPrimaries" => x.capture_primaries = Some(v),
                    "GammaForLook" => x.look_gamma = Some(v),
                    "ColorForLook" => x.look_primaries = Some(v),
                    "CameraAttributes" => x.camera_attributes = Some(v),
                    _ => {}
                }
            }
            _ => {}
        }
    }
    Some(x)
}

#[derive(Debug, Default, Clone, PartialEq)]
pub(crate) struct CanonXml {
    pub manufacturer: Option<String>,
    pub model: Option<String>,
    pub serial: Option<String>,
    pub lens: Option<String>,
    /// `DateAndTime` as written (ISO 8601 with offset).
    pub date_time: Option<String>,
    /// "HH:MM:SS:FF" / "HH:MM:SS;FF".
    pub start_tc: Option<String>,
}

pub(crate) fn parse_canon(xml: &str) -> Option<CanonXml> {
    let doc = roxmltree::Document::parse(xml).ok()?;
    let mut x = CanonXml::default();
    let text = |n: roxmltree::Node| {
        n.text()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string)
    };
    for n in doc.descendants().filter(|n| n.is_element()) {
        let parent = n.parent().map(|p| p.tag_name().name()).unwrap_or("");
        match (parent, n.tag_name().name()) {
            ("Device", "Manufacturer") => x.manufacturer = text(n),
            ("Device", "ModelName") => x.model = text(n),
            ("Device", "SerialNo") => x.serial = text(n),
            ("Device", "Lens") => x.lens = text(n),
            ("TimeInformation", "DateAndTime") => x.date_time = text(n),
            ("TimeInformation", "StartTimecode") => x.start_tc = text(n),
            _ => {}
        }
    }
    Some(x)
}

/// "YYYY-MM-DDTHH:MM:SS(+hh:mm|Z)" → ("YYYY-MM-DD HH:MM:SS", has local offset).
pub(crate) fn iso_local(s: &str) -> Option<(String, bool)> {
    let s = s.trim();
    let date = s.get(..10)?;
    let ok_date = date.as_bytes().iter().enumerate().all(|(i, c)| match i {
        4 | 7 => *c == b'-',
        _ => c.is_ascii_digit(),
    });
    if !ok_date {
        return None;
    }
    let time = s.get(11..19).filter(|t| {
        t.as_bytes().iter().enumerate().all(|(i, c)| match i {
            2 | 5 => *c == b':',
            _ => c.is_ascii_digit(),
        })
    });
    let rest = s.get(19..).unwrap_or("");
    let rest = rest.trim_start_matches(|c: char| c == '.' || c.is_ascii_digit());
    let local = rest.starts_with('+') || rest.starts_with('-');
    let out = match time {
        Some(t) => format!("{date} {t}"),
        None => date.to_string(),
    };
    Some((out, local))
}

#[cfg(test)]
mod tests {
    use super::*;

    const SONY: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<NonRealTimeMeta xmlns="urn:schemas-professionalDisc:nonRealTimeMeta:ver.2.00">
  <Duration value="79"/>
  <LtcChangeTable tcFps="24">
    <LtcChange frameCount="0" status="increment" value="00595004"/>
  </LtcChangeTable>
  <CreationDate value="2020-11-02T01:39:05Z"/>
  <Device manufacturer="Sony" modelName="MPC-3610" serialNo="0010658"/>
  <Lens modelName="FE 50mm F1.4 ZA"/>
  <VideoFormat><VideoLayout pixel="6048" numOfVerticalLine="4032"/></VideoFormat>
  <AcquisitionRecord>
    <Group name="LensUnitMetadataSet">
      <Item name="IrisTNumber" value="7.77"/>
      <Item name="LensZoomActualFocalLength" value="86.00mm"/>
      <Item name="LensAttributes" value="F08000183"/>
    </Group>
    <Group name="CameraUnitMetadataSet">
      <Item name="ExposureIndexOfPhotoMeter" value="500"/>
      <Item name="NeutralDensityFilterWheelSetting" value="1"/>
      <Item name="ShutterSpeed_Angle" value="180.00deg"/>
      <Item name="WhiteBalance" value="6300"/>
      <Item name="CaptureGammaEquation" value="scene-linear"/>
    </Group>
  </AcquisitionRecord>
</NonRealTimeMeta>"#;

    #[test]
    fn sony_fields() {
        let x = parse_sony(SONY).unwrap();
        assert_eq!(x.start_tc.unwrap().to_string(), "04:50:59:00");
        assert_eq!(x.model.as_deref(), Some("MPC-3610"));
        assert_eq!(x.width, Some(6048));
        assert_eq!(x.iris_t, Some(7.77));
        assert_eq!(x.focal_mm, Some(86.0));
        assert_eq!(x.lens.as_deref(), Some("F08000183"));
        assert_eq!(x.lens_model.as_deref(), Some("FE 50mm F1.4 ZA"));
        assert_eq!(x.shutter_angle, Some(180.0));
        assert_eq!(x.capture_gamma.as_deref(), Some("scene-linear"));
        assert!(parse_sony("<x/>").is_none());
        assert!(parse_sony("not xml").is_none());
    }

    #[test]
    fn canon_fields() {
        let x = parse_canon(
            r#"<VideoClip xmlns="http://www.canon.com/ns/RawMovieClip"><Device><Manufacturer>CANON</Manufacturer><SerialNo></SerialNo><ModelName>EOS C70</ModelName></Device><TimeInformation><DateAndTime>2024-10-03T18:07:57+10:00</DateAndTime><StartTimecode>00:25:14:10</StartTimecode></TimeInformation></VideoClip>"#,
        )
        .unwrap();
        assert_eq!(x.model.as_deref(), Some("EOS C70"));
        assert_eq!(x.serial, None);
        assert_eq!(x.start_tc.as_deref(), Some("00:25:14:10"));
        assert_eq!(
            iso_local(x.date_time.as_deref().unwrap()),
            Some(("2024-10-03 18:07:57".into(), true))
        );
        assert_eq!(
            iso_local("2020-11-02T01:39:05Z"),
            Some(("2020-11-02 01:39:05".into(), false))
        );
    }

    #[test]
    fn drop_frame_timecode_round_trips() {
        let tc = timecode_from_hmsf(1, 0, 0, 2, 30, true);
        assert_eq!(tc.to_string(), "01:00:00;02");
    }
}
