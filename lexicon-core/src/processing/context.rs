use std::path::{Path, PathBuf};

use crate::session::RuntimeContextPaths;
use crate::session::store::RunningSession;

/// Bound processing context provided to processing source handlers.
///
/// Constructed from an admitted processing invocation, validated `RuntimeContextPaths`,
/// and an owned `RunningSession`. Fields are private; use the provided accessors.
///
/// The `Default` implementation has been removed. Unbound processing contexts are not
/// supported in the managed runner path.
pub struct ProcessingContext {
    project_root: PathBuf,
    protocol_root: PathBuf,
    operation_root: PathBuf,
    session_directory: PathBuf,
    raw_data_directory: PathBuf,
    processed_data_directory: PathBuf,
    running_session: Option<RunningSession>,
}

impl ProcessingContext {
    /// Construct a context from validated runtime context paths and a running session.
    pub fn from_context_paths(
        paths: &RuntimeContextPaths,
        running_session: RunningSession,
    ) -> Self {
        Self {
            project_root: paths.project_root().to_path_buf(),
            protocol_root: paths.protocol_root().to_path_buf(),
            operation_root: paths.operation_root().to_path_buf(),
            session_directory: paths.session_directory().to_path_buf(),
            raw_data_directory: paths.raw_data_directory().to_path_buf(),
            processed_data_directory: paths.processed_data_directory().to_path_buf(),
            running_session: Some(running_session),
        }
    }

    /// Take the running session out of the context, releasing ownership.
    pub(crate) fn take_running_session(&mut self) -> Option<RunningSession> {
        self.running_session.take()
    }

    pub fn project_root(&self) -> &Path { &self.project_root }
    pub fn protocol_root(&self) -> &Path { &self.protocol_root }
    pub fn operation_root(&self) -> &Path { &self.operation_root }
    pub fn session_directory(&self) -> &Path { &self.session_directory }
    pub fn raw_data_directory(&self) -> &Path { &self.raw_data_directory }
    pub fn processed_data_directory(&self) -> &Path { &self.processed_data_directory }

    #[cfg(test)]
    pub(crate) fn new_for_tests() -> Self {
        use std::path::PathBuf;
        Self {
            project_root: PathBuf::from("/test/project"),
            protocol_root: PathBuf::from("/test/project/sources/test-source/http"),
            operation_root: PathBuf::from("/test/project/sources/test-source/http/process-data"),
            session_directory: PathBuf::from("/test/project/sources/test-source/http/process-data/sessions/test-session"),
            raw_data_directory: PathBuf::from("/test/project/sources/test-source/http/data/raw"),
            processed_data_directory: PathBuf::from("/test/project/sources/test-source/http/data/processed"),
            running_session: None,
        }
    }
}

impl std::fmt::Debug for ProcessingContext {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ProcessingContext")
            .field("operation_root", &self.operation_root)
            .finish_non_exhaustive()
    }
}
