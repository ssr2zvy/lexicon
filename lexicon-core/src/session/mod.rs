pub mod binding;
pub mod context;
pub mod error;
pub mod handoff;
pub mod lease;
pub mod model;
pub mod store;
#[cfg(test)]
pub(crate) mod test_support;
pub(crate) mod transition;

pub use binding::{
    BoundRuntimeSession, RunningRuntimeSession, RuntimeSessionBindingError, bind_runtime_session,
};
pub use context::{
    DecodedRuntimeContext, RUNTIME_CONTEXT_ENVIRONMENT_VARIABLE, RuntimeContextPaths,
    SessionDataPaths, decode_runtime_context, decode_runtime_context_from_env,
    encode_runtime_context,
};
pub use error::{
    CoreRunnerSessionError, RuntimeContextDecodingError, RuntimeContextEncodingError,
    RuntimeContextError, SessionDecodingError, SessionEncodingError, SessionLeaseError,
    SessionStoreError, SessionTransitionError,
};
pub use handoff::{
    HANDOFF_DIGEST_BYTES, HANDOFF_SCHEMA_VERSION, HANDOFF_TOKEN_BYTES, HandoffAuthorityStateV1,
    HandoffOwnsershipEvidenceRecordV1, HandoffReservationRecordV1,
    HandoffAcknowledgementRecordV1, HandoffRevocationReasonV1, HandoffToken,
    HandoffTokenDigest, OperatorIdentityV1, generate_instance_nonce,
};
pub use lease::{SessionLease, SessionLeaseState, inspect_session_lease};
pub use model::{
    MAX_FAILURE_SUMMARY_BYTES, NewSessionRecord, ProjectIdentity, SafeSessionFailure,
    SESSION_SCHEMA_VERSION, SessionClock, SessionFailureCode, SessionFailureKind, SessionFailureV1,
    SessionIdentity, SessionOperation, SessionRecordV1, SessionState, SessionStatusV1,
    SessionTimestamp, SessionTransition, SystemClock, generate_session_id,
};
pub use store::{PreparedSession, SessionOperationRoot, SessionStore};
