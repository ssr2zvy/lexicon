use std::fmt;

use super::ProcessingSourceContractV1;
use crate::runtime::{RuntimeIdentity, RuntimeOperation, RuntimeProtocol};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProcessingRuntimeInformationV1 {
    identity: RuntimeIdentity,
    descriptor_contract_version: u32,
}

impl ProcessingRuntimeInformationV1 {
    pub const fn identity(&self) -> RuntimeIdentity {
        self.identity
    }

    pub const fn descriptor_contract_version(&self) -> u32 {
        self.descriptor_contract_version
    }

    pub fn from_processing_source(
        identity: RuntimeIdentity,
        source: &ProcessingSourceContractV1,
    ) -> Result<Self, ProcessingRuntimeInformationConstructionError> {
        let _ = source;

        if identity.protocol() != RuntimeProtocol::Http {
            return Err(ProcessingRuntimeInformationConstructionError::WrongProtocol {
                actual: identity.protocol(),
            });
        }

        if identity.operation() != RuntimeOperation::Processing {
            return Err(ProcessingRuntimeInformationConstructionError::WrongOperation {
                actual: identity.operation(),
            });
        }

        if identity.source_contract_version() != ProcessingSourceContractV1::CONTRACT_VERSION {
            return Err(
                ProcessingRuntimeInformationConstructionError::IdentityContractVersionMismatch {
                    identity_version: identity.source_contract_version(),
                    descriptor_version: ProcessingSourceContractV1::CONTRACT_VERSION,
                },
            );
        }

        Ok(Self {
            identity,
            descriptor_contract_version: ProcessingSourceContractV1::CONTRACT_VERSION,
        })
    }

    pub fn validate_compatibility(
        &self,
        expected_identity: RuntimeIdentity,
    ) -> Result<(), ProcessingRuntimeCompatibilityError> {
        if self.identity() != expected_identity {
            return Err(ProcessingRuntimeCompatibilityError::IdentityMismatch {
                expected: expected_identity,
                actual: self.identity(),
            });
        }

        if self.descriptor_contract_version() != self.identity().source_contract_version() {
            return Err(
                ProcessingRuntimeCompatibilityError::DescriptorContractVersionMismatch {
                    identity_version: self.identity().source_contract_version(),
                    descriptor_version: self.descriptor_contract_version(),
                },
            );
        }

        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProcessingRuntimeInformationConstructionError {
    WrongProtocol {
        actual: RuntimeProtocol,
    },
    WrongOperation {
        actual: RuntimeOperation,
    },
    IdentityContractVersionMismatch {
        identity_version: u32,
        descriptor_version: u32,
    },
}

impl fmt::Display for ProcessingRuntimeInformationConstructionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WrongProtocol { actual } => {
                write!(formatter, "processing runtime information requires HTTP protocol, actual: {actual:?}")
            }
            Self::WrongOperation { actual } => {
                write!(formatter, "processing runtime information requires processing operation, actual: {actual:?}")
            }
            Self::IdentityContractVersionMismatch {
                identity_version,
                descriptor_version,
            } => write!(
                formatter,
                "processing runtime information identity/version mismatch: identity version {identity_version}, descriptor version {descriptor_version}"
            ),
        }
    }
}

impl std::error::Error for ProcessingRuntimeInformationConstructionError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProcessingRuntimeCompatibilityError {
    IdentityMismatch {
        expected: RuntimeIdentity,
        actual: RuntimeIdentity,
    },
    DescriptorContractVersionMismatch {
        identity_version: u32,
        descriptor_version: u32,
    },
}

impl fmt::Display for ProcessingRuntimeCompatibilityError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::IdentityMismatch { expected, actual } => write!(
                formatter,
                "processing runtime identity mismatch: expected {expected:?}, actual {actual:?}"
            ),
            Self::DescriptorContractVersionMismatch {
                identity_version,
                descriptor_version,
            } => write!(
                formatter,
                "processing descriptor contract version mismatch: identity version {identity_version}, descriptor version {descriptor_version}"
            ),
        }
    }
}

impl std::error::Error for ProcessingRuntimeCompatibilityError {}

