use std::fmt;

use crate::runtime::invocation::RuntimeInvocationEnvelopeV1;
use crate::runtime::{RuntimeExecutionMode, RuntimeSupervisionMode};
use crate::session::error::{SessionLeaseError, SessionStoreError};
use crate::session::lease::SessionLease;
use crate::session::model::{
    MAX_FAILURE_SUMMARY_BYTES, SessionFailureKind, SessionRecordV1, SessionState, SessionTransition,
};
use crate::session::store::SessionStore;

// ---------------------------------------------------------------------------
// RuntimeSessionBindingError
// ---------------------------------------------------------------------------

/// Error returned when the runtime invocation envelope does not agree with the
/// durable session record found in the store.
#[derive(Debug)]
pub enum RuntimeSessionBindingError {
    /// The session record could not be loaded.
    StoreLoad(SessionStoreError),
    /// An identity field in the envelope does not match the durable record.
    IdentityMismatch {
        field: &'static str,
        expected: String,
        actual: String,
    },
    /// The session is not in the `Prepared` state and therefore cannot be bound.
    SessionNotPrepared { actual_state: SessionState },
    /// The execution mode in the envelope does not match the durable record.
    ExecutionModeMismatch {
        expected: RuntimeExecutionMode,
        actual: RuntimeExecutionMode,
    },
    /// The supervision mode in the envelope does not match the durable record.
    SupervisionModeMismatch {
        expected: RuntimeSupervisionMode,
        actual: RuntimeSupervisionMode,
    },
}

impl fmt::Display for RuntimeSessionBindingError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::StoreLoad(err) => write!(f, "session record load failed: {err}"),
            Self::IdentityMismatch { field, expected, actual } => {
                write!(
                    f,
                    "session identity mismatch for {field}: envelope says '{expected}', record says '{actual}'"
                )
            }
            Self::SessionNotPrepared { actual_state } => {
                write!(
                    f,
                    "session cannot be bound: expected Prepared state, found {actual_state:?}"
                )
            }
            Self::ExecutionModeMismatch { expected, actual } => {
                write!(
                    f,
                    "execution mode mismatch: envelope says '{expected:?}', record says '{actual:?}'"
                )
            }
            Self::SupervisionModeMismatch { expected, actual } => {
                write!(
                    f,
                    "supervision mode mismatch: envelope says '{expected:?}', record says '{actual:?}'"
                )
            }
        }
    }
}

impl std::error::Error for RuntimeSessionBindingError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::StoreLoad(err) => Some(err),
            _ => None,
        }
    }
}

// ---------------------------------------------------------------------------
// bind_runtime_session
// ---------------------------------------------------------------------------

/// Bind an admitted runtime invocation to the existing prepared session in `store`.
///
/// Validates exact agreement for project, runtime source, protocol, operation,
/// source contract version, session ID, execution mode, and supervision mode.
///
/// Returns a `BoundRuntimeSession` that can be advanced to `Running` once the
/// caller acquires the session lease.
pub fn bind_runtime_session<'store>(
    store: &'store SessionStore,
    envelope: &RuntimeInvocationEnvelopeV1,
) -> Result<BoundRuntimeSession<'store>, RuntimeSessionBindingError> {
    let session_id = envelope.session();
    let record = store.load(session_id).map_err(RuntimeSessionBindingError::StoreLoad)?;

    // Require Prepared state.
    if record.state() != SessionState::Prepared {
        return Err(RuntimeSessionBindingError::SessionNotPrepared {
            actual_state: record.state(),
        });
    }

    // Project name
    if record.project().name() != envelope.project().name() {
        return Err(RuntimeSessionBindingError::IdentityMismatch {
            field: "project",
            expected: envelope.project().name().to_string(),
            actual: record.project().name().to_string(),
        });
    }

    // Runtime source name
    let env_runtime = envelope.runtime();
    if record.runtime().source_name() != env_runtime.source_name() {
        return Err(RuntimeSessionBindingError::IdentityMismatch {
            field: "runtime.source",
            expected: env_runtime.source_name().to_string(),
            actual: record.runtime().source_name().to_string(),
        });
    }

    // Protocol
    if record.runtime().protocol() != env_runtime.protocol() {
        return Err(RuntimeSessionBindingError::IdentityMismatch {
            field: "runtime.protocol",
            expected: format!("{:?}", env_runtime.protocol()),
            actual: format!("{:?}", record.runtime().protocol()),
        });
    }

    // Operation
    if record.runtime().operation() != env_runtime.operation() {
        return Err(RuntimeSessionBindingError::IdentityMismatch {
            field: "runtime.operation",
            expected: format!("{:?}", env_runtime.operation()),
            actual: format!("{:?}", record.runtime().operation()),
        });
    }

    // Source contract version
    if record.runtime().source_contract_version() != env_runtime.source_contract_version() {
        return Err(RuntimeSessionBindingError::IdentityMismatch {
            field: "runtime.source_contract_version",
            expected: env_runtime.source_contract_version().to_string(),
            actual: record.runtime().source_contract_version().to_string(),
        });
    }

    // Session ID
    if record.session().id() != session_id.id() {
        return Err(RuntimeSessionBindingError::IdentityMismatch {
            field: "session",
            expected: session_id.id().to_string(),
            actual: record.session().id().to_string(),
        });
    }

    // Execution mode
    if record.execution_mode() != envelope.execution_mode() {
        return Err(RuntimeSessionBindingError::ExecutionModeMismatch {
            expected: envelope.execution_mode(),
            actual: record.execution_mode(),
        });
    }

    // Supervision mode
    if record.supervision_mode() != envelope.supervision_mode() {
        return Err(RuntimeSessionBindingError::SupervisionModeMismatch {
            expected: envelope.supervision_mode(),
            actual: record.supervision_mode(),
        });
    }

    Ok(BoundRuntimeSession { store, record })
}

