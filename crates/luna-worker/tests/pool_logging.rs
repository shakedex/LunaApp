//! The pool's own log events. Pool threads only see a global subscriber, so this
//! file holds a single test and installs one.

use std::fmt::Debug;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use luna_formats::{ClipFiles, Container};
use luna_worker::ClipJob;
use luna_worker::pool::{JobOutcome, PoolConfig, run_pool};
use tracing::field::{Field, Visit};
use tracing::span::{Attributes, Id, Record};
use tracing::{Event, Level, Metadata};

/// Keeps every event as its level and `name=value` fields.
struct Collect(Arc<Mutex<Vec<(Level, String)>>>);

struct Fields(String);

impl Visit for Fields {
    fn record_debug(&mut self, field: &Field, value: &dyn Debug) {
        self.0.push_str(&format!("{}={:?} ", field.name(), value));
    }
}

impl tracing::Subscriber for Collect {
    fn enabled(&self, _: &Metadata<'_>) -> bool {
        true
    }
    fn new_span(&self, _: &Attributes<'_>) -> Id {
        Id::from_u64(1)
    }
    fn record(&self, _: &Id, _: &Record<'_>) {}
    fn record_follows_from(&self, _: &Id, _: &Id) {}
    fn event(&self, event: &Event<'_>) {
        let mut f = Fields(String::new());
        event.record(&mut f);
        let level = *event.metadata().level();
        self.0.lock().unwrap().push((level, f.0));
    }
    fn enter(&self, _: &Id) {}
    fn exit(&self, _: &Id) {}
}

#[test]
fn crash_is_logged_with_clip_path() {
    let events = Arc::new(Mutex::new(Vec::new()));
    tracing::subscriber::set_global_default(Collect(events.clone())).unwrap();

    let config = PoolConfig {
        exe: env!("CARGO_BIN_EXE_luna-worker").into(),
        args: vec![],
        env: vec![("LUNA_WORKER_TEST_HOOKS".into(), "1".into())],
        workers: 1,
        timeout: Duration::from_secs(30),
        on_line: None,
    };
    let job = ClipJob {
        id: 0,
        files: ClipFiles {
            container: Container::QuickTime,
            main: "Z:/does-not-exist/A001__crash__.mov".into(),
            segments: vec![],
            sidecars: vec![],
        },
        strip: 3,
        thumb_width: 640,
    };
    let out = run_pool(&config, vec![job], &AtomicBool::new(false), &|_, _| {}).unwrap();
    assert_eq!(out[0].1, JobOutcome::Crashed);

    let events = events.lock().unwrap();
    assert!(
        events
            .iter()
            .any(|(level, text)| *level == Level::ERROR && text.contains("A001__crash__.mov")),
        "{events:?}"
    );
}
