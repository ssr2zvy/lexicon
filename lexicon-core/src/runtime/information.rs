use crate::protocols::http::{HttpCapabilitySet, HttpSourceContractV1};
use crate::runtime::RuntimeIdentity;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeInformationV1 {
    identity: RuntimeIdentity,
    descriptor_contract_version: u32,
    required_capabilities: HttpCapabilitySet,
    resume_handler_registered: bool,
}

impl RuntimeInformationV1 {
    pub const fn from_http_source(
        identity: RuntimeIdentity,
        source: &HttpSourceContractV1,
    ) -> Self {
        Self {
            identity,
            descriptor_contract_version: HttpSourceContractV1::CONTRACT_VERSION,
            required_capabilities: source.required_capabilities(),
            resume_handler_registered: source.resume_handler().is_some(),
        }
    }

    pub const fn identity(&self) -> RuntimeIdentity {
        self.identity
    }

    pub const fn descriptor_contract_version(&self) -> u32 {
        self.descriptor_contract_version
    }

    pub const fn required_capabilities(&self) -> HttpCapabilitySet {
        self.required_capabilities
    }

    pub const fn resume_handler_registered(&self) -> bool {
        self.resume_handler_registered
    }
}

#[cfg(test)]
mod tests {
    use super::RuntimeInformationV1;
    use crate::http::HttpCapability;
    use crate::protocols::http::{HttpCapabilitySet, HttpSourceContractV1};
    use crate::runtime::RuntimeIdentity;

    fn acquire_handler(
        _context: &mut crate::HttpAcquisitionContext,
        _args: &[std::ffi::OsString],
    ) -> crate::protocols::http::AcquisitionResult<()> {
        Ok(())
    }

    fn resume_handler(
        _context: &mut crate::HttpAcquisitionContext,
        _args: &[std::ffi::OsString],
    ) -> crate::protocols::http::AcquisitionResult<()> {
        Ok(())
    }

    fn failing_acquire(
        _context: &mut crate::HttpAcquisitionContext,
        _args: &[std::ffi::OsString],
    ) -> crate::protocols::http::AcquisitionResult<()> {
        panic!("acquire should not be invoked while building runtime metadata");
    }

    fn failing_resume(
        _context: &mut crate::HttpAcquisitionContext,
        _args: &[std::ffi::OsString],
    ) -> crate::protocols::http::AcquisitionResult<()> {
        panic!("resume should not be invoked while building runtime metadata");
    }

    #[test]
    fn runtime_information_can_be_constructed_in_const() {
        const IDENTITY: RuntimeIdentity = RuntimeIdentity::http_acquisition("example-source", 1);
        const SOURCE: HttpSourceContractV1 = HttpSourceContractV1::new(acquire_handler);
        const INFO: RuntimeInformationV1 = RuntimeInformationV1::from_http_source(IDENTITY, &SOURCE);

        assert_eq!(INFO.identity(), IDENTITY);
        assert_eq!(INFO.descriptor_contract_version(), HttpSourceContractV1::CONTRACT_VERSION);
        assert_eq!(INFO.required_capabilities(), HttpCapabilitySet::empty());
        assert!(!INFO.resume_handler_registered());
    }

    #[test]
    fn supplied_runtime_identity_is_preserved_exactly() {
        let identity = RuntimeIdentity::http_acquisition("example-source", 1);
        let source = HttpSourceContractV1::new(acquire_handler);

        let info = RuntimeInformationV1::from_http_source(identity, &source);

        assert_eq!(info.identity(), identity);
    }

    #[test]
    fn descriptor_contract_version_is_one() {
        let identity = RuntimeIdentity::http_acquisition("example-source", 1);
        let source = HttpSourceContractV1::new(acquire_handler);

        let info = RuntimeInformationV1::from_http_source(identity, &source);

        assert_eq!(info.descriptor_contract_version(), 1);
        assert_eq!(info.descriptor_contract_version(), HttpSourceContractV1::CONTRACT_VERSION);
    }

    #[test]
    fn empty_descriptor_produces_empty_required_capability_set() {
        let identity = RuntimeIdentity::http_acquisition("example-source", 1);
        let source = HttpSourceContractV1::new(acquire_handler);

        let info = RuntimeInformationV1::from_http_source(identity, &source);

        assert_eq!(info.required_capabilities(), HttpCapabilitySet::empty());
    }

    #[test]
    fn client_certificate_capability_is_retained_when_required() {
        let identity = RuntimeIdentity::http_acquisition("example-source", 1);
        let source = HttpSourceContractV1::new(acquire_handler)
            .requires(HttpCapability::ClientCertificateV1);

        let info = RuntimeInformationV1::from_http_source(identity, &source);

        assert!(info.required_capabilities().contains(HttpCapability::ClientCertificateV1));
        assert_eq!(
            info.required_capabilities(),
            HttpCapabilitySet::empty().insert(HttpCapability::ClientCertificateV1)
        );
    }

    #[test]
    fn resume_handler_registered_is_false_without_resume() {
        let identity = RuntimeIdentity::http_acquisition("example-source", 1);
        let source = HttpSourceContractV1::new(acquire_handler);

        let info = RuntimeInformationV1::from_http_source(identity, &source);

        assert!(!info.resume_handler_registered());
    }

    #[test]
    fn resume_handler_registered_is_true_with_resume() {
        let identity = RuntimeIdentity::http_acquisition("example-source", 1);
        let source = HttpSourceContractV1::new(acquire_handler).with_resume(resume_handler);

        let info = RuntimeInformationV1::from_http_source(identity, &source);

        assert!(info.resume_handler_registered());
    }

    #[test]
    fn constructing_runtime_information_does_not_invoke_acquire() {
        let identity = RuntimeIdentity::http_acquisition("example-source", 1);
        let source = HttpSourceContractV1::new(failing_acquire);

        let info = RuntimeInformationV1::from_http_source(identity, &source);

        assert_eq!(info.identity(), identity);
        assert!(!info.resume_handler_registered());
    }

    #[test]
    fn constructing_runtime_information_does_not_invoke_resume() {
        let identity = RuntimeIdentity::http_acquisition("example-source", 1);
        let source = HttpSourceContractV1::new(acquire_handler).with_resume(failing_resume);

        let info = RuntimeInformationV1::from_http_source(identity, &source);

        assert_eq!(info.identity(), identity);
        assert!(info.resume_handler_registered());
    }

    #[test]
    fn mismatched_identity_and_descriptor_versions_can_coexist() {
        let identity = RuntimeIdentity::http_acquisition("example-source", 2);
        let source = HttpSourceContractV1::new(acquire_handler);

        let info = RuntimeInformationV1::from_http_source(identity, &source);

        assert_eq!(info.identity().source_contract_version(), 2);
        assert_eq!(info.descriptor_contract_version(), HttpSourceContractV1::CONTRACT_VERSION);
    }

    #[test]
    fn runtime_and_http_export_paths_reference_same_type() {
        let left: crate::runtime::RuntimeInformationV1 = crate::http::RuntimeInformationV1::from_http_source(
            RuntimeIdentity::http_acquisition("example-source", 1),
            &HttpSourceContractV1::new(acquire_handler),
        );
        let right: crate::http::RuntimeInformationV1 = crate::runtime::RuntimeInformationV1::from_http_source(
            RuntimeIdentity::http_acquisition("example-source", 1),
            &HttpSourceContractV1::new(acquire_handler),
        );

        assert_eq!(left, right);
    }
}