// ---------------------------------------------------------------------------
// BoundRuntimeSession
// ---------------------------------------------------------------------------

/// A session that has been validated against a runtime invocation envelope.
///
/// The session record is in the `Prepared` state. Advance to `RunningRuntimeSession`
/// by calling `enter_running` once the caller holds the exclusive session lease.
///
/// No public constructor; obtained only via `bind_runtime_session`.
pub struct BoundRuntimeSession<'store> {
    store: &'store SessionStore,
    record: SessionRecordV1,
}

impl<'store> BoundRuntimeSession<'store> {
    pub fn record(&self) -> &SessionRecordV1 {
        &self.record
    }

    /// Transition the session to `Running`.
    ///
    /// The caller must hold `lease` before calling this method. The lease must
    /// belong to this session — validated by comparing its file path against the
    /// expected path derived from the store's operation root.
    pub fn enter_running(
        self,
        lease: &SessionLease,
    ) -> Result<RunningRuntimeSession<'store>, SessionStoreError> {
        validate_lease_for_session(self.store, self.record.session(), lease)?;

        let session_id = self.record.session().clone();
        let revision = self.record.revision();
        let updated = self
            .store
            .transition(&session_id, revision, SessionTransition::ToRunning)?;

        Ok(RunningRuntimeSession { store: self.store, record: updated })
    }
}

impl fmt::Debug for BoundRuntimeSession<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BoundRuntimeSession")
            .field("session", self.record.session())
            .field("revision", &self.record.revision())
            .finish_non_exhaustive()
    }
}

// ---------------------------------------------------------------------------
// RunningRuntimeSession
// ---------------------------------------------------------------------------

/// A session in the `Running` state.
///
/// Obtained by calling `BoundRuntimeSession::enter_running`. Provides consuming
/// methods for terminal transitions. The caller retains the session lease and
/// must pass it to each transition method for ownership proof.
///
/// No public constructor; obtained only via `BoundRuntimeSession::enter_running`.
pub struct RunningRuntimeSession<'store> {
    store: &'store SessionStore,
    record: SessionRecordV1,
}

impl<'store> RunningRuntimeSession<'store> {
    pub fn record(&self) -> &SessionRecordV1 {
        &self.record
    }

    /// Transition the session to `Succeeded`, consuming this value.
    pub fn complete(
        self,
        lease: &SessionLease,
    ) -> Result<SessionRecordV1, SessionStoreError> {
        validate_lease_for_session(self.store, self.record.session(), lease)?;

        let session_id = self.record.session().clone();
        let revision = self.record.revision();
        self.store
            .transition(&session_id, revision, SessionTransition::ToSucceeded)
    }

    /// Transition the session to `Failed` with `Source` failure kind, consuming this value.
    ///
    /// The error message is sanitized and truncated before storage.
    pub fn fail_source(
        self,
        lease: &SessionLease,
        error: &dyn std::error::Error,
    ) -> Result<SessionRecordV1, SessionStoreError> {
        validate_lease_for_session(self.store, self.record.session(), lease)?;

        let session_id = self.record.session().clone();
        let revision = self.record.revision();
        let summary = sanitize_error_message(error);
        self.store.transition(
            &session_id,
            revision,
            SessionTransition::ToFailed {
                kind: SessionFailureKind::Source,
                summary: Some(summary),
            },
        )
    }

    /// Transition the session to `Failed` with `Runtime` failure kind, consuming this value.
    ///
    /// The error message is sanitized and truncated before storage.
    pub fn fail_runtime(
        self,
        lease: &SessionLease,
        error: &dyn std::error::Error,
    ) -> Result<SessionRecordV1, SessionStoreError> {
        validate_lease_for_session(self.store, self.record.session(), lease)?;

        let session_id = self.record.session().clone();
        let revision = self.record.revision();
        let summary = sanitize_error_message(error);
        self.store.transition(
            &session_id,
            revision,
            SessionTransition::ToFailed {
                kind: SessionFailureKind::Runtime,
                summary: Some(summary),
            },
        )
    }
}

impl fmt::Debug for RunningRuntimeSession<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RunningRuntimeSession")
            .field("session", self.record.session())
            .field("revision", &self.record.revision())
            .finish_non_exhaustive()
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Verify that `lease` was acquired for the given session in `store`.
///
/// Compares the lease's file path against the canonical path derived from the
/// store's operation root and the session identity.
fn validate_lease_for_session(
    store: &SessionStore,
    session: &crate::session::model::SessionIdentity,
    lease: &SessionLease,
) -> Result<(), SessionStoreError> {
    let expected = store.operation_root().lease_path(session);
    if lease.path() != expected {
        return Err(SessionStoreError::LeaseRequired(SessionLeaseError::Io(
            std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "lease does not belong to this session",
            ),
        )));
    }
    Ok(())
}

/// Produce a sanitized diagnostic string from an error value.
///
/// Uses only `Display` formatting. Truncates at `MAX_FAILURE_SUMMARY_BYTES` on a
/// character boundary. Does not capture backtraces, source chains, arguments, or
/// any raw I/O content beyond the top-level message.
fn sanitize_error_message(error: &dyn std::error::Error) -> String {
    let raw = error.to_string();
    if raw.len() <= MAX_FAILURE_SUMMARY_BYTES {
        return raw;
    }
    let mut end = MAX_FAILURE_SUMMARY_BYTES;
    while !raw.is_char_boundary(end) {
        end -= 1;
    }
    raw[..end].to_string()
}
