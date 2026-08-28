pub mod processing;
pub mod protocols;
pub mod runtime;
pub mod session;

pub use protocols::http;
pub use protocols::http::HttpAcquisitionContext;
pub use runtime::{
    CORE_CONTRACT_VERSION, MANAGED_RUNNER_TEMPLATE_VERSION, MissingHttpCapabilities,
    OwnedRuntimeIdentity, RUNTIME_INVOCATION_PROTOCOL_VERSION, RUNTIME_PROTOCOL_VERSION,
    RuntimeCompatibilityError, RuntimeIdentifierError, RuntimeIdentity,
    RuntimeInformationDecodingError, RuntimeInformationEncodingError, RuntimeOperation,
    RuntimeProtocol,
};
pub use session::{
    RUNTIME_CONTEXT_ENVIRONMENT_VARIABLE, RuntimeContextPaths, SessionDataPaths, SessionIdentity,
};

pub use rusqlite;

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
