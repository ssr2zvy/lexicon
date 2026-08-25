pub mod contract;
pub mod error;

pub use contract::{HttpAcquireFn, HttpCapability, HttpResumeFn, HttpSourceContractV1};
pub use error::{AcquisitionError, AcquisitionResult};

pub use crate::{
    HttpAcquisition,
    HttpAcquisitionContext,
    run_http_source,
};

pub mod runner {
    use std::ffi::OsString;
    use std::process::ExitCode;

    use crate::HttpAcquisitionContext;

    use super::{HttpSourceContractV1, RuntimeIdentity};

    pub fn run(identity: RuntimeIdentity, source: &HttpSourceContractV1) -> ExitCode {
        let _ = identity;
        let mut context = match HttpAcquisitionContext::from_env() {
            Ok(context) => context,
            Err(error) => {
                eprintln!("{error}");
                return ExitCode::FAILURE;
            }
        };

        let args: Vec<OsString> = std::env::args_os().skip(1).collect();
        match source.acquire(&mut context, &args) {
            Ok(()) => ExitCode::SUCCESS,
            Err(error) => {
                eprintln!("{error}");
                ExitCode::FAILURE
            }
        }
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct RuntimeIdentity {
    kind: &'static str,
    source: &'static str,
    version: u32,
}

impl RuntimeIdentity {
    pub const fn http_acquisition(source: &'static str, version: u32) -> Self {
        Self {
            kind: "http-acquisition",
            source,
            version,
        }
    }

    pub const fn as_parts(&self) -> (&'static str, &'static str, u32) {
        (self.kind, self.source, self.version)
    }
}
