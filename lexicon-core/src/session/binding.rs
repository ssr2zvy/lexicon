use std::fmt;

use crate::runtime::invocation::RuntimeInvocationEnvelopeV1;
use crate::runtime::{RuntimeExecutionMode, RuntimeSupervisionMode};
use crate::session::model::{
    SafeSessionFailure, SessionFailureCode, SessionRecordV1, SessionState, SessionTransition,
};
use crate::session::error::{SessionLeaseError, SessionStoreError};
use crate::session::lease::SessionLeaseState;
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
    SupervisorLeaseUnavailable,
    SupervisorLeaseInspectionFailed(SessionLeaseError),
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
            Self::SupervisorLeaseUnavailable => {
                f.write_str("supervisor lease is not currently owned for this session")
            }
            Self::SupervisorLeaseInspectionFailed(err) => {
                write!(f, "failed to inspect supervisor lease state: {err}")
            }
        }
    }
}

impl std::error::Error for RuntimeSessionBindingError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::StoreLoad(err) => Some(err),
            Self::SupervisorLeaseInspectionFailed(err) => Some(err),
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
/// Returns a `BoundRuntimeSession` that can be advanced to `Running` only after
/// confirming an active external supervisor lease owner exists.
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

    match store.inspect_lease_state(session_id) {
        Ok(SessionLeaseState::Owned) => {}
        Ok(SessionLeaseState::Available) => {
            return Err(RuntimeSessionBindingError::SupervisorLeaseUnavailable)
        }
        Err(err) => {
            return Err(RuntimeSessionBindingError::SupervisorLeaseInspectionFailed(err))
        }
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
    pub fn enter_running(self) -> Result<RunningRuntimeSession<'store>, SessionStoreError> {
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
/// methods for terminal transitions.
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
    pub fn complete(self) -> Result<SessionRecordV1, SessionStoreError> {
        let session_id = self.record.session().clone();
        let revision = self.record.revision();
        self.store
            .transition(&session_id, revision, SessionTransition::ToSucceeded)
    }

    /// Transition the session to `Failed` with `Source` failure kind, consuming this value.
    ///
    pub fn fail_source(self) -> Result<SessionRecordV1, SessionStoreError> {
        let session_id = self.record.session().clone();
        let revision = self.record.revision();
        self.store.transition(
            &session_id,
            revision,
            SessionTransition::ToFailed {
                failure: SafeSessionFailure::source_failure(),
            },
        )
    }

    /// Transition the session to `Failed` with `Runtime` failure kind, consuming this value.
    ///
    pub fn fail_runtime(
        self,
        code: SessionFailureCode,
        diagnostic: Option<String>,
    ) -> Result<SessionRecordV1, SessionStoreError> {
        let session_id = self.record.session().clone();
        let revision = self.record.revision();
        self.store.transition(
            &session_id,
            revision,
            SessionTransition::ToFailed {
                failure: SafeSessionFailure::runtime_failure(code, diagnostic),
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
