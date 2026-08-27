use std::path::{Path, PathBuf};

pub mod processing;
pub mod protocols;
pub mod runtime;
pub mod session;

pub use protocols::http;
pub use runtime::{
    MissingHttpCapabilities, OwnedRuntimeIdentity, RuntimeCompatibilityError,
    RuntimeIdentifierError, RuntimeIdentity, RuntimeInformationDecodingError,
    RuntimeInformationEncodingError, RuntimeOperation, RuntimeProtocol,
};
pub use session::{
    RUNTIME_CONTEXT_ENVIRONMENT_VARIABLE, RuntimeContextPaths, SessionDataPaths, SessionIdentity,
};

// ---------------------------------------------------------------------------
// HttpAcquisitionContext
// ---------------------------------------------------------------------------

/// Bound acquisition context provided to HTTP source handlers.
///
/// Constructed from an admitted HTTP invocation and validated session data paths.
///
/// # Legacy note
///
/// The old `LEXICON_SOURCE_DIRECTORY`-based construction path (`from_env`) is
/// quarantined and unsupported for managed runners after this milestone.
pub struct HttpAcquisitionContext {
    paths: SessionDataPaths,
    session_identity: Option<SessionIdentity>,
}

impl HttpAcquisitionContext {
    pub fn from_session_data_paths(paths: SessionDataPaths, session_identity: SessionIdentity) -> Self {
        Self {
            paths,
            session_identity: Some(session_identity),
        }
    }

    /// Compatibility accessor; returns protocol root in managed mode.
    pub fn source_directory(&self) -> &Path {
        self.paths.protocol_root()
    }

    pub fn protocol_root(&self) -> &Path { self.paths.protocol_root() }
    pub fn operation_root(&self) -> &Path { self.paths.operation_root() }
    pub fn session_directory(&self) -> &Path { self.paths.session_directory() }
    pub fn raw_data_directory(&self) -> &Path { self.paths.raw_data_directory() }
    pub fn processed_data_directory(&self) -> &Path { self.paths.processed_data_directory() }
    pub fn session_identity(&self) -> Option<&SessionIdentity> { self.session_identity.as_ref() }

    /// **Quarantined legacy constructor.** Uses `LEXICON_SOURCE_DIRECTORY` environment variable.
    ///
    /// This path is unsupported for managed runners as of the session milestone.
    /// It remains available only to support the legacy `run_http_source` API.
    #[doc(hidden)]
    pub fn from_env_legacy() -> Result<Self, String> {
        let value = std::env::var("LEXICON_SOURCE_DIRECTORY")
            .map_err(|_| "missing LEXICON_SOURCE_DIRECTORY; the runtime must supply the absolute source directory".to_string())?;
        let source_directory = PathBuf::from(value);

        if source_directory.is_relative() {
            return Err(format!(
                "invalid LEXICON_SOURCE_DIRECTORY '{}': must be an absolute path",
                source_directory.display()
            ));
        }

        if !source_directory.is_dir() {
            return Err(format!(
                "invalid LEXICON_SOURCE_DIRECTORY '{}': path does not exist or is not a directory",
                source_directory.display()
            ));
        }

        let raw_data_directory = source_directory.join("data/raw");
        let processed_data_directory = source_directory.join("data/processed");
        let operation_root = source_directory.join("get-raw-data");
        let session_directory = operation_root.join("sessions/legacy");
        Ok(Self {
            paths: SessionDataPaths::from_legacy_parts(
                source_directory,
                operation_root,
                session_directory,
                raw_data_directory,
                processed_data_directory,
            ),
            session_identity: None,
        })
    }
}

pub trait HttpAcquisition {
    fn acquire(&self, context: &mut HttpAcquisitionContext) -> Result<(), String>;
}

/// **Legacy entry point.** Uses the quarantined `LEXICON_SOURCE_DIRECTORY` path.
///
/// New managed-runner code should use the session-aware runner instead.
pub fn run_http_source<A>(acquisition: A) -> Result<(), String>
where
    A: HttpAcquisition,
{
    let mut context = HttpAcquisitionContext::from_env_legacy()?;
    acquisition.acquire(&mut context)
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;
    use std::sync::{Mutex, OnceLock};
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::{HttpAcquisition, HttpAcquisitionContext, run_http_source};

    static ENV_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

    struct Dummy;

    impl HttpAcquisition for Dummy {
        fn acquire(&self, context: &mut HttpAcquisitionContext) -> Result<(), String> {
            assert_eq!(
                context.raw_data_directory(),
                context.source_directory().join("data/raw")
            );
            Ok(())
        }
    }

    fn with_temp_source_directory(test: impl FnOnce(&PathBuf)) {
        let lock = ENV_LOCK.get_or_init(|| Mutex::new(())).lock().unwrap();
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let source_directory =
            std::env::temp_dir().join(format!("lexicon-http-context-{timestamp}"));
        fs::create_dir_all(&source_directory).unwrap();

        let prior = std::env::var("LEXICON_SOURCE_DIRECTORY").ok();
        unsafe {
            std::env::set_var("LEXICON_SOURCE_DIRECTORY", &source_directory);
        }

        test(&source_directory);

        match prior {
            Some(value) => unsafe { std::env::set_var("LEXICON_SOURCE_DIRECTORY", value) },
            None => unsafe { std::env::remove_var("LEXICON_SOURCE_DIRECTORY") },
        }
        let _ = fs::remove_dir_all(&source_directory);
        drop(lock);
    }

    #[test]
    fn run_http_source_populates_source_and_raw_data_paths() {
        with_temp_source_directory(|source_directory| {
            let result = run_http_source(Dummy);

            assert!(result.is_ok(), "result: {result:?}");
            assert_eq!(
                std::env::var("LEXICON_SOURCE_DIRECTORY").unwrap(),
                source_directory.to_string_lossy().to_string()
            );
        });
    }
}
