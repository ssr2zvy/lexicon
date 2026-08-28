//! Test-only fixtures for exercising the real runtime-invocation execution path.
//!
//! `run_http_runtime_invocation` and `run_processing_runtime_invocation` both require,
//! before any handler is reached: a `SessionStore`-backed `Prepared` session record, an
//! owned session lease, and a valid `LEXICON_RUNTIME_CONTEXT_V1` environment value whose
//! paths satisfy `RuntimeContextPaths`'s invariants. This module builds exactly that
//! minimum real environment so execution tests reach the actual production handler
//! dispatch path instead of stopping at `Session(ContextDecode(MissingEnvironmentVariable))`.
//!
//! No production, non-test API is added, altered, or bypassed here: every constructor
//! used below (`SessionStore::open`, `SessionStore::create_prepared`,
//! `SessionStore::acquire_lease`, `RuntimeContextPaths::new`, `encode_runtime_context`) is
//! the same one a real runtime invocation uses.
#![cfg(test)]

use std::ffi::OsString;
use std::sync::{Mutex, MutexGuard, OnceLock};

use crate::runtime::{
    ProjectInvocationIdentity, RuntimeExecutionMode, RuntimeIdentity, RuntimeInvocationEnvelopeV1,
    RuntimeOperation, RuntimeSupervisionMode, SessionInvocationIdentity,
};
use crate::session::context::{
    RUNTIME_CONTEXT_ENVIRONMENT_VARIABLE, RuntimeContextPaths, encode_runtime_context,
};
use crate::session::model::{NewSessionRecord, SessionOperation};
use crate::session::store::{SessionOperationRoot, SessionStore};

/// Serializes access to the process-global `LEXICON_RUNTIME_CONTEXT_V1` environment
/// variable across concurrently running tests within this crate.
fn env_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

/// RAII guard that sets `LEXICON_RUNTIME_CONTEXT_V1` for the duration of a fixture and
/// restores the prior value on drop, including during panic unwinding. Holds the
/// serialization lock for its entire lifetime so no other fixture can observe or
/// mutate the environment variable concurrently.
struct RuntimeContextEnvGuard {
    _lock: MutexGuard<'static, ()>,
    previous: Option<OsString>,
}

impl RuntimeContextEnvGuard {
    fn set(value: &str) -> Self {
        let lock = env_lock()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let previous = std::env::var_os(RUNTIME_CONTEXT_ENVIRONMENT_VARIABLE);
        // SAFETY: access to this variable is serialized by `_lock` for the lifetime of
        // every `RuntimeContextEnvGuard`, and no other code in this crate sets it.
        unsafe {
            std::env::set_var(RUNTIME_CONTEXT_ENVIRONMENT_VARIABLE, value);
        }
        Self {
            _lock: lock,
            previous,
        }
    }
}

impl Drop for RuntimeContextEnvGuard {
    fn drop(&mut self) {
        // SAFETY: see `set`; still covered by the held `_lock` during this restore.
        unsafe {
            match self.previous.take() {
                Some(value) => std::env::set_var(RUNTIME_CONTEXT_ENVIRONMENT_VARIABLE, value),
                None => std::env::remove_var(RUNTIME_CONTEXT_ENVIRONMENT_VARIABLE),
            }
        }
    }
}

/// A real, minimal runtime-invocation execution environment for one operation.
///
/// Owns the temporary directory tree, the held session lease, and the environment-variable
/// guard for its entire lifetime; drop this value only after the invocation under test has
/// returned.
pub(crate) struct RuntimeInvocationFixture {
    _temp: tempfile::TempDir,
    _lease: crate::session::lease::SessionLease,
    _env_guard: RuntimeContextEnvGuard,
    store: SessionStore,
    project: ProjectInvocationIdentity,
    runtime_identity: RuntimeIdentity,
    session: SessionInvocationIdentity,
    execution_mode: RuntimeExecutionMode,
    supervision_mode: RuntimeSupervisionMode,
}

