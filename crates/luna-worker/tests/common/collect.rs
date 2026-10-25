//! A hand-written subscriber for tests: keeps every event with its level, the
//! clip of the innermost `clip` span it ran in, and its `name=value` fields.

use std::fmt::Debug;
use std::sync::{Arc, Mutex};

use tracing::field::{Field, Visit};
use tracing::span::{Attributes, Id, Record};
use tracing::{Event, Level, Metadata};

#[derive(Debug, Clone)]
pub struct Seen {
    pub level: Level,
    pub clip: Option<String>,
    pub text: String,
}

#[derive(Default)]
struct State {
    /// The `clip` field of each span, by id − 1.
    spans: Vec<Option<String>>,
    entered: Vec<u64>,
    events: Vec<Seen>,
}

struct Collect(Arc<Mutex<State>>);

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
        let mut state = self.0.lock().unwrap();
        state.spans.push(clip.0);
        Id::from_u64(state.spans.len() as u64)
    }
    fn record(&self, _: &Id, _: &Record<'_>) {}
    fn record_follows_from(&self, _: &Id, _: &Id) {}
    fn event(&self, event: &Event<'_>) {
        let mut f = Fields(String::new());
        event.record(&mut f);
        let mut state = self.0.lock().unwrap();
        let clip = state
            .entered
            .iter()
            .rev()
            .find_map(|id| state.spans[*id as usize - 1].clone());
        state.events.push(Seen {
            level: *event.metadata().level(),
            clip,
            text: f.0,
        });
    }
    fn enter(&self, id: &Id) {
        self.0.lock().unwrap().entered.push(id.into_u64());
    }
    fn exit(&self, _: &Id) {
        self.0.lock().unwrap().entered.pop();
    }
}

/// Runs `f` with the collector as this thread's subscriber.
pub fn capture<R>(f: impl FnOnce() -> R) -> (R, Vec<Seen>) {
    let state = Arc::new(Mutex::new(State::default()));
    let out = tracing::subscriber::with_default(Collect(state.clone()), f);
    let events = std::mem::take(&mut state.lock().unwrap().events);
    (out, events)
}
