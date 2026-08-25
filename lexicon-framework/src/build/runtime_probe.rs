use std::fmt;

use lexicon_core::runtime::{
    RuntimeCompatibilityError, RuntimeIdentity, RuntimeInformationDecodingError, RuntimeInformationV1,
};

pub const MAX_RUNTIME_INFORMATION_PROBE_BYTES: usize = 64 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdmittedRuntimeInformation {
    information: RuntimeInformationV1,
}

impl AdmittedRuntimeInformation {
    pub fn information(&self) -> &RuntimeInformationV1 {
        &self.information
    }
}

#[derive(Debug)]
pub enum RuntimeProbeAdmissionError {
    OutputTooLarge {
        maximum: usize,
        actual: usize,
    },
    EmptyOutput,
    ContainsNul,
    InvalidUtf8(std::str::Utf8Error),
    InvalidOutputBoundary,
    Decode(RuntimeInformationDecodingError),
    Incompatible(RuntimeCompatibilityError),
}

impl fmt::Display for RuntimeProbeAdmissionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::OutputTooLarge { maximum, actual } => write!(
                formatter,
                "runtime information probe output exceeds {} bytes (actual: {actual})",
                maximum
            ),
            Self::EmptyOutput => formatter.write_str("runtime information probe output is empty"),
            Self::ContainsNul => formatter.write_str("runtime information probe output contains a NUL byte"),
            Self::InvalidUtf8(error) => write!(formatter, "runtime information probe output is not valid UTF-8: {error}"),
            Self::InvalidOutputBoundary => formatter.write_str("runtime information probe output does not match the required exact boundary"),
            Self::Decode(error) => write!(formatter, "runtime information probe decode failed: {error}"),
            Self::Incompatible(error) => write!(formatter, "runtime information probe compatibility validation failed: {error}"),
        }
    }
}

impl std::error::Error for RuntimeProbeAdmissionError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::InvalidUtf8(error) => Some(error),
            Self::Decode(error) => Some(error),
            Self::Incompatible(error) => Some(error),
            Self::OutputTooLarge { .. }
            | Self::EmptyOutput
            | Self::ContainsNul
            | Self::InvalidOutputBoundary => None,
        }
    }
}

pub fn admit_http_runtime_information_probe(
    expected_identity: RuntimeIdentity,
    stdout: &[u8],
) -> Result<AdmittedRuntimeInformation, RuntimeProbeAdmissionError> {
    if stdout.len() > MAX_RUNTIME_INFORMATION_PROBE_BYTES {
        return Err(RuntimeProbeAdmissionError::OutputTooLarge {
            maximum: MAX_RUNTIME_INFORMATION_PROBE_BYTES,
            actual: stdout.len(),
        });
    }

    if stdout.is_empty() {
        return Err(RuntimeProbeAdmissionError::EmptyOutput);
    }

    if stdout.iter().any(|byte| *byte == 0) {
        return Err(RuntimeProbeAdmissionError::ContainsNul);
    }

    let text = std::str::from_utf8(stdout).map_err(RuntimeProbeAdmissionError::InvalidUtf8)?;

    if !text.ends_with('\n') {
        return Err(RuntimeProbeAdmissionError::InvalidOutputBoundary);
    }

    if text.bytes().filter(|byte| *byte == b'\n').count() != 1 {
        return Err(RuntimeProbeAdmissionError::InvalidOutputBoundary);
    }

    if text.starts_with('\n') || text.starts_with('\r') || text.contains('\r') {
        return Err(RuntimeProbeAdmissionError::InvalidOutputBoundary);
    }

    let json_text = &text[..text.len() - 1];
    if json_text
        .chars()
        .next()
        .is_some_and(|character| character.is_whitespace())
        || json_text
            .chars()
            .next_back()
            .is_some_and(|character| character.is_whitespace())
    {
        return Err(RuntimeProbeAdmissionError::InvalidOutputBoundary);
    }

    let information = RuntimeInformationV1::from_json(json_text)
        .map_err(RuntimeProbeAdmissionError::Decode)?;
    information
        .validate_compatibility(expected_identity)
        .map_err(RuntimeProbeAdmissionError::Incompatible)?;

    Ok(AdmittedRuntimeInformation { information })
}

