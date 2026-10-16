//! What Luna does with the update library's events: the title-bar mark, and
//! quitting when an update is about to install.

use luna_update::Event;

/// Whether an update is waiting to be installed (the title-bar mark).
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct UpdateState {
    pub pending: bool,
}

/// What the window must do after an event.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    None,
    /// The installer is about to run: quit.
    Quit,
    /// As [`Action::Quit`], cancelling the run in progress first, as the
    /// close button does.
    CancelAndQuit,
}

impl UpdateState {
    pub fn apply(&mut self, event: Event, run_in_progress: bool) -> Action {
        match event {
            Event::Found => self.pending = true,
            Event::NotFound | Event::Skipped => self.pending = false,
            Event::ShutdownRequested if run_in_progress => return Action::CancelAndQuit,
            Event::ShutdownRequested => return Action::Quit,
        }
        Action::None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn found_sets_mark() {
        let mut s = UpdateState::default();
        assert_eq!(s.apply(Event::Found, false), Action::None);
        assert!(s.pending);
    }

    #[test]
    fn skip_clears_mark() {
        let mut s = UpdateState::default();
        s.apply(Event::Found, false);
        s.apply(Event::Skipped, false);
        assert!(!s.pending);
    }

    #[test]
    fn not_found_clears_mark() {
        let mut s = UpdateState::default();
        s.apply(Event::Found, false);
        s.apply(Event::NotFound, false);
        assert!(!s.pending);
    }

    #[test]
    fn shutdown_request_cancels_run() {
        let mut s = UpdateState::default();
        assert_eq!(
            s.apply(Event::ShutdownRequested, true),
            Action::CancelAndQuit
        );
        assert_eq!(s.apply(Event::ShutdownRequested, false), Action::Quit);
    }
}
