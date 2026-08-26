use std::collections::BTreeSet;
use std::fmt;

use lexicon_core::processing::{
    ProcessingRuntimeInformationDecodingError, ProcessingRuntimeInformationEncodingError,
    ProcessingRuntimeInformationV1,
};

use super::is_safe_executable_name;
use super::runtime_manifest::ExecutableSha256;
use super::runtime_verification::VerifiedProcessingRuntime;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProcessingRuntimeManifestConstructionError {
    InvalidExecutableName,
}

impl fmt::Display for ProcessingRuntimeManifestConstructionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidExecutableName => {
                formatter.write_str("invalid processing runtime manifest executable name")
            }
        }
    }
}

impl std::error::Error for ProcessingRuntimeManifestConstructionError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProcessingRuntimeManifestEncodingError {
    RuntimeInformation(ProcessingRuntimeInformationEncodingError),
    Serialization(String),
}

impl fmt::Display for ProcessingRuntimeManifestEncodingError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::RuntimeInformation(error) => {
                write!(
                    formatter,
                    "processing runtime information encoding failed: {error}"
                )
            }
            Self::Serialization(message) => {
                write!(
                    formatter,
                    "processing runtime manifest serialization failed: {message}"
                )
            }
        }
    }
}

impl std::error::Error for ProcessingRuntimeManifestEncodingError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::RuntimeInformation(error) => Some(error),
            Self::Serialization(_) => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProcessingRuntimeManifestDecodingError {
    Json(String),
    UnknownSchemaVersion(u32),
    InvalidExecutableName,
    InvalidExecutableSize(u64),
    InvalidSha256(String),
    MalformedRuntimeInformation(ProcessingRuntimeInformationDecodingError),
}

impl fmt::Display for ProcessingRuntimeManifestDecodingError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Json(message) => write!(
                formatter,
                "invalid processing runtime manifest JSON: {message}"
            ),
            Self::UnknownSchemaVersion(version) => {
                write!(
                    formatter,
                    "unknown processing runtime manifest schema version: {version}"
                )
            }
            Self::InvalidExecutableName => {
                formatter.write_str("invalid processing runtime manifest executable name")
            }
            Self::InvalidExecutableSize(size) => {
                write!(
                    formatter,
                    "invalid processing runtime manifest executable size: {size}"
                )
            }
            Self::InvalidSha256(value) => {
                write!(
                    formatter,
                    "invalid processing runtime manifest SHA-256: {value}"
                )
            }
            Self::MalformedRuntimeInformation(error) => {
                write!(
                    formatter,
                    "malformed nested processing runtime information: {error}"
                )
            }
        }
    }
}

impl std::error::Error for ProcessingRuntimeManifestDecodingError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::MalformedRuntimeInformation(error) => Some(error),
            Self::Json(_)
            | Self::UnknownSchemaVersion(_)
            | Self::InvalidExecutableName
            | Self::InvalidExecutableSize(_)
            | Self::InvalidSha256(_) => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProcessingRuntimeManifestV1 {
    executable_name: String,
    executable_size: u64,
    executable_sha256: ExecutableSha256,
    runtime_information: ProcessingRuntimeInformationV1,
}

impl ProcessingRuntimeManifestV1 {
    pub fn executable_name(&self) -> &str {
        &self.executable_name
    }

    pub const fn executable_size(&self) -> u64 {
        self.executable_size
    }

    pub const fn executable_sha256(&self) -> ExecutableSha256 {
        self.executable_sha256
    }

    pub fn runtime_information(&self) -> &ProcessingRuntimeInformationV1 {
        &self.runtime_information
    }