#[cfg(test)]
mod tests {
    use std::ffi::OsString;

    use lexicon_core::protocols::http::{HttpCapability, HttpCapabilitySet, HttpSourceContractV1};
    use lexicon_core::runtime::{RuntimeCompatibilityError, RuntimeIdentity, RuntimeInformationV1};
    use lexicon_core::{
        HttpAcquisitionContext,
        protocols::http::runner::{
            RUNTIME_INFORMATION_PROBE_ARGUMENT, try_write_runtime_information_probe,
        },
    };

    use super::{
        MAX_RUNTIME_INFORMATION_PROBE_BYTES, AdmittedRuntimeInformation,
        RuntimeProbeAdmissionError, admit_http_runtime_information_probe,
    };

    fn acquire_handler(
        _context: &mut HttpAcquisitionContext,
        _args: &[std::ffi::OsString],
    ) -> lexicon_core::protocols::http::AcquisitionResult<()> {
        Ok(())
    }

    fn resume_handler(
        _context: &mut HttpAcquisitionContext,
        _args: &[std::ffi::OsString],
    ) -> lexicon_core::protocols::http::AcquisitionResult<()> {
        Ok(())
    }

    fn failing_acquire(
        _context: &mut HttpAcquisitionContext,
        _args: &[std::ffi::OsString],
    ) -> lexicon_core::protocols::http::AcquisitionResult<()> {
        panic!("acquire should not be invoked while admitting runtime information")
    }

    fn failing_resume(
        _context: &mut HttpAcquisitionContext,
        _args: &[std::ffi::OsString],
    ) -> lexicon_core::protocols::http::AcquisitionResult<()> {
        panic!("resume should not be invoked while admitting runtime information")
    }

    fn valid_probe_output(
        identity: RuntimeIdentity,
        source: &HttpSourceContractV1,
        available: HttpCapabilitySet,
    ) -> Vec<u8> {
        let mut output = Vec::new();
        try_write_runtime_information_probe(
            identity,
            source,
            available,
            &[OsString::from(RUNTIME_INFORMATION_PROBE_ARGUMENT)],
            &mut output,
        )
        .unwrap();
        output
    }

    #[test]
    fn exact_output_from_core_probe_is_admitted() {
        let identity = RuntimeIdentity::http_acquisition("example-source", 1);
        let source = HttpSourceContractV1::new(acquire_handler);
        let available = HttpCapabilitySet::empty();
        let output = valid_probe_output(identity, &source, available);

        let admitted = admit_http_runtime_information_probe(identity, &output).unwrap();
        let json = std::str::from_utf8(&output).unwrap();
        let expected = RuntimeInformationV1::from_json(json.trim_end_matches('\n')).unwrap();

        assert_eq!(admitted.information(), &expected);
    }

    #[test]
    fn admitted_wrapper_exposes_decoded_information() {
        let identity = RuntimeIdentity::http_acquisition("example-source", 1);
        let source = HttpSourceContractV1::new(acquire_handler)
            .with_resume(resume_handler)
            .requires(HttpCapability::ClientCertificateV1);
        let available = HttpCapabilitySet::empty().insert(HttpCapability::ClientCertificateV1);
        let output = valid_probe_output(identity, &source, available);

        let admitted = admit_http_runtime_information_probe(identity, &output).unwrap();
        assert_eq!(admitted.information().identity(), identity);
        assert_eq!(admitted.information().required_capabilities(), source.required_capabilities());
        assert_eq!(admitted.information().available_capabilities(), available);
        assert!(admitted.information().resume_handler_registered());
    }

    #[test]
    fn matching_identity_and_capabilities_succeed() {
        let identity = RuntimeIdentity::http_acquisition("example-source", 1);
        let source = HttpSourceContractV1::new(acquire_handler)
            .with_resume(resume_handler)
            .requires(HttpCapability::ClientCertificateV1);
        let available = HttpCapabilitySet::empty().insert(HttpCapability::ClientCertificateV1);
        let output = valid_probe_output(identity, &source, available);

        let result = admit_http_runtime_information_probe(identity, &output);
        assert!(result.is_ok(), "{:?}", result);
    }

