//! Luna's logging: one file per run, `luna.log` for everything else, retention
//! at startup. Binaries call [`init`]; library crates only emit `tracing` events.

use std::io::{BufWriter, Write};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, SystemTime};

use tracing::Level;
use tracing_subscriber::EnvFilter;
use tracing_subscriber::layer::SubscriberExt;

use crate::layer::RunLayer;

mod files;
mod format;
mod layer;

pub use files::{prune, run_file_name, sanitise};
pub use format::parse_level;

/// How many run logs survive a prune.
pub const KEEP_RUNS: usize = 50;
/// Run logs older than this are deleted at startup.
pub const MAX_AGE: Duration = Duration::from_secs(60 * 24 * 3600);
/// `luna.log` rolls over to `luna.old.log` past this size.
pub const APP_LOG_MAX_BYTES: u64 = 5_000_000;

/// Which process is logging.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// The GUI app: files only (and stderr in debug builds).
    App,
    /// The command line: files plus INFO and above on stderr.
    Cli,
    /// A worker process: DEBUG and above on stderr for the parent to forward.
    Worker,
}

/// `LUNA_LOG_DIR` when set, else `<local data>/Luna/logs`.
pub fn logs_dir() -> Option<PathBuf> {
    match std::env::var_os("LUNA_LOG_DIR") {
        Some(dir) if !dir.is_empty() => Some(PathBuf::from(dir)),
        _ => dirs::data_local_dir().map(|d| d.join("Luna").join("logs")),
    }
}

/// Installs the global subscriber and the panic hook. A second call does nothing.
pub fn init(mode: Mode) {
    init_with_dir(mode, logs_dir());
}

/// Luna's own targets at DEBUG; other crates only when something is wrong.
const FILTER: &str = "warn,luna=debug";

/// [`init`] with an explicit logs folder; `None` writes no files.
pub fn init_with_dir(mode: Mode, dir: Option<PathBuf>) {
    static STARTED: AtomicBool = AtomicBool::new(false);
    if STARTED.swap(true, Ordering::SeqCst) {
        return;
    }
    let dir = if mode == Mode::Worker { None } else { dir };
    let app = dir.as_deref().and_then(|dir| {
        let _ = std::fs::create_dir_all(dir);
        prune(dir, SystemTime::now());
        files::open_app_log(dir)
    });
    let stderr = match mode {
        Mode::App => cfg!(debug_assertions).then_some(Level::DEBUG),
        Mode::Cli => Some(Level::INFO),
        Mode::Worker => Some(Level::DEBUG),
    };
    let app = app.map(|f| Box::new(BufWriter::new(f)) as Box<dyn Write + Send>);
    let subscriber = tracing_subscriber::registry()
        .with(EnvFilter::new(FILTER))
        .with(RunLayer::new(dir, app, stderr));
    if tracing::subscriber::set_global_default(subscriber).is_err() {
        return;
    }
    install_panic_hook(mode != Mode::Worker);
}

/// Logs panics as ERROR events. A worker's goes to stderr only once, as a
/// formatted line, so it does not chain to the default hook.
fn install_panic_hook(chain: bool) {
    let default = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        tracing::error!(target: "luna::panic", "{info}");
        if chain {
            default(info);
        }
    }));
}
