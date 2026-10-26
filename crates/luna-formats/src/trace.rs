//! Where each metadata field came from, for the run log.

use crate::types::ClipMeta;

/// Every `ClipMeta` field by name, and whether it holds a value.
pub(crate) fn present(m: &ClipMeta) -> [(&'static str, bool); 25] {
    [
        ("recording_date", m.recording_date.is_some()),
        ("start_timecode", m.start_timecode.is_some()),
        ("frame_rate", m.frame_rate.is_some()),
        ("frame_count", m.frame_count.is_some()),
        ("width", m.width.is_some()),
        ("height", m.height.is_some()),
        ("camera_make", m.camera_make.is_some()),
        ("camera_model", m.camera_model.is_some()),
        ("camera_serial", m.camera_serial.is_some()),
        ("lens", m.lens.is_some()),
        ("focal_length", m.focal_length.is_some()),
        ("stop", m.stop.is_some()),
        ("iso", m.iso.is_some()),
        ("shutter", m.shutter.is_some()),
        ("white_balance", m.white_balance.is_some()),
        ("nd", m.nd.is_some()),
        ("sensor_fps", m.sensor_fps.is_some()),
        ("audio", m.audio.is_some()),
        ("embedded_reel", m.embedded_reel.is_some()),
        ("embedded_camera_id", m.embedded_camera_id.is_some()),
        ("scene", m.slate.scene.is_some()),
        ("shot", m.slate.shot.is_some()),
        ("take", m.slate.take.is_some()),
        ("transfer", m.colour.transfer.is_some()),
        ("gamut", m.colour.gamut.is_some()),
    ]
}

/// The source of each field that has one.
#[derive(Debug, Default)]
pub(crate) struct Sources(Vec<(&'static str, String)>);

impl Sources {
    /// Names `source` as where `field` came from, replacing any earlier one.
    pub(crate) fn set(&mut self, field: &'static str, source: impl Into<String>) {
        let source = source.into();
        match self.0.iter_mut().find(|(f, _)| *f == field) {
            Some(slot) => slot.1 = source,
            None => self.0.push((field, source)),
        }
    }

    /// Credits `source(field)` with every field `after` has and `before` lacked.
    pub(crate) fn gained(
        &mut self,
        before: &[(&'static str, bool); 25],
        after: &ClipMeta,
        source: impl Fn(&'static str) -> String,
    ) {
        for ((field, had), (_, has)) in before.iter().zip(present(after)) {
            if has && !had {
                self.set(field, source(field));
            }
        }
    }

    pub(crate) fn get(&self, field: &str) -> Option<&str> {
        self.0
            .iter()
            .find(|(f, _)| *f == field)
            .map(|(_, s)| s.as_str())
    }

    /// Logs at DEBUG which metadata blocks the file has, then each field's
    /// source, or where it was looked for: the field's own place (`base`)
    /// and every block that was found.
    pub(crate) fn log(
        &self,
        m: &ClipMeta,
        base: impl Fn(&str) -> &'static str,
        blocks: &[(String, bool)],
    ) {
        let all: Vec<String> = blocks
            .iter()
            .map(|(b, found)| format!("{b}: {}", if *found { "found" } else { "absent" }))
            .collect();
        tracing::debug!(blocks = %all.join("; "), "meta blocks");
        let found: Vec<&str> = blocks
            .iter()
            .filter(|(_, f)| *f)
            .map(|(b, _)| b.as_str())
            .collect();
        for (field, has) in present(m) {
            match (has, self.get(field)) {
                (true, Some(source)) => tracing::debug!(field, source, "meta source"),
                (true, None) => tracing::debug!(field, source = "unknown", "meta source"),
                (false, _) => {
                    let places: Vec<&str> = std::iter::once(base(field))
                        .filter(|b| !b.is_empty())
                        .chain(found.iter().copied())
                        .collect();
                    let looked_in = if places.is_empty() {
                        "no vendor metadata block".to_string()
                    } else {
                        places.join("; ")
                    };
                    tracing::debug!(field, looked_in = %looked_in, "meta not found")
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gained_credits_only_new_fields() {
        let mut s = Sources::default();
        let mut m = ClipMeta {
            lens: Some("A".into()),
            ..ClipMeta::default()
        };
        s.set("lens", "first");
        let before = present(&m);
        m.iso = Some("800".into());
        s.gained(&before, &m, |_| "second".into());
        assert_eq!(s.get("lens"), Some("first"));
        assert_eq!(s.get("iso"), Some("second"));
        assert_eq!(s.get("nd"), None);
    }
}
