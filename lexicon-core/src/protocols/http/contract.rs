use std::ffi::OsString;

use crate::HttpAcquisitionContext;

use super::{HttpCapability, HttpCapabilitySet};
use super::error::AcquisitionResult;

pub type HttpAcquireFn = fn(
    &mut HttpAcquisitionContext,
    &[OsString],
) -> AcquisitionResult<()>;

pub type HttpResumeFn = fn(
    &mut HttpAcquisitionContext,
    &[OsString],
) -> AcquisitionResult<()>;

#[derive(Clone, Copy)]
pub struct HttpSourceContractV1 {
    acquire: HttpAcquireFn,
    resume: Option<HttpResumeFn>,
    required_capabilities: HttpCapabilitySet,
}

impl HttpSourceContractV1 {
    pub const CONTRACT_VERSION: u32 = 1;

    pub const fn new(acquire: HttpAcquireFn) -> Self {
        Self {
            acquire,
            resume: None,
            required_capabilities: HttpCapabilitySet::empty(),
        }
    }

    pub const fn acquire(&self) -> HttpAcquireFn {
        self.acquire
    }

    pub const fn with_resume(mut self, resume: HttpResumeFn) -> Self {
        self.resume = Some(resume);
        self
    }

    pub const fn resume_handler(&self) -> Option<HttpResumeFn> {
        self.resume
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

    fn resume_handler(
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
    const SOURCE_WITH_RESUME: HttpSourceContractV1 = HttpSourceContractV1::new(acquire_handler)
        .with_resume(resume_handler)
        .requires(HttpCapability::ClientCertificateV1);

    #[test]
    fn source_contract_without_requires_has_empty_requirement_set() {
        let contract = HttpSourceContractV1::new(acquire_handler);

        assert_eq!(contract.required_capabilities(), HttpCapabilitySet::empty());
        assert!(!contract.required_capabilities().contains(HttpCapability::ClientCertificateV1));
    }

    #[test]
    fn resume_handler_is_none_by_default() {
        let contract = HttpSourceContractV1::new(acquire_handler);

        assert!(contract.resume_handler().is_none());
    }

    #[test]
    fn source_contract_with_resume_can_be_declared_as_const() {
        let contract_ptr = SOURCE_WITH_RESUME.acquire() as *const ();
        let handler_ptr = acquire_handler as *const ();
        assert_eq!(contract_ptr, handler_ptr);

        let resume_ptr = SOURCE_WITH_RESUME.resume_handler().unwrap() as *const ();
        let resume_handler_ptr = resume_handler as *const ();
        assert_eq!(resume_ptr, resume_handler_ptr);
    }

    #[test]
    fn resume_handler_returns_some_after_registration() {
        let contract = HttpSourceContractV1::new(acquire_handler).with_resume(resume_handler);

        let resume_ptr = contract.resume_handler().unwrap() as *const ();
        let resume_handler_ptr = resume_handler as *const ();
        assert_eq!(resume_ptr, resume_handler_ptr);
    }

    #[test]
    fn retained_resume_handler_receives_same_context_and_args() {
        let mut context = HttpAcquisitionContext {
            source_directory: PathBuf::from("/tmp/source"),
            raw_data_directory: PathBuf::from("/tmp/source/data/raw"),
        };
        let args = [OsString::from("alpha"), OsString::from("beta")];

        let outcome = HttpSourceContractV1::new(acquire_handler)
            .with_resume(resume_handler)
            .resume_handler()
            .unwrap()(&mut context, &args);

        assert!(outcome.is_ok(), "result: {outcome:?}");
    }

    #[test]
    fn registering_resume_preserves_acquire_handler() {
        let contract = HttpSourceContractV1::new(acquire_handler).with_resume(resume_handler);
        let contract_ptr = contract.acquire() as *const ();
        let handler_ptr = acquire_handler as *const ();

        assert_eq!(contract_ptr, handler_ptr);

        let mut context = HttpAcquisitionContext {
            source_directory: PathBuf::from("/tmp/source"),
            raw_data_directory: PathBuf::from("/tmp/source/data/raw"),
        };
        let args = [OsString::from("alpha"), OsString::from("beta")];

        assert!(contract.acquire()(&mut context, &args).is_ok());
    }

    #[test]
    fn registering_resume_preserves_client_certificate_capability() {
        let contract = HttpSourceContractV1::new(acquire_handler)
            .with_resume(resume_handler)
            .requires(HttpCapability::ClientCertificateV1);

        let required = contract.required_capabilities();
        assert!(required.contains(HttpCapability::ClientCertificateV1));
        assert_eq!(
            required,
            HttpCapabilitySet::empty().insert(HttpCapability::ClientCertificateV1)
        );
    }

    #[test]
    fn second_resume_registration_replaces_previous_handler() {
        fn replacement_resume(
            context: &mut HttpAcquisitionContext,
            args: &[OsString],
        ) -> AcquisitionResult<()> {
            assert_eq!(context.source_directory, PathBuf::from("/tmp/source"));
            assert_eq!(args, &[OsString::from("alpha"), OsString::from("beta")]);
            Ok(())
        }

        let contract = HttpSourceContractV1::new(acquire_handler)
            .with_resume(resume_handler)
            .with_resume(replacement_resume);

        let resume_ptr = contract.resume_handler().unwrap() as *const ();
        let replacement_ptr = replacement_resume as *const ();
        assert_eq!(resume_ptr, replacement_ptr);
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
