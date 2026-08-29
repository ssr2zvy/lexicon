//! FOREGROUND-01 cancellation plumbing between the CLI signal handlers
//! and the foreground supervisor's wait loop.
//!
//! A single atomic slot holds the most recently received cancellation
//! kind, and a tested helper drains it so `request_graceful_shutdown`
//! sees the same kind exactly once per supervisor invocation.

use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};

use crate::process::CancellationKind;

#[derive(Debug, Default)]
pub struct CancellationState {
    /// Set whenever a new cancellation has been received.
    received: AtomicBool,
    /// Protected slot holding the kind once the atomic flag is set. The
    /// atomic flag protects the read-write contract: producers set
    /// `kind` then `received`; consumers clear `received` then read
    /// `kind`.
    kind: Mutex<Option<CancellationKind>>,
}

impl CancellationState {
    pub fn new() -> Self {
        Self::default()
    }

    /// Record a cancellation kind. Called from signal handlers and
    /// console-control callbacks; never allocates or blocks.
    pub fn record(&self, kind: CancellationKind) {
        let mut slot = self.kind.lock().expect("poisoning");
        *slot = Some(kind);
        self.received.store(true, Ordering::SeqCst);
    }

    /// Poll for the most recent cancellation. After consuming, the
    /// receiver returns to `None` so a stale value is not re-delivered.
    pub fn poll(&self) -> Option<CancellationKind> {
        if !self.received.swap(false, Ordering::SeqCst) {
            return None;
        }
        let mut slot = self.kind.lock().expect("poisoning");
        slot.take()
    }
}

impl crate::process::CancellationSource for CancellationState {
    fn requested(&self) -> Option<CancellationKind> {
        self.poll()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_state_yields_no_cancellation() {
        let state = CancellationState::new();
        assert!(state.poll().is_none());
        assert!(state.poll().is_none());
    }

    #[test]
    fn recorded_kind_is_returned_once() {
        let state = CancellationState::new();
        state.record(CancellationKind::Interrupt);
        assert_eq!(state.poll(), Some(CancellationKind::Interrupt));
        assert!(state.poll().is_none());
    }

    #[test]
    fn second_record_replaces_first() {
        let state = CancellationState::new();
        state.record(CancellationKind::Interrupt);
        state.record(CancellationKind::Terminate);
        assert_eq!(state.poll(), Some(CancellationKind::Terminate));
        assert!(state.poll().is_none());
    }

    #[test]
    fn satisfies_cancellation_source() {
        let state = CancellationState::new();
        let source: &dyn crate::process::CancellationSource = &state;
        state.record(CancellationKind::Terminate);
        assert_eq!(source.requested(), Some(CancellationKind::Terminate));
        assert!(source.requested().is_none());
    }
}
