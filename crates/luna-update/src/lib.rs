//! Luna's updates: WinSparkle on Windows and Sparkle on macOS, loaded at run
//! time from the bundled-library folder. A development build has neither
//! library, so it never checks for updates.
//!
//! Both libraries check GitHub once a day, show their own update window when
//! they find a newer version, and verify its ed25519 signature before
//! installing.

use std::sync::{Mutex, OnceLock};

#[cfg(windows)]
mod windows;

/// This platform's update feed, on the latest GitHub release.
pub const FEED_URL: &str = if cfg!(target_os = "macos") {
    "https://github.com/shakedex/LunaApp/releases/latest/download/appcast-macos.xml"
} else {
    "https://github.com/shakedex/LunaApp/releases/latest/download/appcast-windows.xml"
};

/// Seconds between automatic checks.
pub const CHECK_INTERVAL: u32 = 86_400;

/// What the update library tells Luna.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Event {
    /// A newer version exists (its window is showing).
    Found,
    /// A check found nothing newer.
    NotFound,
    /// The user chose "Skip this version".
    Skipped,
    /// The installer is about to run: Luna must quit (Windows).
    ShutdownRequested,
}

type Handler = Box<dyn Fn(Event) + Send + Sync>;

/// The callbacks take no context, so the handler is process-wide.
static HANDLER: OnceLock<Handler> = OnceLock::new();
static UNAVAILABLE: Mutex<Option<String>> = Mutex::new(None);

fn emit(event: Event) {
    if let Some(handler) = HANDLER.get() {
        handler(event);
    }
}

/// The running updater; dropping it stops the library.
pub struct Updater {
    #[cfg(windows)]
    inner: windows::WinSparkle,
}

impl Updater {
    /// A check the user asked for: the library's window shows the result.
    pub fn check_now(&self) {
        #[cfg(windows)]
        self.inner.check_now();
    }
}

/// Starts automatic checks for `version` (Luna's own version), or `None` when
/// this build has no update library (see [`unavailable_reason`]). Only the
/// first call in a process can succeed.
pub fn start(version: &str, on_event: impl Fn(Event) + Send + Sync + 'static) -> Option<Updater> {
    let started = start_platform(version, Box::new(on_event));
    if let Err(reason) = &started {
        *UNAVAILABLE.lock().unwrap_or_else(|e| e.into_inner()) = Some(reason.clone());
    }
    started.ok()
}

/// Why the last [`start`] returned `None`.
pub fn unavailable_reason() -> Option<String> {
    UNAVAILABLE
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .clone()
}

#[cfg(windows)]
fn start_platform(version: &str, handler: Handler) -> Result<Updater, String> {
    let inner = windows::WinSparkle::load()?;
    HANDLER
        .set(handler)
        .map_err(|_| "the updater is already running".to_string())?;
    inner.start(&feed_url(), public_key(), version)?;
    Ok(Updater { inner })
}

#[cfg(not(windows))]
fn start_platform(_version: &str, _handler: Handler) -> Result<Updater, String> {
    Err("updates aren't available on this platform yet".into())
}

/// The feed to read: [`FEED_URL`], or `LUNA_UPDATE_FEED` when testing an
/// update locally (signatures are still checked).
pub fn feed_url() -> String {
    feed_url_from(std::env::var("LUNA_UPDATE_FEED").ok())
}

fn feed_url_from(over: Option<String>) -> String {
    over.filter(|s| !s.is_empty())
        .unwrap_or_else(|| FEED_URL.to_string())
}

/// The base64 ed25519 key every update must be signed with.
pub fn public_key() -> &'static str {
    include_str!("../../../packaging/update-public-key.txt").trim()
}

#[cfg(test)]
mod tests {
    use super::*;
    use base64::Engine;

    #[test]
    fn no_library_means_no_updater() {
        // The test binary has no lib\WinSparkle.dll (or Sparkle.framework).
        assert!(start("2026.10.0", |_| {}).is_none());
        let reason = unavailable_reason().expect("a reason");
        if cfg!(windows) {
            assert!(reason.contains("WinSparkle.dll"), "{reason}");
        }
    }

    #[test]
    fn feed_url_default_and_override() {
        assert_eq!(feed_url_from(None), FEED_URL);
        assert_eq!(
            feed_url_from(Some("http://127.0.0.1:8000/appcast-windows.xml".into())),
            "http://127.0.0.1:8000/appcast-windows.xml"
        );
        assert!(
            FEED_URL.starts_with(
                "https://github.com/shakedex/LunaApp/releases/latest/download/appcast-"
            )
        );
    }

    #[test]
    fn public_key_is_32_bytes() {
        let key = base64::engine::general_purpose::STANDARD
            .decode(public_key())
            .unwrap();
        assert_eq!(key.len(), 32);
    }
}
