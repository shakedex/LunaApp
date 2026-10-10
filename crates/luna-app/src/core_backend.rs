//! The real backend: `luna_core::run` on a background thread, and the
//! platform settings store.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use luna_core::run::{Progress, RunError, WorkerSpec, run};
use luna_core::settings::{Settings, SettingsStore as CoreStore};

use crate::backend::{RunBackend, RunControl, RunEvent, RunOutcome};
use crate::settings_store::SettingsStore;

pub struct CoreBackend {
    workers: WorkerSpec,
}

impl CoreBackend {
    pub fn new(workers: WorkerSpec) -> Self {
        Self { workers }
    }
}

struct Control(Arc<AtomicBool>);

impl RunControl for Control {
    fn cancel(&self) {
        self.0.store(true, Ordering::SeqCst);
    }
}

impl RunBackend for CoreBackend {
    fn start(
        &self,
        source: PathBuf,
        settings: Settings,
        events: Box<dyn Fn(RunEvent) + Send>,
    ) -> Box<dyn RunControl> {
        let cancel = Arc::new(AtomicBool::new(false));
        let flag = cancel.clone();
        let workers = self.workers.clone();
        std::thread::spawn(move || {
            let events = std::sync::Mutex::new(events);
            let emit = |e: RunEvent| (events.lock().unwrap_or_else(|p| p.into_inner()))(e);
            let progress = |p: Progress| {
                emit(match p {
                    Progress::FilesFound(n) => RunEvent::FilesFound(n),
                    Progress::ClipsTotal(n) => RunEvent::ClipsTotal(n),
                    Progress::ClipDone {
                        done,
                        total,
                        problems,
                    } => RunEvent::ClipDone {
                        done,
                        total,
                        problems,
                    },
                })
            };
            let result = run(&source, &settings, &workers, &progress, &flag);
            emit(match result {
                Ok(outcome) => RunEvent::Finished(RunOutcome {
                    pdf: outcome.files.pdf,
                    html: outcome.files.html,
                    csv: outcome.files.csv,
                    folder: outcome.files.folder,
                    summary: outcome.summary,
                }),
                Err(RunError::Cancelled) => RunEvent::Cancelled,
                Err(e) => RunEvent::Failed(e.to_string()),
            });
        });
        Box::new(Control(cancel))
    }
}

/// Settings in the platform config folder (`%APPDATA%\Luna`,
/// `~/Library/Application Support/Luna`).
pub struct PlatformSettingsStore(CoreStore);

impl PlatformSettingsStore {
    pub fn new() -> Option<Self> {
        CoreStore::platform().map(Self)
    }
}

impl SettingsStore for PlatformSettingsStore {
    fn load(&self) -> Settings {
        self.0.load()
    }

    fn save(&self, settings: &Settings) -> Result<(), String> {
        self.0
            .save(settings)
            .map_err(|e| format!("Settings couldn't be saved: {e}"))
    }

    fn set_logo(&self, settings: &mut Settings, file: &Path) -> Result<(), String> {
        self.0.set_logo(settings, file)
    }

    fn clear_logo(&self, settings: &mut Settings) {
        self.0.clear_logo(settings)
    }
}
