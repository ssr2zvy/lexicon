use std::ffi::{OsStr, OsString};
use std::fmt;

use super::RuntimeInvocationEnvelopeV1;

pub const RUNTIME_INVOCATION_ARGUMENT: &str = "--lexicon-invocation-v1";
pub const RUNTIME_SOURCE_ARGUMENT_DELIMITER: &str = "--";
pub const MAX_RUNTIME_INVOCATION_ENVELOPE_JSON_BYTES: usize = 16 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuntimeInvocationTransportEncodingError {
    Serialization(String),
    EnvelopeTooLarge { actual: usize, maximum: usize },
}

impl fmt::Display for RuntimeInvocationTransportEncodingError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Serialization(message) => write!(formatter, "runtime invocation transport serialization error: {message}"),
            Self::EnvelopeTooLarge { actual, maximum } => {
                write!(
                    formatter,
                    "runtime invocation envelope exceeds the maximum size of {maximum} bytes ({actual} bytes)"
                )
            }
        }
    }
}

impl std::error::Error for RuntimeInvocationTransportEncodingError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuntimeInvocationTransportDecodingError {
    EmptyArguments,
    InvalidFirstArgument(OsString),
    ProbeMode,
    MissingEnvelopeArgument,
    InvalidEnvelopeUtf8,
    EnvelopeTooLarge { actual: usize, maximum: usize },
    MissingDelimiter,
    UnexpectedValueBeforeDelimiter(OsString),
    InvalidEnvelopeJson(String),
}

impl fmt::Display for RuntimeInvocationTransportDecodingError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyArguments => formatter.write_str("runtime invocation arguments are empty"),
            Self::InvalidFirstArgument(argument) => {
                write!(
                    formatter,
                    "invalid runtime invocation argument: {}",
                    argument.to_string_lossy()
                )
            }
            Self::ProbeMode => formatter.write_str("runtime information probe mode is not a runtime invocation"),
            Self::MissingEnvelopeArgument => formatter.write_str("missing runtime invocation envelope argument"),
            Self::InvalidEnvelopeUtf8 => formatter.write_str("runtime invocation envelope is not valid UTF-8"),
            Self::EnvelopeTooLarge { actual, maximum } => {
                write!(
                    formatter,
                    "runtime invocation envelope exceeds the maximum size of {maximum} bytes ({actual} bytes)"
                )
            }
            Self::MissingDelimiter => formatter.write_str("missing runtime source argument delimiter"),
            Self::UnexpectedValueBeforeDelimiter(value) => {
                write!(
                    formatter,
                    "unexpected runtime invocation value before the delimiter: {}",
                    value.to_string_lossy()
                )
            }
            Self::InvalidEnvelopeJson(message) => {
                write!(formatter, "invalid runtime invocation envelope JSON: {message}")
            }
        }
    }
}

impl std::error::Error for RuntimeInvocationTransportDecodingError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EncodedRuntimeInvocation {
    arguments: Vec<OsString>,
}

impl EncodedRuntimeInvocation {
    pub fn arguments(&self) -> &[OsString] {
        &self.arguments
    }

