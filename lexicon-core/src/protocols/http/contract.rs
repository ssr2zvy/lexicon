use std::ffi::OsString;

use crate::HttpAcquisitionContext;

use super::error::AcquisitionResult;

pub type HttpAcquireFn = fn(
    &mut HttpAcquisitionContext,
    &[OsString],
) -> AcquisitionResult<()>;

#[derive(Clone, Copy)]
pub struct HttpSourceContractV1 {
    acquire: HttpAcquireFn,
}

impl HttpSourceContractV1 {
    pub const fn new(acquire: HttpAcquireFn) -> Self {
        Self { acquire }
    }

    pub const fn acquire(&self) -> HttpAcquireFn {
        self.acquire
    }
}

#[cfg(test)]
mod tests {
    use std::ffi::OsString;
    use std::path::PathBuf;

    use super::{AcquisitionResult, HttpSourceContractV1};
    use crate::HttpAcquisitionContext;
    use crate::protocols::http::error::AcquisitionError;

    fn acquire_handler(
        context: &mut HttpAcquisitionContext,
        args: &[OsString],
    ) -> AcquisitionResult<()> {
        assert_eq!(context.source_directory, PathBuf::from("/tmp/source"));
        assert_eq!(args, &[OsString::from("alpha"), OsString::from("beta")]);
        Ok(())
    }

    const SOURCE: HttpSourceContractV1 = HttpSourceContractV1::new(acquire_handler);

    #[test]
    fn correctly_typed_function_constructs_source_contract() {
        let contract = HttpSourceContractV1::new(acquire_handler);
        let contract_ptr = contract.acquire() as *const (); 
        let handler_ptr = acquire_handler as *const ();
        assert_eq!(contract_ptr, handler_ptr);
    }

    #[test]
    fn source_contract_can_be_declared_as_const() {
        let contract_ptr = SOURCE.acquire() as *const (); 
        let handler_ptr = acquire_handler as *const ();
        assert_eq!(contract_ptr, handler_ptr);
    }

    #[test]
    fn retained_handler_receives_same_context_and_args() {
        let mut context = HttpAcquisitionContext {
            source_directory: PathBuf::from("/tmp/source"),
            raw_data_directory: PathBuf::from("/tmp/source/data/raw"),
        };
        let args = [OsString::from("alpha"), OsString::from("beta")];

        let outcome = SOURCE.acquire()(&mut context, &args);

        assert!(outcome.is_ok(), "result: {outcome:?}");
    }

    #[test]
    fn acquisition_error_preserves_and_displays_message() {
        let error = AcquisitionError::source_message("network unreachable");

        assert_eq!(error.message(), "network unreachable");
        assert_eq!(error.to_string(), "network unreachable");
    }
}
