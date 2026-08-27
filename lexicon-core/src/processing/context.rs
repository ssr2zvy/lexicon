use std::path::Path;

use crate::session::{SessionDataPaths, SessionIdentity};

/// Bound processing context provided to processing source handlers.
///
/// Constructed from an admitted processing invocation and validated session data paths.
///
/// The `Default` implementation has been removed. Unbound processing contexts are not
/// supported in the managed runner path.
pub struct ProcessingContext {
    paths: SessionDataPaths,
    session_identity: SessionIdentity,
}

impl ProcessingContext {
    pub fn from_session_data_paths(paths: SessionDataPaths, session_identity: SessionIdentity) -> Self {
        Self {
            paths,
            session_identity,
        }
    }

    pub fn protocol_root(&self) -> &Path { self.paths.protocol_root() }
    pub fn operation_root(&self) -> &Path { self.paths.operation_root() }
    pub fn session_directory(&self) -> &Path { self.paths.session_directory() }
    pub fn raw_data_directory(&self) -> &Path { self.paths.raw_data_directory() }
    pub fn processed_data_directory(&self) -> &Path { self.paths.processed_data_directory() }
    pub fn session_identity(&self) -> &SessionIdentity { &self.session_identity }

    #[cfg(test)]
    pub(crate) fn new_for_tests() -> Self {
        Self {
            paths: SessionDataPaths::from_legacy_parts(
                std::path::PathBuf::from("/test/project/sources/test-source/http"),
                std::path::PathBuf::from("/test/project/sources/test-source/http/process-data"),
                std::path::PathBuf::from("/test/project/sources/test-source/http/process-data/sessions/test-session"),
                std::path::PathBuf::from("/test/project/sources/test-source/http/data/raw"),
                std::path::PathBuf::from("/test/project/sources/test-source/http/data/processed"),
            ),
            session_identity: SessionIdentity::new("test-session").expect("valid test session id"),
        }
    }
}

impl std::fmt::Debug for ProcessingContext {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ProcessingContext")
            .field("operation_root", &self.paths.operation_root())
            .finish_non_exhaustive()
    }
}
