use std::path::PathBuf;

pub mod protocols;
pub mod runtime;
pub use protocols::http;
pub use runtime::{
    RuntimeIdentifierError, RuntimeIdentity, RuntimeInformationDecodingError,
    RuntimeInformationEncodingError, RuntimeOperation, RuntimeProtocol,
};

pub struct HttpAcquisitionContext {
    pub source_directory: PathBuf,
    pub raw_data_directory: PathBuf,
}

impl HttpAcquisitionContext {
    pub fn from_env() -> Result<Self, String> {
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
        Ok(Self {
            source_directory,
            raw_data_directory,
        })
    }
}

pub trait HttpAcquisition {
    fn acquire(&self, context: &mut HttpAcquisitionContext) -> Result<(), String>;
}

pub fn run_http_source<A>(acquisition: A) -> Result<(), String>
where
    A: HttpAcquisition,
{
    let mut context = HttpAcquisitionContext::from_env()?;
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
                context.raw_data_directory,
                context.source_directory.join("data/raw")
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