    pub fn from_verified_processing_runtime(
        executable_name: &str,
        verified: &VerifiedProcessingRuntime,
    ) -> Result<Self, ProcessingRuntimeManifestConstructionError> {
        if !is_safe_executable_name(executable_name) {
            return Err(ProcessingRuntimeManifestConstructionError::InvalidExecutableName);
        }

        let sha256 = ExecutableSha256::from_hex(verified.artifact().sha256())
            .map_err(|_| ProcessingRuntimeManifestConstructionError::InvalidExecutableName)?;

        Ok(Self {
            executable_name: executable_name.to_string(),
            executable_size: verified.artifact().size(),
            executable_sha256: sha256,
            runtime_information: *verified.information(),
        })
    }

    pub fn to_json(&self) -> Result<String, ProcessingRuntimeManifestEncodingError> {
        let runtime_information_json = self
            .runtime_information
            .to_json()
            .map_err(ProcessingRuntimeManifestEncodingError::RuntimeInformation)?;
        let runtime_information_value = serde_json::from_str::<serde_json::Value>(
            &runtime_information_json,
        )
        .map_err(|error| {
            ProcessingRuntimeManifestEncodingError::Serialization(format!(
                "nested processing runtime information JSON is malformed: {error}"
            ))
        })?;

        let document = ProcessingRuntimeManifestDocumentV1 {
            schema_version: super::RUNTIME_MANIFEST_SCHEMA_VERSION,
            artifact: ProcessingRuntimeManifestArtifactDocumentV1 {
                executable: self.executable_name.clone(),
                size: self.executable_size,
                sha256: self.executable_sha256.to_string(),
            },
            runtime_information: runtime_information_value,
        };

        serde_json::to_string(&document).map_err(|error| {
            ProcessingRuntimeManifestEncodingError::Serialization(error.to_string())
        })
    }