    pub fn into_arguments(self) -> Vec<OsString> {
        self.arguments
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedRuntimeInvocation {
    envelope: RuntimeInvocationEnvelopeV1,
    source_arguments: Vec<OsString>,
}

impl ParsedRuntimeInvocation {
    pub fn envelope(&self) -> &RuntimeInvocationEnvelopeV1 {
        &self.envelope
    }

    pub fn source_arguments(&self) -> &[OsString] {
        &self.source_arguments
    }

    pub fn into_parts(self) -> (RuntimeInvocationEnvelopeV1, Vec<OsString>) {
        (self.envelope, self.source_arguments)
    }
}

pub fn encode_runtime_invocation(
    envelope: &RuntimeInvocationEnvelopeV1,
    source_arguments: &[OsString],
) -> Result<EncodedRuntimeInvocation, RuntimeInvocationTransportEncodingError> {
    let envelope_json = envelope
        .to_json()
        .map_err(|error| RuntimeInvocationTransportEncodingError::Serialization(error.to_string()))?;
    let actual = envelope_json.len();
    if actual > MAX_RUNTIME_INVOCATION_ENVELOPE_JSON_BYTES {
        return Err(RuntimeInvocationTransportEncodingError::EnvelopeTooLarge {
            actual,
            maximum: MAX_RUNTIME_INVOCATION_ENVELOPE_JSON_BYTES,
        });
    }

    let mut arguments = Vec::with_capacity(3 + source_arguments.len());
    arguments.push(OsString::from(RUNTIME_INVOCATION_ARGUMENT));
    arguments.push(OsString::from(envelope_json));
    arguments.push(OsString::from(RUNTIME_SOURCE_ARGUMENT_DELIMITER));
    arguments.extend(source_arguments.iter().cloned());

    Ok(EncodedRuntimeInvocation { arguments })
}

pub fn parse_runtime_invocation(
    arguments: &[OsString],
) -> Result<ParsedRuntimeInvocation, RuntimeInvocationTransportDecodingError> {
    let first_argument = arguments.first().ok_or(RuntimeInvocationTransportDecodingError::EmptyArguments)?;

    if first_argument.as_os_str() == OsStr::new(crate::runtime::RUNTIME_INFORMATION_PROBE_ARGUMENT) {
        return Err(RuntimeInvocationTransportDecodingError::ProbeMode);
    }

    if first_argument.as_os_str() != OsStr::new(RUNTIME_INVOCATION_ARGUMENT) {
        return Err(RuntimeInvocationTransportDecodingError::InvalidFirstArgument(
            first_argument.clone(),
        ));
    }

    let envelope_argument = arguments
        .get(1)
        .ok_or(RuntimeInvocationTransportDecodingError::MissingEnvelopeArgument)?;
    let envelope_json = envelope_argument
        .to_str()
        .ok_or(RuntimeInvocationTransportDecodingError::InvalidEnvelopeUtf8)?;
    let actual = envelope_json.len();
    if actual > MAX_RUNTIME_INVOCATION_ENVELOPE_JSON_BYTES {
        return Err(RuntimeInvocationTransportDecodingError::EnvelopeTooLarge {
            actual,
            maximum: MAX_RUNTIME_INVOCATION_ENVELOPE_JSON_BYTES,
        });
    }

    let delimiter_argument = arguments.get(2).ok_or(RuntimeInvocationTransportDecodingError::MissingDelimiter)?;
    if delimiter_argument.as_os_str() != OsStr::new(RUNTIME_SOURCE_ARGUMENT_DELIMITER) {
        return Err(RuntimeInvocationTransportDecodingError::MissingDelimiter);
    }

    let envelope = RuntimeInvocationEnvelopeV1::from_json(envelope_json)
        .map_err(|error| RuntimeInvocationTransportDecodingError::InvalidEnvelopeJson(error.to_string()))?;

    let source_arguments = if arguments.len() > 3 {
        arguments[3..].to_vec()
    } else {
        Vec::new()
    };

    Ok(ParsedRuntimeInvocation {
        envelope,
        source_arguments,
    })
}

#[cfg(test)]
mod tests {
    use std::ffi::OsString;

    use super::{
        MAX_RUNTIME_INVOCATION_ENVELOPE_JSON_BYTES, RUNTIME_INVOCATION_ARGUMENT,
        RUNTIME_SOURCE_ARGUMENT_DELIMITER,
    };
    use crate::runtime::{
        ProjectInvocationIdentity, RuntimeExecutionMode, RuntimeIdentity, RuntimeInvocationEnvelopeV1,
        RuntimeSupervisionMode, SessionInvocationIdentity,
    };

    fn example_envelope() -> RuntimeInvocationEnvelopeV1 {
        RuntimeInvocationEnvelopeV1::new(
            ProjectInvocationIdentity::new("example-project").unwrap(),
            RuntimeIdentity::http_acquisition("example-source", 1),
            SessionInvocationIdentity::new("session-123").unwrap(),
            RuntimeExecutionMode::Run,
            RuntimeSupervisionMode::Foreground,
        )
        .unwrap()
    }

    #[test]
    fn encode_runtime_invocation_round_trips_empty_source_arguments() {
        let envelope = example_envelope();
        let encoded = super::encode_runtime_invocation(&envelope, &[]).unwrap();
        assert_eq!(
            encoded.arguments(),
            &[
                OsString::from(RUNTIME_INVOCATION_ARGUMENT),
                OsString::from(envelope.to_json().unwrap()),
                OsString::from(RUNTIME_SOURCE_ARGUMENT_DELIMITER),
            ]
        );

        let parsed = super::parse_runtime_invocation(encoded.arguments()).unwrap();
        assert_eq!(parsed.envelope(), &envelope);
        assert!(parsed.source_arguments().is_empty());
    }

    #[test]
    fn encode_runtime_invocation_preserves_arguments_after_delimiter() {
        let envelope = example_envelope();
        let source_arguments = [
            OsString::from(""),
            OsString::from("--"),
            OsString::from("--lexicon-invocation-v1"),
            OsString::from("--lexicon-runtime-information-v1"),
            OsString::from("hello world"),
            OsString::from("-x"),
            OsString::from("value"),
            OsString::from("value"),
        ];

        let encoded = super::encode_runtime_invocation(&envelope, &source_arguments).unwrap();
        let parsed = super::parse_runtime_invocation(encoded.arguments()).unwrap();

        assert_eq!(parsed.envelope(), &envelope);
        assert_eq!(parsed.source_arguments(), &source_arguments[..]);
    }

    #[test]
    fn parse_runtime_invocation_rejects_probe_mode() {
        let args = [OsString::from(crate::runtime::RUNTIME_INFORMATION_PROBE_ARGUMENT)];
        let error = super::parse_runtime_invocation(&args).unwrap_err();
        assert!(matches!(error, super::RuntimeInvocationTransportDecodingError::ProbeMode));
    }

    #[test]
    fn parse_runtime_invocation_rejects_invalid_first_argument() {
        let args = [OsString::from("--missing"), OsString::from("{}"), OsString::from("--")];
        let error = super::parse_runtime_invocation(&args).unwrap_err();
        assert!(matches!(error, super::RuntimeInvocationTransportDecodingError::InvalidFirstArgument(_)));
    }

    #[test]
    fn parse_runtime_invocation_rejects_missing_delimiter() {
        let envelope = example_envelope();
        let args = [
            OsString::from(RUNTIME_INVOCATION_ARGUMENT),
            OsString::from(envelope.to_json().unwrap()),
        ];
        assert!(matches!(
            super::parse_runtime_invocation(&args),
            Err(super::RuntimeInvocationTransportDecodingError::MissingDelimiter)
        ));
    }

    #[test]
    fn parse_runtime_invocation_rejects_delimiter_in_wrong_position() {
        let envelope = example_envelope();
        let args = [
            OsString::from(RUNTIME_INVOCATION_ARGUMENT),
            OsString::from(envelope.to_json().unwrap()),
            OsString::from("value"),
            OsString::from(RUNTIME_SOURCE_ARGUMENT_DELIMITER),
        ];
        assert!(matches!(
            super::parse_runtime_invocation(&args),
            Err(super::RuntimeInvocationTransportDecodingError::MissingDelimiter)
        ));
    }

    #[test]
    fn parse_runtime_invocation_rejects_oversized_envelope() {
        let oversized = "a".repeat(MAX_RUNTIME_INVOCATION_ENVELOPE_JSON_BYTES + 1);
        let args = [
            OsString::from(RUNTIME_INVOCATION_ARGUMENT),
            OsString::from(oversized),
            OsString::from(RUNTIME_SOURCE_ARGUMENT_DELIMITER),
        ];
        assert!(matches!(
            super::parse_runtime_invocation(&args),
            Err(super::RuntimeInvocationTransportDecodingError::EnvelopeTooLarge { .. })
        ));
    }

    #[test]
    fn parse_runtime_invocation_rejects_invalid_json() {
        let args = [
            OsString::from(RUNTIME_INVOCATION_ARGUMENT),
            OsString::from("{not-valid-json}"),
            OsString::from(RUNTIME_SOURCE_ARGUMENT_DELIMITER),
        ];
        assert!(matches!(
            super::parse_runtime_invocation(&args),
            Err(super::RuntimeInvocationTransportDecodingError::InvalidEnvelopeJson(_))
        ));
    }
}