    #[test]
    fn empty_output_is_rejected() {
        let result = admit_http_runtime_information_probe(
            RuntimeIdentity::http_acquisition("example-source", 1),
            &[],
        );
        assert!(matches!(result, Err(RuntimeProbeAdmissionError::EmptyOutput)));
    }

    #[test]
    fn oversized_output_is_rejected_before_decoding() {
        let mut oversized = vec![b'{'];
        while oversized.len() <= MAX_RUNTIME_INFORMATION_PROBE_BYTES {
            oversized.push(b'x');
        }
        oversized.push(b'\n');

        let result = admit_http_runtime_information_probe(
            RuntimeIdentity::http_acquisition("example-source", 1),
            &oversized,
        );
        assert!(matches!(
            result,
            Err(RuntimeProbeAdmissionError::OutputTooLarge { maximum, actual })
                if maximum == MAX_RUNTIME_INFORMATION_PROBE_BYTES && actual > maximum
        ));
    }

    #[test]
    fn nul_containing_output_is_rejected() {
        let identity = RuntimeIdentity::http_acquisition("example-source", 1);
        let source = HttpSourceContractV1::new(acquire_handler);
        let mut output = valid_probe_output(identity, &source, HttpCapabilitySet::empty());
        output.insert(output.len() / 2, 0);

        let result = admit_http_runtime_information_probe(identity, &output);
        assert!(matches!(result, Err(RuntimeProbeAdmissionError::ContainsNul)));
    }

    #[test]
    fn invalid_utf8_is_rejected() {
        let identity = RuntimeIdentity::http_acquisition("example-source", 1);
        let source = HttpSourceContractV1::new(acquire_handler);
        let mut output = valid_probe_output(identity, &source, HttpCapabilitySet::empty());
        output[0] = 0xff;

        let result = admit_http_runtime_information_probe(identity, &output);
        assert!(matches!(result, Err(RuntimeProbeAdmissionError::InvalidUtf8(_))));
    }

    #[test]
    fn missing_final_newline_is_rejected() {
        let identity = RuntimeIdentity::http_acquisition("example-source", 1);
        let source = HttpSourceContractV1::new(acquire_handler);
        let mut output = valid_probe_output(identity, &source, HttpCapabilitySet::empty());
        output.pop();

        let result = admit_http_runtime_information_probe(identity, &output);
        assert!(matches!(result, Err(RuntimeProbeAdmissionError::InvalidOutputBoundary)));
    }

    #[test]
    fn two_final_newlines_are_rejected() {
        let identity = RuntimeIdentity::http_acquisition("example-source", 1);
        let source = HttpSourceContractV1::new(acquire_handler);
        let mut output = valid_probe_output(identity, &source, HttpCapabilitySet::empty());
        output.push(b'\n');

        let result = admit_http_runtime_information_probe(identity, &output);
        assert!(matches!(result, Err(RuntimeProbeAdmissionError::InvalidOutputBoundary)));
    }

    #[test]
    fn carriage_return_line_ending_is_rejected() {
        let identity = RuntimeIdentity::http_acquisition("example-source", 1);
        let source = HttpSourceContractV1::new(acquire_handler);
        let mut output = valid_probe_output(identity, &source, HttpCapabilitySet::empty());
        output.insert(output.len() - 1, b'\r');

        let result = admit_http_runtime_information_probe(identity, &output);
        assert!(matches!(result, Err(RuntimeProbeAdmissionError::InvalidOutputBoundary)));
    }

    #[test]
    fn leading_spaces_are_rejected() {
        let identity = RuntimeIdentity::http_acquisition("example-source", 1);
        let source = HttpSourceContractV1::new(acquire_handler);
        let mut output = valid_probe_output(identity, &source, HttpCapabilitySet::empty());
        output.insert(0, b' ');

        let result = admit_http_runtime_information_probe(identity, &output);
        assert!(matches!(result, Err(RuntimeProbeAdmissionError::InvalidOutputBoundary)));
    }