    pub fn from_json(input: &str) -> Result<Self, ProcessingRuntimeManifestDecodingError> {
        reject_duplicate_json_keys(input)?;

        let document = serde_json::from_str::<ProcessingRuntimeManifestDocumentV1>(input)
            .map_err(|error| ProcessingRuntimeManifestDecodingError::Json(error.to_string()))?;

        if document.schema_version != super::RUNTIME_MANIFEST_SCHEMA_VERSION {
            return Err(
                ProcessingRuntimeManifestDecodingError::UnknownSchemaVersion(
                    document.schema_version,
                ),
            );
        }

        if !is_safe_executable_name(&document.artifact.executable) {
            return Err(ProcessingRuntimeManifestDecodingError::InvalidExecutableName);
        }

        if document.artifact.size == 0 {
            return Err(
                ProcessingRuntimeManifestDecodingError::InvalidExecutableSize(
                    document.artifact.size,
                ),
            );
        }

        let sha256 = ExecutableSha256::from_hex(&document.artifact.sha256).map_err(|_| {
            ProcessingRuntimeManifestDecodingError::InvalidSha256(document.artifact.sha256.clone())
        })?;

        let runtime_information_json = serde_json::to_string(&document.runtime_information)
            .map_err(|error| ProcessingRuntimeManifestDecodingError::Json(error.to_string()))?;
        let runtime_information =
            ProcessingRuntimeInformationV1::from_json(&runtime_information_json)
                .map_err(ProcessingRuntimeManifestDecodingError::MalformedRuntimeInformation)?;

        Ok(Self {
            executable_name: document.artifact.executable,
            executable_size: document.artifact.size,
            executable_sha256: sha256,
            runtime_information,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
struct ProcessingRuntimeManifestDocumentV1 {
    schema_version: u32,
    artifact: ProcessingRuntimeManifestArtifactDocumentV1,
    runtime_information: serde_json::Value,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct ProcessingRuntimeManifestArtifactDocumentV1 {
    executable: String,
    size: u64,
    sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct ProcessingRuntimeManifestJsonDocumentV1 {
    schema_version: u32,
    artifact: ProcessingRuntimeManifestArtifactDocumentV1,
    runtime_information: serde_json::Value,
}

impl<'de> serde::Deserialize<'de> for ProcessingRuntimeManifestDocumentV1 {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let document = ProcessingRuntimeManifestJsonDocumentV1::deserialize(deserializer)?;
        Ok(Self {
            schema_version: document.schema_version,
            artifact: document.artifact,
            runtime_information: document.runtime_information,
        })
    }
}

fn reject_duplicate_json_keys(input: &str) -> Result<(), ProcessingRuntimeManifestDecodingError> {
    let mut parser = DuplicateKeyJsonParser::new(input);
    parser.parse_value()?;
    parser.skip_whitespace();
    if parser.index != parser.source.len() {
        return Err(ProcessingRuntimeManifestDecodingError::Json(
            "unexpected trailing JSON content".to_string(),
        ));
    }
    Ok(())
}

struct DuplicateKeyJsonParser<'a> {
    source: &'a [u8],
    index: usize,
}

impl<'a> DuplicateKeyJsonParser<'a> {
    fn new(input: &'a str) -> Self {
        Self {
            source: input.as_bytes(),
            index: 0,
        }
    }

    fn parse_value(&mut self) -> Result<(), ProcessingRuntimeManifestDecodingError> {
        self.skip_whitespace();
        if self.index >= self.source.len() {
            return Err(ProcessingRuntimeManifestDecodingError::Json(
                "unexpected end of JSON input".to_string(),
            ));
        }

        match self.source[self.index] {
            b'{' => self.parse_object(),
            b'[' => self.parse_array(),
            b'"' => {
                self.parse_string()?;
                Ok(())
            }
            b'-' | b'0'..=b'9' => {
                self.parse_number();
                Ok(())
            }
            b't' => self.consume_literal("true"),
            b'f' => self.consume_literal("false"),
            b'n' => self.consume_literal("null"),
            other => Err(ProcessingRuntimeManifestDecodingError::Json(format!(
                "unexpected JSON token at byte {}: {}",
                self.index,
                char::from(other)
            ))),
        }
    }

    fn parse_object(&mut self) -> Result<(), ProcessingRuntimeManifestDecodingError> {
        self.require_byte(b'{')?;
        self.skip_whitespace();
        if self.consume_if(b'}') {
            return Ok(());
        }

        let mut seen = BTreeSet::new();
        loop {
            self.skip_whitespace();
            let key = self.parse_string()?;
            if !seen.insert(key.clone()) {
                return Err(ProcessingRuntimeManifestDecodingError::Json(format!(
                    "duplicate JSON field: {key}"
                )));
            }
            self.skip_whitespace();
            self.require_byte(b':')?;
            self.parse_value()?;
            self.skip_whitespace();
            if self.consume_if(b',') {
                self.skip_whitespace();
                continue;
            }
            if self.consume_if(b'}') {
                return Ok(());
            }
            return Err(ProcessingRuntimeManifestDecodingError::Json(
                "expected ',' or '}' in JSON object".to_string(),
            ));
        }
    }

    fn parse_array(&mut self) -> Result<(), ProcessingRuntimeManifestDecodingError> {
        self.require_byte(b'[')?;
        self.skip_whitespace();
        if self.consume_if(b']') {
            return Ok(());
        }

        loop {
            self.parse_value()?;
            self.skip_whitespace();
            if self.consume_if(b',') {
                self.skip_whitespace();
                continue;
            }
            if self.consume_if(b']') {
                return Ok(());
            }
            return Err(ProcessingRuntimeManifestDecodingError::Json(
                "expected ',' or ']' in JSON array".to_string(),
            ));
        }
    }

    fn parse_string(&mut self) -> Result<String, ProcessingRuntimeManifestDecodingError> {
        self.require_byte(b'"')?;
        let mut output = String::new();
        while self.index < self.source.len() {
            let byte = self.source[self.index];
            self.index += 1;
            match byte {
                b'"' => return Ok(output),
                b'\\' => {
                    if self.index >= self.source.len() {
                        return Err(ProcessingRuntimeManifestDecodingError::Json(
                            "unterminated escape sequence".to_string(),
                        ));
                    }
                    let escaped = self.source[self.index];
                    self.index += 1;
                    match escaped {
                        b'"' => output.push('"'),
                        b'\\' => output.push('\\'),
                        b'/' => output.push('/'),
                        b'b' => output.push('\u{0008}'),
                        b'f' => output.push('\u{000c}'),
                        b'n' => output.push('\n'),
                        b'r' => output.push('\r'),
                        b't' => output.push('\t'),
                        b'u' => {
                            if self.index + 4 > self.source.len() {
                                return Err(ProcessingRuntimeManifestDecodingError::Json(
                                    "incomplete Unicode escape sequence".to_string(),
                                ));
                            }
                            let mut codepoint = 0_u32;
                            for _ in 0..4 {
                                let digit = self.source[self.index];
                                self.index += 1;
                                let value = match digit {
                                    b'0'..=b'9' => digit - b'0',
                                    b'a'..=b'f' => digit - b'a' + 10,
                                    b'A'..=b'F' => digit - b'A' + 10,
                                    _ => {
                                        return Err(ProcessingRuntimeManifestDecodingError::Json(
                                            "invalid Unicode escape sequence".to_string(),
                                        ));
                                    }
                                } as u32;
                                codepoint = (codepoint << 4) | value;
                            }
                            let ch = char::from_u32(codepoint).ok_or_else(|| {
                                ProcessingRuntimeManifestDecodingError::Json(
                                    "invalid Unicode code point".to_string(),
                                )
                            })?;
                            output.push(ch);
                        }
                        _ => {
                            return Err(ProcessingRuntimeManifestDecodingError::Json(
                                "unsupported JSON escape sequence".to_string(),
                            ));
                        }
                    }
                }
                byte if byte < 0x20 => {
                    return Err(ProcessingRuntimeManifestDecodingError::Json(
                        "control characters are not valid in JSON strings".to_string(),
                    ));
                }
                other => output.push(char::from(other)),
            }
        }

        Err(ProcessingRuntimeManifestDecodingError::Json(
            "unterminated JSON string".to_string(),
        ))
    }

    fn parse_number(&mut self) {
        while self.index < self.source.len() {
            let byte = self.source[self.index];
            match byte {
                b'0'..=b'9' | b'-' | b'+' | b'.' | b'e' | b'E' => {
                    self.index += 1;
                }
                _ => break,
            }
        }
    }

    fn consume_literal(
        &mut self,
        literal: &str,
    ) -> Result<(), ProcessingRuntimeManifestDecodingError> {
        if self.source.get(self.index..self.index + literal.len()) != Some(literal.as_bytes()) {
            return Err(ProcessingRuntimeManifestDecodingError::Json(format!(
                "expected JSON literal '{literal}'"
            )));
        }
        self.index += literal.len();
        Ok(())
    }

    fn require_byte(&mut self, expected: u8) -> Result<(), ProcessingRuntimeManifestDecodingError> {
        self.skip_whitespace();
        if self.index >= self.source.len() {
            return Err(ProcessingRuntimeManifestDecodingError::Json(
                "unexpected end of JSON input".to_string(),
            ));
        }
        if self.source[self.index] != expected {
            return Err(ProcessingRuntimeManifestDecodingError::Json(format!(
                "expected '{}' at byte {}",
                char::from(expected),
                self.index
            )));
        }
        self.index += 1;
        Ok(())
    }

    fn consume_if(&mut self, expected: u8) -> bool {
        self.skip_whitespace();
        if self.index < self.source.len() && self.source[self.index] == expected {
            self.index += 1;
            true
        } else {
            false
        }
    }

    fn skip_whitespace(&mut self) {
        while self.index < self.source.len() {
            match self.source[self.index] {
                b' ' | b'\n' | b'\r' | b'\t' => self.index += 1,
                _ => break,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::os::unix::fs::PermissionsExt;

    use lexicon_core::processing::{ProcessingRuntimeInformationV1, ProcessingSourceContractV1};
    use lexicon_core::runtime::{RuntimeIdentity, RuntimeOperation, RuntimeProtocol};

    use super::{
        ProcessingRuntimeManifestConstructionError, ProcessingRuntimeManifestDecodingError,
        ProcessingRuntimeManifestV1,
    };
    use crate::build::runtime_verification::verify_processing_runtime_candidate;

    fn fixture_verified_runtime() -> crate::build::VerifiedProcessingRuntime {
        let dir = tempfile::tempdir().unwrap();
        let candidate = dir.path().join("processing-runtime-manifest-candidate");
        let source = ProcessingSourceContractV1::new(|_, _| Ok(()));
        let info = ProcessingRuntimeInformationV1::from_processing_source(
            RuntimeIdentity::from_parts(
                Box::leak("example-source".to_owned().into_boxed_str()),
                RuntimeProtocol::Http,
                RuntimeOperation::Processing,
                1,
            ),
            &source,
        )
        .unwrap();
        let json = info.to_json().unwrap();
        let script = format!(
            "#!/bin/sh\nif [ \"$1\" = \"--lexicon-runtime-information-v1\" ]; then\n  printf '%s\\n' '{}'\n  exit 0\nfi\nexit 1\n",
            json.replace('\\', "\\\\").replace('\'', "\\'")
        );
        fs::write(&candidate, script).unwrap();
        let mut permissions = fs::metadata(&candidate).unwrap().permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&candidate, permissions).unwrap();

        verify_processing_runtime_candidate(
            &candidate,
            RuntimeIdentity::from_parts(
                Box::leak("example-source".to_owned().into_boxed_str()),
                RuntimeProtocol::Http,
                RuntimeOperation::Processing,
                1,
            ),
        )
        .unwrap()
    }

    #[test]
    fn verified_runtime_constructs_manifest() {
        let verified = fixture_verified_runtime();
        let manifest = ProcessingRuntimeManifestV1::from_verified_processing_runtime(
            "example-source-process-data",
            &verified,
        )
        .unwrap();

        assert_eq!(manifest.executable_name(), "example-source-process-data");
        assert_eq!(manifest.executable_size(), verified.artifact().size());
        assert_eq!(
            manifest.executable_sha256().to_string(),
            verified.artifact().sha256()
        );
        assert_eq!(
            manifest.runtime_information().identity(),
            verified.information().identity()
        );
    }

    #[test]
    fn constructor_rejects_invalid_names() {
        let verified = fixture_verified_runtime();
        assert!(matches!(
            ProcessingRuntimeManifestV1::from_verified_processing_runtime("", &verified),
            Err(ProcessingRuntimeManifestConstructionError::InvalidExecutableName)
        ));
        assert!(matches!(
            ProcessingRuntimeManifestV1::from_verified_processing_runtime(".", &verified),
            Err(ProcessingRuntimeManifestConstructionError::InvalidExecutableName)
        ));
        assert!(matches!(
            ProcessingRuntimeManifestV1::from_verified_processing_runtime("../runtime", &verified),
            Err(ProcessingRuntimeManifestConstructionError::InvalidExecutableName)
        ));
    }

    #[test]
    fn manifest_json_round_trip_preserves_equality() {
        let verified = fixture_verified_runtime();
        let original = ProcessingRuntimeManifestV1::from_verified_processing_runtime(
            "example-source-process-data",
            &verified,
        )
        .unwrap();
        let json = original.to_json().unwrap();
        let decoded = ProcessingRuntimeManifestV1::from_json(&json).unwrap();

        assert_eq!(decoded, original);
    }

    #[test]
    fn duplicate_keys_are_rejected() {
        let json = r#"{"schema_version":1,"schema_version":2,"artifact":{"executable":"x","size":1,"sha256":"0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"},"runtime_information":{"schema_version":1,"identity":{"source":"example-source","protocol":"http","operation":"processing","source_contract_version":1},"descriptor":{"contract_version":1}}}"#;
        assert!(matches!(
            ProcessingRuntimeManifestV1::from_json(json),
            Err(ProcessingRuntimeManifestDecodingError::Json(_))
        ));
    }
}