#[cfg(test)]
mod tests {
    use std::ffi::OsString;
    use std::sync::atomic::{AtomicUsize, Ordering};

    use super::{
        ProcessingRuntimeCompatibilityError, ProcessingRuntimeInformationConstructionError,
        ProcessingRuntimeInformationV1,
    };
    use crate::processing::{ProcessingContext, ProcessingResult, ProcessingSourceContractV1};
    use crate::runtime::{RuntimeIdentity, RuntimeOperation, RuntimeProtocol};

    static CONSTRUCTION_CALL_COUNT: AtomicUsize = AtomicUsize::new(0);

    fn process_handler(
        _context: &mut ProcessingContext,
        _args: &[OsString],
    ) -> ProcessingResult<()> {
        CONSTRUCTION_CALL_COUNT.fetch_add(1, Ordering::Relaxed);
        Ok(())
    }

    fn private_handler(
        _context: &mut ProcessingContext,
        _args: &[OsString],
    ) -> ProcessingResult<()> {
        Ok(())
    }

    fn processing_identity(version: u32) -> RuntimeIdentity {
        RuntimeIdentity::http_processing("example-source", version)
    }

    fn acquisition_identity() -> RuntimeIdentity {
        RuntimeIdentity::http_acquisition("example-source", 1)
    }

    #[test]
    fn valid_processing_identity_and_descriptor_construct_successfully() {
        let source = ProcessingSourceContractV1::new(process_handler);
        let result = ProcessingRuntimeInformationV1::from_processing_source(processing_identity(1), &source);

        assert!(result.is_ok(), "result: {result:?}");
    }

    #[test]
    fn information_preserves_source_identity() {
        let source = ProcessingSourceContractV1::new(process_handler);
        let identity = processing_identity(1);
        let info = ProcessingRuntimeInformationV1::from_processing_source(identity, &source).unwrap();

        assert_eq!(info.identity(), identity);
    }

    #[test]
    fn information_reports_http_protocol() {
        let source = ProcessingSourceContractV1::new(process_handler);
        let info = ProcessingRuntimeInformationV1::from_processing_source(processing_identity(1), &source).unwrap();

        assert_eq!(info.identity().protocol(), RuntimeProtocol::Http);
    }

    #[test]
    fn information_reports_processing_operation() {
        let source = ProcessingSourceContractV1::new(process_handler);
        let info = ProcessingRuntimeInformationV1::from_processing_source(processing_identity(1), &source).unwrap();

        assert_eq!(info.identity().operation(), RuntimeOperation::Processing);
    }

    #[test]
    fn descriptor_contract_version_is_one() {
        let source = ProcessingSourceContractV1::new(process_handler);
        let info = ProcessingRuntimeInformationV1::from_processing_source(processing_identity(1), &source).unwrap();

        assert_eq!(info.descriptor_contract_version(), ProcessingSourceContractV1::CONTRACT_VERSION);
        assert_eq!(info.descriptor_contract_version(), 1);
    }

    #[test]
    fn processing_runtime_information_is_copy() {
        let source = ProcessingSourceContractV1::new(process_handler);
        let info = ProcessingRuntimeInformationV1::from_processing_source(processing_identity(1), &source).unwrap();
        let duplicate = info;
        let _ = (info, duplicate);
    }

    #[test]
    fn construction_does_not_invoke_process_handler() {
        CONSTRUCTION_CALL_COUNT.store(0, Ordering::Relaxed);
        let source = ProcessingSourceContractV1::new(process_handler);
        let _ = ProcessingRuntimeInformationV1::from_processing_source(processing_identity(1), &source);

        assert_eq!(CONSTRUCTION_CALL_COUNT.load(Ordering::Relaxed), 0);
    }

    #[test]
    fn acquisition_identity_returns_wrong_operation() {
        let source = ProcessingSourceContractV1::new(process_handler);
        let result = ProcessingRuntimeInformationV1::from_processing_source(acquisition_identity(), &source);

        assert_eq!(
            result,
            Err(ProcessingRuntimeInformationConstructionError::WrongOperation {
                actual: RuntimeOperation::Acquisition,
            })
        );
    }