    #[test]
    fn leading_newline_is_rejected() {
        let identity = RuntimeIdentity::http_acquisition("example-source", 1);
        let source = HttpSourceContractV1::new(acquire_handler);
        let mut output = valid_probe_output(identity, &source, HttpCapabilitySet::empty());
        output.insert(0, b'\n');

        let result = admit_http_runtime_information_probe(identity, &output);
        assert!(matches!(result, Err(RuntimeProbeAdmissionError::InvalidOutputBoundary)));
    }

    #[test]
    fn trailing_spaces_before_final_newline_are_rejected() {
        let identity = RuntimeIdentity::http_acquisition("example-source", 1);
        let source = HttpSourceContractV1::new(acquire_handler);
        let mut output = valid_probe_output(identity, &source, HttpCapabilitySet::empty());
        output.insert(output.len() - 1, b' ');

        let result = admit_http_runtime_information_probe(identity, &output);
        assert!(matches!(result, Err(RuntimeProbeAdmissionError::InvalidOutputBoundary)));
    }

    #[test]
    fn diagnostic_text_before_json_is_rejected() {
        let identity = RuntimeIdentity::http_acquisition("example-source", 1);
        let source = HttpSourceContractV1::new(acquire_handler);
        let mut output = valid_probe_output(identity, &source, HttpCapabilitySet::empty());
        output.splice(..0, b"noise ".iter().copied());

        let result = admit_http_runtime_information_probe(identity, &output);
        assert!(matches!(result, Err(RuntimeProbeAdmissionError::Decode(_))));
    }

    #[test]
    fn diagnostic_text_after_json_is_rejected() {
        let identity = RuntimeIdentity::http_acquisition("example-source", 1);
        let source = HttpSourceContractV1::new(acquire_handler);
        let mut output = valid_probe_output(identity, &source, HttpCapabilitySet::empty());
        output.insert(output.len() - 1, b'x');

        let result = admit_http_runtime_information_probe(identity, &output);
        assert!(matches!(
            result,
            Err(RuntimeProbeAdmissionError::InvalidOutputBoundary)
                | Err(RuntimeProbeAdmissionError::Decode(_))
        ));
    }

    #[test]
    fn multiple_json_documents_are_rejected() {
        let identity = RuntimeIdentity::http_acquisition("example-source", 1);
        let source = HttpSourceContractV1::new(acquire_handler);
        let mut output = valid_probe_output(identity, &source, HttpCapabilitySet::empty());
        output.extend_from_slice(&valid_probe_output(identity, &source, HttpCapabilitySet::empty()));

        let result = admit_http_runtime_information_probe(identity, &output);
        assert!(matches!(
            result,
            Err(RuntimeProbeAdmissionError::InvalidOutputBoundary)
                | Err(RuntimeProbeAdmissionError::Decode(_))
        ));
    }

    #[test]
    fn structurally_invalid_json_produces_decode_error() {
        let result = admit_http_runtime_information_probe(
            RuntimeIdentity::http_acquisition("example-source", 1),
            b"{not json}\n",
        );
        assert!(matches!(result, Err(RuntimeProbeAdmissionError::Decode(_))));
    }

    #[test]
    fn unknown_schema_version_produces_decode_error() {
        let identity = RuntimeIdentity::http_acquisition("example-source", 1);
        let source = HttpSourceContractV1::new(acquire_handler);
        let output = valid_probe_output(identity, &source, HttpCapabilitySet::empty());

        let json = String::from_utf8(output.clone()).unwrap();
        let mut document: serde_json::Value = serde_json::from_str(json.trim_end()).unwrap();
        document["schema_version"] = serde_json::Value::from(2);
        let mut candidate = serde_json::to_vec(&document).unwrap();
        candidate.push(b'\n');

        let result = admit_http_runtime_information_probe(identity, &candidate);
        assert!(matches!(result, Err(RuntimeProbeAdmissionError::Decode(_))));
    }

