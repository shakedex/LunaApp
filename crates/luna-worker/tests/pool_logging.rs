//! The pool's own log events. Pool threads only see a global subscriber, so this
//! file holds a single test and installs one.

use std::cell::RefCell;
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

/// One event: its level, the clip of the innermost `clip` span it ran in on
/// its thread, and its `name=value` fields.
type Seen = (Level, Option<String>, String);

/// Keeps every event.
struct Collect {
    events: Arc<Mutex<Vec<Seen>>>,
    /// The `clip` field of each span, by id − 1.
    spans: Mutex<Vec<Option<String>>>,
}

thread_local! {
    static ENTERED: RefCell<Vec<u64>> = const { RefCell::new(Vec::new()) };
}

struct Fields(String);

impl Visit for Fields {
    fn record_debug(&mut self, field: &Field, value: &dyn Debug) {
        self.0.push_str(&format!("{}={:?} ", field.name(), value));
    }
}

struct ClipField(Option<String>);

impl Visit for ClipField {
    fn record_debug(&mut self, field: &Field, value: &dyn Debug) {
        if field.name() == "clip" {
            self.0 = Some(format!("{value:?}"));
        }
    }
}

impl tracing::Subscriber for Collect {
    fn enabled(&self, _: &Metadata<'_>) -> bool {
        true
    }
    fn new_span(&self, attrs: &Attributes<'_>) -> Id {
        let mut clip = ClipField(None);
        attrs.record(&mut clip);
        let mut spans = self.spans.lock().unwrap();
        spans.push(clip.0);
        Id::from_u64(spans.len() as u64)
    }
    fn record(&self, _: &Id, _: &Record<'_>) {}
    fn record_follows_from(&self, _: &Id, _: &Id) {}
    fn event(&self, event: &Event<'_>) {
        let mut f = Fields(String::new());
        event.record(&mut f);
        let level = *event.metadata().level();
        let spans = self.spans.lock().unwrap();
        let clip = ENTERED.with(|e| {
            e.borrow()
                .iter()
                .rev()
                .find_map(|id| spans[*id as usize - 1].clone())
        });
        self.events.lock().unwrap().push((level, clip, f.0));
    }
    fn enter(&self, id: &Id) {
        ENTERED.with(|e| e.borrow_mut().push(id.into_u64()));
    }
    fn exit(&self, _: &Id) {
        ENTERED.with(|e| e.borrow_mut().pop());
    }
}

#[test]
fn crash_is_logged_under_its_clip_after_the_workers_last_lines() {
    let events = Arc::new(Mutex::new(Vec::new()));
    tracing::subscriber::set_global_default(Collect {
        events: events.clone(),
        spans: Mutex::new(Vec::new()),
    })
    .unwrap();

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
            main: "Z:/does-not-exist/A001__log____crash__.mov".into(),
            segments: vec![],
            sidecars: vec![],
        },
        strip: 3,
        thumb_width: 640,
        colour: Default::default(),
    };
    let out = run_pool(&config, vec![job], &AtomicBool::new(false), &|_, _| {}).unwrap();
    assert_eq!(out[0].1, JobOutcome::Crashed);

    let events = events.lock().unwrap();
    let crashed = events
        .iter()
        .position(|(level, _, text)| *level == Level::ERROR && text.contains("worker crashed"))
        .unwrap_or_else(|| panic!("{events:#?}"));
    let (_, clip, text) = &events[crashed];
    assert_eq!(clip.as_deref(), Some("A001__log____crash__"), "{events:#?}");
    assert!(text.contains("A001__log____crash__.mov"), "{events:#?}");
    let forwarded = events
        .iter()
        .position(|(_, _, text)| text.contains("hook: hello"))
        .unwrap_or_else(|| panic!("{events:#?}"));
    assert!(forwarded < crashed, "{events:#?}");
    let started = events
        .iter()
        .find(|(_, _, text)| text.contains("job started"))
        .unwrap_or_else(|| panic!("{events:#?}"));
    assert_eq!(
        started.1.as_deref(),
        Some("A001__log____crash__"),
        "{events:#?}"
    );
}
