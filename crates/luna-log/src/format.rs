//! The line format: `HH:MM:SS.mmm LEVEL [clip] target: message key=value…`.

use std::fmt::{Debug, Write};

use tracing::field::{Field, Visit};
use tracing::{Event, Level};

/// Formats one event as a line ending in `\n`.
pub(crate) fn line(level: Level, clip: Option<&str>, target: &str, event: &Event<'_>) -> String {
    let now = jiff::Zoned::now();
    let mut fields = Fields::default();
    event.record(&mut fields);
    let mut out = String::with_capacity(96);
    let _ = write!(
        out,
        "{:02}:{:02}:{:02}.{:03} {:<5} ",
        now.hour(),
        now.minute(),
        now.second(),
        now.millisecond(),
        level.as_str()
    );
    if let Some(clip) = clip {
        let _ = write!(out, "[{clip}] ");
    }
    let _ = write!(out, "{target}: {}", fields.message);
    let rest = if fields.message.is_empty() {
        fields.rest.trim_start()
    } else {
        &fields.rest
    };
    out.push_str(rest);
    out.push('\n');
    out
}

/// The level of a formatted line: its second whitespace-separated token.
pub fn parse_level(line: &str) -> Option<Level> {
    match line.split_whitespace().nth(1)? {
        "TRACE" => Some(Level::TRACE),
        "DEBUG" => Some(Level::DEBUG),
        "INFO" => Some(Level::INFO),
        "WARN" => Some(Level::WARN),
        "ERROR" => Some(Level::ERROR),
        _ => None,
    }
}

/// The value of one named field, as text.
pub(crate) struct FieldText<'a> {
    pub(crate) name: &'a str,
    pub(crate) value: Option<String>,
}

impl Visit for FieldText<'_> {
    fn record_str(&mut self, field: &Field, value: &str) {
        if field.name() == self.name {
            self.value = Some(value.to_string());
        }
    }

    fn record_debug(&mut self, field: &Field, value: &dyn Debug) {
        if field.name() == self.name {
            self.value = Some(format!("{value:?}"));
        }
    }
}

/// An event's message and its other fields as ` key=value`.
#[derive(Default)]
struct Fields {
    message: String,
    rest: String,
}

impl Visit for Fields {
    fn record_str(&mut self, field: &Field, value: &str) {
        if field.name() == "message" {
            self.message.push_str(value);
        } else {
            self.record_debug(field, &value);
        }
    }

    fn record_debug(&mut self, field: &Field, value: &dyn Debug) {
        if field.name() == "message" {
            let _ = write!(self.message, "{value:?}");
        } else {
            let _ = write!(self.rest, " {}={value:?}", field.name());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layer::RunLayer;
    use std::io::Write;
    use std::sync::{Arc, Mutex};
    use tracing_subscriber::layer::SubscriberExt;

    #[derive(Clone, Default)]
    struct Sink(Arc<Mutex<Vec<u8>>>);

    impl Write for Sink {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            self.0.lock().unwrap().write(buf)
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    impl Sink {
        fn text(&self) -> String {
            String::from_utf8(self.0.lock().unwrap().clone()).unwrap()
        }
    }

    /// Runs `f` with a layer that writes `luna.log` lines into the returned sink.
    fn capture(f: impl FnOnce()) -> String {
        let sink = Sink::default();
        let layer = RunLayer::new(None, Some(Box::new(sink.clone())), None);
        tracing::subscriber::with_default(tracing_subscriber::registry().with(layer), f);
        sink.text()
    }

    /// `HH:MM:SS.mmm`
    fn is_time(s: &str) -> bool {
        let b = s.as_bytes();
        b.len() == 12
            && b.iter().enumerate().all(|(i, c)| match i {
                2 | 5 => *c == b':',
                8 => *c == b'.',
                _ => c.is_ascii_digit(),
            })
    }

    #[test]
    fn format_and_parse_level_round_trip() {
        let text = capture(|| {
            let _clip = tracing::info_span!("clip", clip = %"A001C003").entered();
            tracing::warn!(target: "t", k = 1, "msg");
        });
        let line = text.strip_suffix('\n').unwrap();
        let (time, rest) = line.split_at(12);
        assert!(is_time(time), "{line}");
        assert_eq!(rest, " WARN  [A001C003] t: msg k=1");
        assert_eq!(parse_level(line), Some(Level::WARN));
    }

    #[test]
    fn line_outside_a_clip_has_no_brackets() {
        let text = capture(|| tracing::error!(target: "t", name = "a b", "boom"));
        assert_eq!(&text[12..], " ERROR t: boom name=\"a b\"\n");
    }

    #[test]
    fn parse_level_rejects_other_tokens() {
        assert_eq!(
            parse_level("12:00:00.000 INFO  [c] x: hi"),
            Some(Level::INFO)
        );
        assert_eq!(parse_level("12:00:00.000 DEBUG x: hi"), Some(Level::DEBUG));
        assert_eq!(parse_level("thread 'main' panicked"), None);
        assert_eq!(parse_level("a 3 b"), None);
        assert_eq!(parse_level(""), None);
    }
}