    #[test]
    fn identity_disagreement_produces_incompatible_error() {
        let actual_identity = RuntimeIdentity::http_acquisition("example-source", 1);
        let source = HttpSourceContractV1::new(acquire_handler);
        let output = valid_probe_output(actual_identity, &source, HttpCapabilitySet::empty());

        let result = admit_http_runtime_information_probe(
            RuntimeIdentity::http_acquisition("other-source", 1),
            &output,
        );
        assert!(matches!(
            result,
            Err(RuntimeProbeAdmissionError::Incompatible(
                RuntimeCompatibilityError::IdentityMismatch { expected, actual }
            )) if expected == RuntimeIdentity::http_acquisition("other-source", 1)
                && actual == actual_identity
        ));
    }

    #[test]
    fn descriptor_version_disagreement_produces_incompatible_error() {
        let identity = RuntimeIdentity::http_acquisition("example-source", 1);
        let source = HttpSourceContractV1::new(acquire_handler);
        let output = valid_probe_output(identity, &source, HttpCapabilitySet::empty());

        let json = String::from_utf8(output.clone()).unwrap();
        let mut document: serde_json::Value = serde_json::from_str(json.trim_end()).unwrap();
        document["descriptor"]["contract_version"] = serde_json::Value::from(2);
        let mut candidate = serde_json::to_vec(&document).unwrap();
        candidate.push(b'\n');

        let result = admit_http_runtime_information_probe(identity, &candidate);
        assert!(matches!(
            result,
            Err(RuntimeProbeAdmissionError::Incompatible(
                RuntimeCompatibilityError::DescriptorContractVersionMismatch {
                    identity_version,
                    descriptor_version,
                }
            )) if identity_version == 1 && descriptor_version == 2
        ));
    }

    #[test]
    fn missing_required_capabilities_produce_incompatible_error() {
        let identity = RuntimeIdentity::http_acquisition("example-source", 1);
        let source = HttpSourceContractV1::new(acquire_handler)
            .requires(HttpCapability::ClientCertificateV1);
        let available = HttpCapabilitySet::empty();
        let output = valid_probe_output(identity, &source, available);

        let result = admit_http_runtime_information_probe(identity, &output);
        assert!(matches!(
            result,
            Err(RuntimeProbeAdmissionError::Incompatible(
                RuntimeCompatibilityError::MissingCapabilities(missing)
            )) if missing.missing() == HttpCapabilitySet::empty().insert(HttpCapability::ClientCertificateV1)
        ));
    }

    #[test]
    fn missing_capability_set_remains_inspectable() {
        let identity = RuntimeIdentity::http_acquisition("example-source", 1);
        let source = HttpSourceContractV1::new(acquire_handler)
            .requires(HttpCapability::ClientCertificateV1)
            .requires(HttpCapability::ClientCertificateV1);
        let available = HttpCapabilitySet::empty();
        let output = valid_probe_output(identity, &source, available);

        let error = admit_http_runtime_information_probe(identity, &output).unwrap_err();
        match error {
            RuntimeProbeAdmissionError::Incompatible(RuntimeCompatibilityError::MissingCapabilities(missing)) => {
                assert_eq!(missing.missing().ordered_capabilities().len(), 1);
                assert_eq!(missing.missing().ordered_capabilities()[0], HttpCapability::ClientCertificateV1);
            }
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[test]
    fn admission_does_not_invoke_acquire_or_resume_handlers() {
        let identity = RuntimeIdentity::http_acquisition("example-source", 1);
        let source = HttpSourceContractV1::new(failing_acquire).with_resume(failing_resume);
        let available = HttpCapabilitySet::empty();
        let output = valid_probe_output(identity, &source, available);

        let admitted = admit_http_runtime_information_probe(identity, &output).unwrap();
        assert_eq!(admitted.information().identity(), identity);
    }

    #[test]
    fn type_is_not_publicly_constructible() {
        let _ = AdmittedRuntimeInformation {
            information: RuntimeInformationV1::from_http_source(
                RuntimeIdentity::http_acquisition("example-source", 1),
                &HttpSourceContractV1::new(acquire_handler),
                HttpCapabilitySet::empty(),
            ),
        };
    }
}
