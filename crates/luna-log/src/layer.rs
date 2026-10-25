//! The routing layer: events inside a `run` span go to that run's file,
//! everything else to `luna.log`.

use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, PoisonError};
use std::thread::ThreadId;

use tracing::span::{Attributes, Id};
use tracing::{Event, Level, Subscriber};
use tracing_subscriber::Layer;
use tracing_subscriber::layer::Context;
use tracing_subscriber::registry::LookupSpan;

use crate::files::open_run_file;
use crate::format::{self, FieldText};

pub(crate) struct RunLayer {
    /// Where run files go; `None` writes none.
    dir: Option<PathBuf>,
    /// `luna.log`, for events outside a run.
    app: Mutex<Option<Box<dyn Write + Send>>>,
    /// Lines at this level or above are copied to stderr.
    stderr: Option<Level>,
}

/// The open file of a `run` span, and the thread that opened it.
struct RunFile {
    file: Mutex<BufWriter<File>>,
    owner: ThreadId,
}

impl RunFile {
    fn flush(&self) {
        let mut w = self.file.lock().unwrap_or_else(PoisonError::into_inner);
        let _ = w.flush();
    }
}

/// The `clip` field of a `clip` span.
struct Clip(String);

impl RunLayer {
    pub(crate) fn new(
        dir: Option<PathBuf>,
        app: Option<Box<dyn Write + Send>>,
        stderr: Option<Level>,
    ) -> Self {
        Self {
            dir,
            app: Mutex::new(app),
            stderr,
        }
    }
}