impl RuntimeInvocationFixture {
    /// Build a fixture for `runtime_identity`'s operation, with the given execution and
    /// supervision modes. Creates `protocol_root/data/{raw,processed}` up front; the
    /// `operation_root/sessions/<id>` directory is created by `create_prepared`.
    pub(crate) fn new(
        runtime_identity: RuntimeIdentity,
        execution_mode: RuntimeExecutionMode,
        supervision_mode: RuntimeSupervisionMode,
    ) -> Self {
        let operation = runtime_identity.operation();

        let temp = tempfile::tempdir().expect("tempdir for runtime invocation fixture");
        let project_root = temp.path().join("project");
        let protocol_root = project_root
            .join("sources")
            .join(runtime_identity.source_name())
            .join("http");
        let raw_data_directory = protocol_root.join("data").join("raw");
        let processed_data_directory = protocol_root.join("data").join("processed");
        let operation_segment = match operation {
            RuntimeOperation::Acquisition => "get-raw-data",
            RuntimeOperation::Processing => "process-data",
        };
        let operation_root = protocol_root.join(operation_segment);
        // Processing transaction discovery validates `protocol_root/get-raw-data` as an
        // existing managed directory even when it is not this invocation's own operation
        // root. For an Acquisition fixture this is the same path as `operation_root` and
        // `create_dir_all` is idempotent, so it is always created up front.
        let acquisition_root = protocol_root.join("get-raw-data");

        std::fs::create_dir_all(&raw_data_directory).expect("create raw data directory");
        std::fs::create_dir_all(&processed_data_directory)
            .expect("create processed data directory");
        std::fs::create_dir_all(&acquisition_root).expect("create acquisition root directory");

        let project = ProjectInvocationIdentity::new("example-project").unwrap();

        let store = SessionStore::open(
            SessionOperationRoot::new(operation_root.clone()).expect("valid operation root"),
        )
        .expect("open session store");

        let prepared = store
            .create_prepared(NewSessionRecord {
                project: project.clone(),
                runtime: runtime_identity.into_owned_identity(),
                operation: SessionOperation::from_runtime_operation(operation),
                execution_mode,
                supervision_mode,
            })
            .expect("create prepared session");
        let session = prepared.record().session().clone();

        let lease = store
            .acquire_lease(&session)
            .expect("acquire session lease");

        let session_directory = operation_root.join("sessions").join(session.id());
        let paths = RuntimeContextPaths::new(
            project_root,
            protocol_root,
            operation_root,
            session_directory,
            raw_data_directory,
            processed_data_directory,
            operation,
            &session,
        )
        .expect("valid runtime context paths");

        let env_value =
            encode_runtime_context(&project, &runtime_identity.into_owned_identity(), &session, &paths)
                .expect("encode runtime context");
        let env_guard = RuntimeContextEnvGuard::set(&env_value);

        Self {
            _temp: temp,
            _lease: lease,
            _env_guard: env_guard,
            store,
            project,
            runtime_identity,
            session,
            execution_mode,
            supervision_mode,
        }
    }

    /// Convenience constructor for `RuntimeExecutionMode::Run` + `Foreground`.
    pub(crate) fn foreground_run(runtime_identity: RuntimeIdentity) -> Self {
        Self::new(
            runtime_identity,
            RuntimeExecutionMode::Run,
            RuntimeSupervisionMode::Foreground,
        )
    }

    /// Convenience constructor for `RuntimeExecutionMode::Run` + `Background`.
    pub(crate) fn background_run(runtime_identity: RuntimeIdentity) -> Self {
        Self::new(
            runtime_identity,
            RuntimeExecutionMode::Run,
            RuntimeSupervisionMode::Background,
        )
    }

    pub(crate) fn session(&self) -> &SessionInvocationIdentity {
        &self.session
    }

    pub(crate) fn store(&self) -> &SessionStore {
        &self.store
    }

    /// Build CLI argv (`--lexicon-invocation-v1 <json> --` + `source_arguments`) for this
    /// fixture's envelope.
    pub(crate) fn build_argv(&self, source_arguments: &[OsString]) -> Vec<OsString> {
        let envelope = RuntimeInvocationEnvelopeV1::new(
            self.project.clone(),
            self.runtime_identity,
            self.session.clone(),
            self.execution_mode,
            self.supervision_mode,
        )
        .expect("valid runtime invocation envelope");

        let mut argv = vec![
            OsString::from("--lexicon-invocation-v1"),
            OsString::from(envelope.to_json().expect("encode envelope")),
            OsString::from("--"),
        ];
        argv.extend_from_slice(source_arguments);
        argv
    }
}