    #[test]
    fn non_matching_identity_contract_version_returns_identity_contract_version_mismatch() {
        let source = ProcessingSourceContractV1::new(process_handler);
        let result = ProcessingRuntimeInformationV1::from_processing_source(processing_identity(2), &source);

        assert_eq!(
            result,
            Err(
                ProcessingRuntimeInformationConstructionError::IdentityContractVersionMismatch {
                    identity_version: 2,
                    descriptor_version: ProcessingSourceContractV1::CONTRACT_VERSION,
                }
            )
        );
    }

    #[test]
    fn validation_against_same_processing_identity_succeeds() {
        let source = ProcessingSourceContractV1::new(process_handler);
        let info = ProcessingRuntimeInformationV1::from_processing_source(processing_identity(1), &source).unwrap();

        assert!(info.validate_compatibility(processing_identity(1)).is_ok());
    }

    #[test]
    fn validation_against_another_source_returns_identity_mismatch() {
        let source = ProcessingSourceContractV1::new(process_handler);
        let info = ProcessingRuntimeInformationV1::from_processing_source(processing_identity(1), &source).unwrap();

        assert!(matches!(
            info.validate_compatibility(RuntimeIdentity::http_processing("other-source", 1)),
            Err(ProcessingRuntimeCompatibilityError::IdentityMismatch { .. })
        ));
    }

    #[test]
    fn validation_against_acquisition_identity_returns_identity_mismatch() {
        let source = ProcessingSourceContractV1::new(process_handler);
        let info = ProcessingRuntimeInformationV1::from_processing_source(processing_identity(1), &source).unwrap();

        assert!(matches!(
            info.validate_compatibility(acquisition_identity()),
            Err(ProcessingRuntimeCompatibilityError::IdentityMismatch { .. })
        ));
    }

    #[test]
    fn descriptor_version_disagreement_returns_descriptor_contract_version_mismatch() {
        let source = ProcessingSourceContractV1::new(process_handler);
        let info = ProcessingRuntimeInformationV1::from_processing_source(processing_identity(1), &source).unwrap();
        let mutated = ProcessingRuntimeInformationV1 {
            identity: info.identity(),
            descriptor_contract_version: 2,
        };

        assert!(matches!(
            mutated.validate_compatibility(info.identity()),
            Err(ProcessingRuntimeCompatibilityError::DescriptorContractVersionMismatch { .. })
        ));
    }

    #[test]
    fn construction_and_validation_do_not_mutate_descriptor() {
        let source = ProcessingSourceContractV1::new(process_handler);
        let original_ptr = source.process_handler() as *const ();
        let info = ProcessingRuntimeInformationV1::from_processing_source(processing_identity(1), &source).unwrap();
        let validated = info.validate_compatibility(processing_identity(1));

        assert!(validated.is_ok());
        assert_eq!(source.process_handler() as *const (), original_ptr);
    }

    #[test]
    fn construction_and_validation_do_not_invoke_process_handler() {
        CONSTRUCTION_CALL_COUNT.store(0, Ordering::Relaxed);
        let source = ProcessingSourceContractV1::new(process_handler);
        let info = ProcessingRuntimeInformationV1::from_processing_source(processing_identity(1), &source).unwrap();
        let _ = info.validate_compatibility(processing_identity(1));

        assert_eq!(CONSTRUCTION_CALL_COUNT.load(Ordering::Relaxed), 0);
    }

    #[test]
    fn private_handler_works_behind_public_descriptor_constant() {
        const PRIVATE_SOURCE: ProcessingSourceContractV1 =
            ProcessingSourceContractV1::new(private_handler);

        let mut context = ProcessingContext::new_for_tests();
        let args = [OsString::from("alpha")];
        let result = PRIVATE_SOURCE.process_handler()(&mut context, &args);

        assert!(result.is_ok(), "result: {result:?}");
    }

    #[test]
    fn native_source_arguments_are_not_involved_in_information_construction() {
        let source = ProcessingSourceContractV1::new(process_handler);
        let _ = source.process_handler();
        let info = ProcessingRuntimeInformationV1::from_processing_source(processing_identity(1), &source).unwrap();

        assert_eq!(info.identity().source_contract_version(), 1);
    }
}