fn field(record: impl FnOnce(&mut FieldText<'_>), name: &str) -> Option<String> {
    let mut text = FieldText { name, value: None };
    record(&mut text);
    text.value
}

/// Writes one line; the first failure is reported once on stderr, the rest
/// are dropped. Runs under a writer lock, so nothing here may panic.
fn write_line(w: &mut dyn Write, line: &str, flush: bool) {
    static REPORTED: AtomicBool = AtomicBool::new(false);
    let result = w
        .write_all(line.as_bytes())
        .and_then(|()| if flush { w.flush() } else { Ok(()) });
    if let Err(e) = result
        && !REPORTED.swap(true, Ordering::Relaxed)
    {
        let note = format!("luna-log: could not write the log: {e}\n");
        let _ = std::io::stderr().lock().write_all(note.as_bytes());
    }
}

impl<S: Subscriber + for<'a> LookupSpan<'a>> Layer<S> for RunLayer {
    fn on_new_span(&self, attrs: &Attributes<'_>, id: &Id, ctx: Context<'_, S>) {
        let Some(span) = ctx.span(id) else {
            return;
        };
        let meta = attrs.metadata();
        let mut ext = span.extensions_mut();
        if let Some(clip) = field(|v| attrs.record(v), "clip") {
            ext.insert(Clip(clip));
        }
        if meta.name() == "run"
            && meta.target() == "luna::run"
            && let Some(dir) = &self.dir
        {
            let source = field(|v| attrs.record(v), "source").unwrap_or_default();
            let started = jiff::Zoned::now().datetime();
            if let Some(file) = open_run_file(dir, &started, &source) {
                ext.insert(RunFile {
                    file: Mutex::new(BufWriter::new(file)),
                    owner: std::thread::current().id(),
                });
            }
        }
    }

    fn on_event(&self, event: &Event<'_>, ctx: Context<'_, S>) {
        let meta = event.metadata();
        let level = *meta.level();
        let text = if meta.target() == "luna::worker_line" {
            let mut line = field(|v| event.record(v), "line").unwrap_or_default();
            line.push('\n');
            line
        } else {
            let clip = ctx.event_scope(event).and_then(|mut scope| {
                scope.find_map(|s| s.extensions().get::<Clip>().map(|c| c.0.clone()))
            });
            format::line(level, clip.as_deref(), meta.target(), event)
        };

        if self.stderr.is_some_and(|min| level <= min) {
            let _ = std::io::stderr().lock().write_all(text.as_bytes());
        }

        // WARN and above reach the disk right away, so a crash keeps them.
        let urgent = level <= Level::WARN;
        if let Some(scope) = ctx.event_scope(event) {
            for span in scope {
                let ext = span.extensions();
                if let Some(run) = ext.get::<RunFile>() {
                    let mut w = run.file.lock().unwrap_or_else(PoisonError::into_inner);
                    write_line(&mut *w, &text, urgent);
                    return;
                }
            }
        }
        let mut app = self.app.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some(w) = app.as_mut() {
            write_line(w.as_mut(), &text, true);
        }
    }

    /// The thread that opened the run leaving it means the run is over, even
    /// while other threads still hold the span.
    fn on_exit(&self, id: &Id, ctx: Context<'_, S>) {
        if let Some(span) = ctx.span(id)
            && let Some(run) = span.extensions().get::<RunFile>()
            && run.owner == std::thread::current().id()
        {
            run.flush();
        }
    }

    fn on_close(&self, id: Id, ctx: Context<'_, S>) {
        if let Some(span) = ctx.span(&id)
            && let Some(run) = span.extensions().get::<RunFile>()
        {
            run.flush();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tracing_subscriber::layer::SubscriberExt;

    #[test]
    fn unwritable_dir_does_not_panic() {
        let dir = tempfile::tempdir().unwrap();
        let not_a_dir = dir.path().join("file");
        std::fs::write(&not_a_dir, "").unwrap();
        let layer = RunLayer::new(Some(not_a_dir.clone()), None, None);
        tracing::subscriber::with_default(tracing_subscriber::registry().with(layer), || {
            let _run =
                tracing::info_span!(target: "luna::run", "run", source = %"Card A").entered();
            tracing::error!(target: "luna::test", "still fine");
        });
        assert_eq!(std::fs::read(&not_a_dir).unwrap(), b"");
    }

    #[test]
    fn run_file_is_flushed_when_its_thread_leaves_the_run() {
        let dir = tempfile::tempdir().unwrap();
        let layer = RunLayer::new(Some(dir.path().to_path_buf()), None, None);
        tracing::subscriber::with_default(tracing_subscriber::registry().with(layer), || {
            let run = tracing::info_span!(target: "luna::run", "run", source = %"Card A");
            // A reader thread that outlives the run still holds a clone.
            let still_alive = run.clone();
            run.in_scope(|| tracing::info!(target: "luna::run", "summary: 3 clips"));
            let files: Vec<_> = std::fs::read_dir(dir.path()).unwrap().flatten().collect();
            let text = std::fs::read_to_string(files[0].path()).unwrap();
            assert!(
                text.ends_with(" INFO  luna::run: summary: 3 clips\n"),
                "{text}"
            );
            drop(still_alive);
        });
    }

    #[test]
    fn run_file_is_flushed_when_the_run_closes() {
        let dir = tempfile::tempdir().unwrap();
        let layer = RunLayer::new(Some(dir.path().to_path_buf()), None, None);
        tracing::subscriber::with_default(tracing_subscriber::registry().with(layer), || {
            let run = tracing::info_span!(target: "luna::run", "run", source = %"E:\\");
            run.in_scope(|| {
                let _clip = tracing::info_span!("clip", clip = %"A001").entered();
                tracing::debug!(target: "luna::test", "quiet");
            });
        });
        let files: Vec<_> = std::fs::read_dir(dir.path()).unwrap().flatten().collect();
        assert_eq!(files.len(), 1);
        let name = files[0].file_name().to_string_lossy().into_owned();
        assert!(name.ends_with(" E__.log"), "{name}");
        let text = std::fs::read_to_string(files[0].path()).unwrap();
        assert!(
            text.ends_with(" DEBUG [A001] luna::test: quiet\n"),
            "{text}"
        );
    }
}
