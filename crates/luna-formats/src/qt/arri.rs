//! ARRI ProRes MOV (ALEXA Mini and family): `com.arri.camera.*` keys in the
//! movie-level `meta`. Key names and value encodings as ARRI writes them:
//! ShutterAngle in tenths of a degree, NdFilterDensity in thousandths,
//! UserDate "YYYYMMDD" + UserTime "HHhMMmSS" (camera wall clock).

use super::display;
use super::meta::Items;
use crate::types::{ClipMeta, ColourHint, Gamut, Transfer};

const NS: &str = "com.arri.camera.";

pub(crate) fn read(items: &Items) -> Option<ClipMeta> {
    if !items.has_prefix(NS) {
        return None;
    }
    let text = |k: &str| items.text(&format!("{NS}{k}"));
    let int = |k: &str| items.int(&format!("{NS}{k}"));

    let model = text("CameraModel");
    let mut m = ClipMeta {
        // ARRI writes into its own key namespace; the maker is ARRI by definition.
        camera_make: Some("ARRI".into()),
        camera_model: model.clone(),
        camera_serial: text("CameraSerialNumber"),
        lens: text("LensType").or_else(|| text("LensModel")),
        iso: int("ExposureIndexAsa")
            .filter(|&v| v > 0)
            .map(|v| v.to_string()),
        shutter: int("ShutterAngle").and_then(|v| display::shutter_angle(v as f64 / 10.0)),
        white_balance: int("WhiteBalanceKelvin").and_then(display::kelvin),
        nd: int("NdFilterDensity")
            .filter(|&v| v > 0)
            .map(|v| format!("{:.1}", v as f64 / 1000.0)),
        // Thousandths of a frame per second.
        sensor_fps: int("SensorFps")
            .and_then(|v| u32::try_from(v).ok())
            .and_then(|v| crate::types::Rational::reduced(v, 1000)),
        embedded_reel: text("ReelName"),
        embedded_camera_id: text("CameraIndex"),
        recording_date: date(text("UserDate"), text("UserTime")),
        ..ClipMeta::default()
    };
    if let Some(gamma) = text("ColorGammaSxS").or_else(|| text("ColorGamma")) {
        m.colour = colour(&gamma, model.as_deref());
    }
    Some(m)
}

fn colour(gamma: &str, model: Option<&str>) -> ColourHint {
    let g = gamma.to_ascii_uppercase().replace(['-', ' ', '_'], "");
    let alexa35 = model.is_some_and(|m| m.to_ascii_uppercase().contains("ALEXA 35"));
    if g.starts_with("LOGC4") || (g.starts_with("LOGC") && alexa35) {
        ColourHint {
            transfer: Some(Transfer::LogC4),
            gamut: Some(Gamut::ArriWideGamut4),
        }
    } else if g.starts_with("LOGC") {
        ColourHint {
            transfer: Some(Transfer::LogC3),
            gamut: Some(Gamut::ArriWideGamut3),
        }
    } else if g.contains("709") {
        ColourHint {
            transfer: Some(Transfer::Rec709),
            gamut: Some(Gamut::Rec709),
        }
    } else {
        ColourHint::default()
    }
}

fn date(d: Option<String>, t: Option<String>) -> Option<String> {
    let d = d?;
    if d.len() != 8 || !d.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let iso = format!("{}-{}-{}", &d[0..4], &d[4..6], &d[6..8]);
    let date = display::iso_local(&iso)?;
    let time = t.and_then(|t| {
        // "15h08m05"
        let b = t.as_bytes();
        (b.len() == 8 && b[2] == b'h' && b[5] == b'm')
            .then(|| format!("{}:{}:{}", &t[0..2], &t[3..5], &t[6..8]))
    });
    match time {
        Some(t) => display::iso_local(&format!("{date}T{t}")),
        None => Some(date),
    }
}

#[cfg(test)]
mod tests {
    use super::super::meta::Value;
    use super::*;

    fn items(kv: &[(&str, Value)]) -> Items {
        Items(
            kv.iter()
                .map(|(k, v)| (format!("{NS}{k}"), v.clone()))
                .collect(),
        )
    }

    #[test]
    fn alexa_mini_fields() {
        let it = items(&[
            ("CameraModel", Value::Text("ARRI ALEXA Mini".into())),
            ("CameraSerialNumber", Value::Text("20017".into())),
            ("ColorGammaSxS", Value::Text("LOG-C".into())),
            ("ExposureIndexAsa", Value::Int(800)),
            ("NdFilterDensity", Value::Int(1200)),
            ("ShutterAngle", Value::Int(1728)),
            ("WhiteBalanceKelvin", Value::Int(5600)),
            ("ReelName", Value::Text("M001R00H".into())),
            ("CameraIndex", Value::Text("M".into())),
            ("UserDate", Value::Text("20161207".into())),
            ("UserTime", Value::Text("15h08m05".into())),
            ("LensType", Value::Text("".into())),
        ]);
        let m = read(&it).unwrap();
        assert_eq!(m.camera_make.as_deref(), Some("ARRI"));
        assert_eq!(m.camera_model.as_deref(), Some("ARRI ALEXA Mini"));
        assert_eq!(m.camera_serial.as_deref(), Some("20017"));
        assert_eq!(m.iso.as_deref(), Some("800"));
        assert_eq!(m.nd.as_deref(), Some("1.2"));
        assert_eq!(m.shutter.as_deref(), Some("172.8°"));
        assert_eq!(m.white_balance.as_deref(), Some("5600 K"));
        assert_eq!(m.embedded_reel.as_deref(), Some("M001R00H"));
        assert_eq!(m.embedded_camera_id.as_deref(), Some("M"));
        assert_eq!(m.recording_date.as_deref(), Some("2016-12-07 15:08:05"));
        assert_eq!(m.lens, None);
        assert_eq!(m.colour.transfer, Some(Transfer::LogC3));
        assert_eq!(m.colour.gamut, Some(Gamut::ArriWideGamut3));
    }

    #[test]
    fn not_arri_without_its_keys() {
        assert!(read(&Items::default()).is_none());
    }

    #[test]
    fn alexa_35_logc_is_logc4() {
        let c = colour("LOG-C", Some("ARRI ALEXA 35"));
        assert_eq!(c.transfer, Some(Transfer::LogC4));
        assert_eq!(colour("REC-709", None).transfer, Some(Transfer::Rec709));
        assert_eq!(colour("custom", None), ColourHint::default());
    }
}
