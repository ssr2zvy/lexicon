use std::ffi::OsString;

use crate::HttpAcquisitionContext;

use super::{HttpCapability, HttpCapabilitySet};
use super::error::AcquisitionResult;

pub type HttpAcquireFn = fn(
    &mut HttpAcquisitionContext,
    &[OsString],
) -> AcquisitionResult<()>;

#[derive(Clone, Copy)]
pub struct HttpSourceContractV1 {
    acquire: HttpAcquireFn,
    required_capabilities: HttpCapabilitySet,
}

impl HttpSourceContractV1 {
    pub const fn new(acquire: HttpAcquireFn) -> Self {
        Self {
            acquire,
            required_capabilities: HttpCapabilitySet::empty(),
        }
    }

    pub const fn acquire(&self) -> HttpAcquireFn {
        self.acquire
    }

    pub const fn requires(mut self, capability: HttpCapability) -> Self {
        self.required_capabilities = self.required_capabilities.insert(capability);
        self
    }

    pub const fn required_capabilities(&self) -> HttpCapabilitySet {
        self.required_capabilities
    }
}

#[cfg(test)]
mod tests {
    use std::ffi::OsString;
    use std::path::PathBuf;

    use super::{AcquisitionResult, HttpCapability, HttpCapabilitySet, HttpSourceContractV1};
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
    const SOURCE_WITH_CAPABILITY: HttpSourceContractV1 = HttpSourceContractV1::new(acquire_handler)
        .requires(HttpCapability::ClientCertificateV1);

    #[test]
    fn source_contract_without_requires_has_empty_requirement_set() {
        let contract = HttpSourceContractV1::new(acquire_handler);

        assert_eq!(contract.required_capabilities(), HttpCapabilitySet::empty());
        assert!(!contract.required_capabilities().contains(HttpCapability::ClientCertificateV1));
    }

    #[test]
    fn client_certificate_capability_can_be_declared_in_const_descriptor() {
        let required = SOURCE_WITH_CAPABILITY.required_capabilities();

        assert!(required.contains(HttpCapability::ClientCertificateV1));
        assert_eq!(
            required,
            HttpCapabilitySet::empty().insert(HttpCapability::ClientCertificateV1)
        );
    }

    #[test]
    fn repeated_requires_calls_are_idempotent() {
        let contract = HttpSourceContractV1::new(acquire_handler)
            .requires(HttpCapability::ClientCertificateV1)
            .requires(HttpCapability::ClientCertificateV1);

        assert_eq!(
            contract.required_capabilities(),
            HttpCapabilitySet::empty().insert(HttpCapability::ClientCertificateV1)
        );
    }

    #[test]
    fn requirement_registration_does_not_replace_acquire_handler() {
        let contract = HttpSourceContractV1::new(acquire_handler)
            .requires(HttpCapability::ClientCertificateV1);
        let contract_ptr = contract.acquire() as *const ();
        let handler_ptr = acquire_handler as *const ();

        assert_eq!(contract_ptr, handler_ptr);

        let mut context = HttpAcquisitionContext {
            source_directory: PathBuf::from("/tmp/source"),
            raw_data_directory: PathBuf::from("/tmp/source/data/raw"),
        };
        let args = [OsString::from("alpha"), OsString::from("beta")];

        let outcome = contract.acquire()(&mut context, &args);
        assert!(outcome.is_ok(), "result: {outcome:?}");
    }

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
