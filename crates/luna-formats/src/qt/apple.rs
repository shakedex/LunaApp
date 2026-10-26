//! Apple's QuickTime keys beyond make, model and date: the camera keys an
//! iPhone writes into its video track's `meta` (`com.apple.quicktime.camera.*`)
//! and the Pro Apps slate keys (`com.apple.proapps.*`) that recorders such as
//! Blackmagic Video Assist write into the movie's `meta`.

use super::meta::{Items, Value};
use crate::trace::Sources;
use crate::types::ClipMeta;

const LENS: &str = "com.apple.quicktime.camera.lens_model";
const FOCAL_35: &str = "com.apple.quicktime.camera.focal_length.35mm_equivalent";
const IRIS: &str = "com.apple.quicktime.camera.lens_irisfnumber";
const REEL: &str = "com.apple.proapps.reel";
const CAMERA: &str = "com.apple.proapps.cameraName";
const SCENE: &str = "com.apple.proapps.scene";
const SHOT: &str = "com.apple.proapps.shot";
const TAKE: &str = "com.apple.proapps.take";

/// Lens, focal length and stop from the video track's keys (or the movie's),
/// and reel, camera and slate from the Pro Apps keys, with each field's key.
pub(crate) fn read(movie: &Items, track: &Items) -> (ClipMeta, Sources) {
    let mut sources = Sources::default();
    let mut find = |field: &'static str, key: &'static str| {
        let (place, v) = [("video track", track), ("movie", movie)]
            .into_iter()
            .find_map(|(place, items)| items.get(key).and_then(text).map(|v| (place, v)))?;
        sources.set(field, format!("QuickTime key {key} ({place} meta)"));
        Some(v)
    };
    let mut m = ClipMeta {
        lens: find("lens", LENS),
        focal_length: find("focal_length", FOCAL_35).map(|mm| format!("{mm} mm (35 mm eq.)")),
        stop: find("stop", IRIS).map(|f| stop(&f)),
        embedded_reel: find("embedded_reel", REEL),
        embedded_camera_id: find("embedded_camera_id", CAMERA),
        ..ClipMeta::default()
    };
    m.slate.scene = find("scene", SCENE);
    m.slate.shot = find("shot", SHOT);
    m.slate.take = find("take", TAKE);
    (m, sources)
}

/// A key's value as display text: text trimmed, numbers as written.
fn text(v: &Value) -> Option<String> {
    match v {
        Value::Float(f) => Some(format!("{f}")),
        v => v
            .text()
            .map(str::to_string)
            .or_else(|| v.int().map(|i| i.to_string())),
    }
}

/// "F1.78" → "f/1.78"; anything else as written.
fn stop(f: &str) -> String {
    match f
        .strip_prefix(['F', 'f'])
        .map(|n| n.trim_start_matches('/'))
    {
        Some(n) if n.parse::<f64>().is_ok() => format!("f/{n}"),
        _ => f.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn items(pairs: &[(&str, Value)]) -> Items {
        Items(
            pairs
                .iter()
                .map(|(k, v)| (k.to_string(), v.clone()))
                .collect(),
        )
    }

    fn t(s: &str) -> Value {
        Value::Text(s.into())
    }

    #[test]
    fn iphone_lens_keys_from_the_video_track() {
        let track = items(&[
            (LENS, t("iPhone 17 Pro back camera 6.765mm f/1.78")),
            (FOCAL_35, t("25")),
            (IRIS, t("F1.78")),
        ]);
        let (m, sources) = read(&Items::default(), &track);
        assert_eq!(
            m.lens.as_deref(),
            Some("iPhone 17 Pro back camera 6.765mm f/1.78")
        );
        assert_eq!(m.focal_length.as_deref(), Some("25 mm (35 mm eq.)"));
        assert_eq!(m.stop.as_deref(), Some("f/1.78"));
        assert_eq!(
            sources.get("lens"),
            Some("QuickTime key com.apple.quicktime.camera.lens_model (video track meta)")
        );
    }

    #[test]
    fn video_assist_slate_keys_from_the_movie() {
        let movie = items(&[
            (REEL, t("1")),
            (CAMERA, t("A")),
            (SCENE, t("1")),
            (SHOT, Value::Int(99)),
        ]);
        let (m, sources) = read(&movie, &Items::default());
        assert_eq!(m.embedded_reel.as_deref(), Some("1"));
        assert_eq!(m.embedded_camera_id.as_deref(), Some("A"));
        assert_eq!(m.slate.scene.as_deref(), Some("1"));
        assert_eq!(m.slate.shot.as_deref(), Some("99"));
        assert_eq!(m.slate.take, None);
        assert_eq!(
            sources.get("shot"),
            Some("QuickTime key com.apple.proapps.shot (movie meta)")
        );
    }

    #[test]
    fn stop_reads_apple_and_plain_forms() {
        assert_eq!(stop("F1.78"), "f/1.78");
        assert_eq!(stop("f/2.8"), "f/2.8");
        assert_eq!(stop("T2.8"), "T2.8");
    }
}
