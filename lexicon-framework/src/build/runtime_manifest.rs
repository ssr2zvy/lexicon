use std::collections::BTreeSet;
use std::fmt;

use lexicon_core::runtime::{RuntimeInformationDecodingError, RuntimeInformationEncodingError, RuntimeInformationV1};

use super::is_safe_executable_name;
use super::runtime_verification::VerifiedHttpRuntime;

pub const RUNTIME_MANIFEST_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ExecutableSha256([u8; 32]);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExecutableSha256ParseError {
    InvalidLength(usize),
    InvalidCharacter { index: usize, value: char },
}

impl ExecutableSha256 {
    pub fn from_hex(value: &str) -> Result<Self, ExecutableSha256ParseError> {
        if value.len() != 64 {
            return Err(ExecutableSha256ParseError::InvalidLength(value.len()));
        }

        let mut bytes = [0_u8; 32];
        for (index, chunk) in value.as_bytes().chunks_exact(2).enumerate() {
            let hi = chunk[0];
            let lo = chunk[1];
            let hi_value = match hi {
                b'0'..=b'9' => hi - b'0',
                b'a'..=b'f' => hi - b'a' + 10,
                _ => {
                    return Err(ExecutableSha256ParseError::InvalidCharacter {
                        index: index * 2,
                        value: char::from(hi),
                    });
                }
            };
            let lo_value = match lo {
                b'0'..=b'9' => lo - b'0',
                b'a'..=b'f' => lo - b'a' + 10,
                _ => {
                    return Err(ExecutableSha256ParseError::InvalidCharacter {
                        index: index * 2 + 1,
                        value: char::from(lo),
                    });
                }
            };
            bytes[index] = (hi_value << 4) | lo_value;
        }

        Ok(Self(bytes))
    }

    pub fn as_hex(&self) -> String {
        let mut output = String::with_capacity(64);
        for byte in &self.0 {
            output.push(char::from(b"0123456789abcdef"[usize::from(byte >> 4)]));
            output.push(char::from(b"0123456789abcdef"[usize::from(byte & 0x0f)]));
        }
        output
    }

    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

impl fmt::Display for ExecutableSha256 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.as_hex())
    }
}

impl fmt::Display for ExecutableSha256ParseError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidLength(length) => {
                write!(formatter, "invalid SHA-256 length: expected 64 chars, found {length}")
            }
            Self::InvalidCharacter { index, value } => {
                write!(
                    formatter,
                    "invalid SHA-256 character at index {index}: {value:?}"
                )
            }
        }
    }
}

impl std::error::Error for ExecutableSha256ParseError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuntimeManifestConstructionError {
    InvalidExecutableName,
}

impl fmt::Display for RuntimeManifestConstructionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidExecutableName => formatter.write_str("invalid runtime manifest executable name"),
        }
    }
}

impl std::error::Error for RuntimeManifestConstructionError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuntimeManifestEncodingError {
    RuntimeInformation(RuntimeInformationEncodingError),
    Serialization(String),
}

impl fmt::Display for RuntimeManifestEncodingError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::RuntimeInformation(error) => write!(formatter, "runtime information encoding failed: {error}"),
            Self::Serialization(message) => write!(formatter, "runtime manifest serialization failed: {message}"),
        }
    }
}

impl std::error::Error for RuntimeManifestEncodingError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::RuntimeInformation(error) => Some(error),
            Self::Serialization(_) => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuntimeManifestDecodingError {
    Json(String),
    UnknownSchemaVersion(u32),
    InvalidExecutableName,
    InvalidExecutableSize(u64),
    InvalidSha256(String),
    MalformedRuntimeInformation(RuntimeInformationDecodingError),
}

impl fmt::Display for RuntimeManifestDecodingError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Json(message) => write!(formatter, "invalid runtime manifest JSON: {message}"),
            Self::UnknownSchemaVersion(version) => {
                write!(formatter, "unknown runtime manifest schema version: {version}")
            }
            Self::InvalidExecutableName => formatter.write_str("invalid runtime manifest executable name"),
            Self::InvalidExecutableSize(size) => {
                write!(formatter, "invalid runtime manifest executable size: {size}")
            }
            Self::InvalidSha256(value) => {
                write!(formatter, "invalid runtime manifest SHA-256: {value}")
            }
            Self::MalformedRuntimeInformation(error) => {
                write!(formatter, "malformed nested runtime information: {error}")
            }
        }
    }
}

impl std::error::Error for RuntimeManifestDecodingError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::MalformedRuntimeInformation(error) => Some(error),
            Self::Json(_) | Self::UnknownSchemaVersion(_) | Self::InvalidExecutableName | Self::InvalidExecutableSize(_) | Self::InvalidSha256(_) => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeManifestV1 {
    executable_name: String,
    executable_size: u64,
    executable_sha256: ExecutableSha256,
    runtime_information: RuntimeInformationV1,
}

impl RuntimeManifestV1 {
    pub fn executable_name(&self) -> &str {
        &self.executable_name
    }

    pub const fn executable_size(&self) -> u64 {
        self.executable_size
    }

    pub const fn executable_sha256(&self) -> ExecutableSha256 {
        self.executable_sha256
    }

    pub fn runtime_information(&self) -> &RuntimeInformationV1 {
        &self.runtime_information
    }

    pub fn from_verified_http_runtime(
        executable_name: &str,
        verified: &VerifiedHttpRuntime,
    ) -> Result<Self, RuntimeManifestConstructionError> {
        if !is_safe_executable_name(executable_name) {
            return Err(RuntimeManifestConstructionError::InvalidExecutableName);
        }

        let sha256 = ExecutableSha256::from_hex(verified.artifact().sha256())
            .map_err(|_| RuntimeManifestConstructionError::InvalidExecutableName)?;

        Ok(Self {
            executable_name: executable_name.to_string(),
            executable_size: verified.artifact().size(),
            executable_sha256: sha256,
            runtime_information: *verified.information(),
        })
    }

    pub fn to_json(&self) -> Result<String, RuntimeManifestEncodingError> {
        let runtime_information_json = self
            .runtime_information
            .to_json()
            .map_err(RuntimeManifestEncodingError::RuntimeInformation)?;
        let runtime_information_value = serde_json::from_str::<serde_json::Value>(&runtime_information_json)
            .map_err(|error| {
                RuntimeManifestEncodingError::Serialization(format!(
                    "nested runtime information JSON is malformed: {error}"
                ))
            })?;

        let document = RuntimeManifestDocumentV1 {
            schema_version: RUNTIME_MANIFEST_SCHEMA_VERSION,
            artifact: RuntimeManifestArtifactDocumentV1 {
                executable: self.executable_name.clone(),
                size: self.executable_size,
                sha256: self.executable_sha256.to_string(),
            },
            runtime_information: runtime_information_value,
        };

        serde_json::to_string(&document)
            .map_err(|error| RuntimeManifestEncodingError::Serialization(error.to_string()))
    }

    pub fn from_json(input: &str) -> Result<Self, RuntimeManifestDecodingError> {
        reject_duplicate_json_keys(input)?;

        let document = serde_json::from_str::<RuntimeManifestDocumentV1>(input).map_err(|error| {
            RuntimeManifestDecodingError::Json(error.to_string())
        })?;

        if document.schema_version != RUNTIME_MANIFEST_SCHEMA_VERSION {
            return Err(RuntimeManifestDecodingError::UnknownSchemaVersion(
                document.schema_version,
            ));
        }

        if !is_safe_executable_name(&document.artifact.executable) {
            return Err(RuntimeManifestDecodingError::InvalidExecutableName);
        }

        if document.artifact.size == 0 {
            return Err(RuntimeManifestDecodingError::InvalidExecutableSize(
                document.artifact.size,
            ));
        }

        let sha256 = ExecutableSha256::from_hex(&document.artifact.sha256)
            .map_err(|_| RuntimeManifestDecodingError::InvalidSha256(document.artifact.sha256.clone()))?;

        let runtime_information_json = serde_json::to_string(&document.runtime_information)
            .map_err(|error| RuntimeManifestDecodingError::Json(error.to_string()))?;
        let runtime_information = RuntimeInformationV1::from_json(&runtime_information_json)
            .map_err(RuntimeManifestDecodingError::MalformedRuntimeInformation)?;

        Ok(Self {
            executable_name: document.artifact.executable,
            executable_size: document.artifact.size,
            executable_sha256: sha256,
            runtime_information,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
struct RuntimeManifestDocumentV1 {
    schema_version: u32,
    artifact: RuntimeManifestArtifactDocumentV1,
    runtime_information: serde_json::Value,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct RuntimeManifestArtifactDocumentV1 {
    executable: String,
    size: u64,
    sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct RuntimeManifestJsonDocumentV1 {
    schema_version: u32,
    artifact: RuntimeManifestArtifactDocumentV1,
    runtime_information: serde_json::Value,
}

impl<'de> serde::Deserialize<'de> for RuntimeManifestDocumentV1 {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let document = RuntimeManifestJsonDocumentV1::deserialize(deserializer)?;
        Ok(Self {
            schema_version: document.schema_version,
            artifact: document.artifact,
            runtime_information: document.runtime_information,
        })
    }
}

fn reject_duplicate_json_keys(input: &str) -> Result<(), RuntimeManifestDecodingError> {
    let mut parser = DuplicateKeyJsonParser::new(input);
    parser.parse_value()?;
    parser.skip_whitespace();
    if parser.index != parser.source.len() {
        return Err(RuntimeManifestDecodingError::Json(
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

    fn parse_value(&mut self) -> Result<(), RuntimeManifestDecodingError> {
        self.skip_whitespace();
        if self.index >= self.source.len() {
            return Err(RuntimeManifestDecodingError::Json(
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
            other => Err(RuntimeManifestDecodingError::Json(format!(
                "unexpected JSON token at byte {}: {}",
                self.index,
                char::from(other)
            ))),
        }
    }

    fn parse_object(&mut self) -> Result<(), RuntimeManifestDecodingError> {
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
                return Err(RuntimeManifestDecodingError::Json(format!(
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
            return Err(RuntimeManifestDecodingError::Json(
                "expected ',' or '}' in JSON object".to_string(),
            ));
        }
    }

    fn parse_array(&mut self) -> Result<(), RuntimeManifestDecodingError> {
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
            return Err(RuntimeManifestDecodingError::Json(
                "expected ',' or ']' in JSON array".to_string(),
            ));
        }
    }

    fn parse_string(&mut self) -> Result<String, RuntimeManifestDecodingError> {
        self.require_byte(b'"')?;
        let mut output = String::new();
        while self.index < self.source.len() {
            let byte = self.source[self.index];
            self.index += 1;
            match byte {
                b'"' => return Ok(output),
                b'\\' => {
                    if self.index >= self.source.len() {
                        return Err(RuntimeManifestDecodingError::Json(
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
                                return Err(RuntimeManifestDecodingError::Json(
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
                                        return Err(RuntimeManifestDecodingError::Json(
                                            "invalid Unicode escape sequence".to_string(),
                                        ));
                                    }
                                } as u32;
                                codepoint = (codepoint << 4) | value;
                            }
                            let ch = char::from_u32(codepoint).ok_or_else(|| {
                                RuntimeManifestDecodingError::Json(
                                    "invalid Unicode code point".to_string(),
                                )
                            })?;
                            output.push(ch);
                        }
                        _ => {
                            return Err(RuntimeManifestDecodingError::Json(
                                "unsupported JSON escape sequence".to_string(),
                            ));
                        }
                    }
                }
                byte if byte < 0x20 => {
                    return Err(RuntimeManifestDecodingError::Json(
                        "control characters are not valid in JSON strings".to_string(),
                    ));
                }
                other => output.push(char::from(other)),
            }
        }

        Err(RuntimeManifestDecodingError::Json(
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

    fn consume_literal(&mut self, literal: &str) -> Result<(), RuntimeManifestDecodingError> {
        if self.source.get(self.index..self.index + literal.len()) != Some(literal.as_bytes()) {
            return Err(RuntimeManifestDecodingError::Json(format!(
                "expected JSON literal '{literal}'"
            )));
        }
        self.index += literal.len();
        Ok(())
    }

    fn require_byte(&mut self, expected: u8) -> Result<(), RuntimeManifestDecodingError> {
        self.skip_whitespace();
        if self.index >= self.source.len() {
            return Err(RuntimeManifestDecodingError::Json(
                "unexpected end of JSON input".to_string(),
            ));
        }
        if self.source[self.index] != expected {
            return Err(RuntimeManifestDecodingError::Json(format!(
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

    use lexicon_core::protocols::http::{HttpCapabilitySet, HttpSourceContractV1};
    use lexicon_core::runtime::{RuntimeIdentity, RuntimeInformationV1};

    use super::{
        ExecutableSha256, RuntimeManifestConstructionError, RuntimeManifestDecodingError,
        RuntimeManifestV1, RUNTIME_MANIFEST_SCHEMA_VERSION, is_safe_executable_name,
    };
    use crate::build::runtime_verification::verify_http_runtime_candidate;

    fn fixture_verified_runtime() -> crate::build::VerifiedHttpRuntime {
        let dir = tempfile::tempdir().unwrap();
        let candidate = dir.path().join("runtime-manifest-candidate");
        let source = HttpSourceContractV1::new(|_, _| Ok(()));
        let info = RuntimeInformationV1::from_http_source(
            RuntimeIdentity::http_acquisition("example-source", 1),
            &source,
            HttpCapabilitySet::empty(),
        );
        let json = info.to_json().unwrap();
        let script = format!(
            "#!/bin/sh\nif [ \"$1\" = \"--lexicon-runtime-information-v1\" ]; then\n  printf '%s\\n' '{}'\n  exit 0\nfi\nexit 1\n",
            json.replace('\\', "\\\\").replace('\'', "\\'")
        );
        fs::write(&candidate, script).unwrap();
        let mut permissions = fs::metadata(&candidate).unwrap().permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&candidate, permissions).unwrap();

        verify_http_runtime_candidate(&candidate, RuntimeIdentity::http_acquisition("example-source", 1)).unwrap()
    }

    #[test]
    fn verified_runtime_constructs_manifest() {
        let verified = fixture_verified_runtime();
        let manifest = RuntimeManifestV1::from_verified_http_runtime("example-source-get-raw-data", &verified).unwrap();

        assert_eq!(manifest.executable_name(), "example-source-get-raw-data");
        assert_eq!(manifest.executable_size(), verified.artifact().size());
        assert_eq!(manifest.executable_sha256().to_string(), verified.artifact().sha256());
        assert_eq!(manifest.runtime_information().identity(), verified.information().identity());
    }

    #[test]
    fn manifest_size_and_digest_came_from_verified_artifact() {
        let verified = fixture_verified_runtime();
        let manifest = RuntimeManifestV1::from_verified_http_runtime("rv", &verified).unwrap();

        assert_eq!(manifest.executable_size(), verified.artifact().size());
        assert_eq!(manifest.executable_sha256().to_string(), verified.artifact().sha256());
    }

    #[test]
    fn manifest_uses_admitted_runtime_information() {
        let verified = fixture_verified_runtime();
        let manifest = RuntimeManifestV1::from_verified_http_runtime("runtime", &verified).unwrap();

        assert_eq!(manifest.runtime_information().to_json().unwrap(), verified.information().to_json().unwrap());
    }

    #[test]
    fn construction_does_not_allow_independent_digest_substitution() {
        let verified = fixture_verified_runtime();
        let manifest = RuntimeManifestV1::from_verified_http_runtime("runtime", &verified).unwrap();
        let other = ExecutableSha256::from_hex(&"0".repeat(64)).unwrap();

        assert_ne!(manifest.executable_sha256(), other);
        assert_eq!(manifest.executable_sha256().to_string(), verified.artifact().sha256());
    }

    #[test]
    fn safe_executable_names_are_accepted() {
        assert!(is_safe_executable_name("example-source-get-raw-data"));
        assert!(is_safe_executable_name("example-source-get-raw-data.exe"));
    }

    #[test]
    fn invalid_executable_names_are_rejected() {
        assert!(!is_safe_executable_name(""));
        assert!(!is_safe_executable_name("."));
        assert!(!is_safe_executable_name(".."));
        assert!(!is_safe_executable_name("folder/runtime"));
        assert!(!is_safe_executable_name("folder\\runtime"));
        assert!(!is_safe_executable_name("/tmp/runtime"));
        assert!(!is_safe_executable_name("C:\\temp\\runtime.exe"));
        assert!(!is_safe_executable_name("C:/temp/runtime.exe"));
        assert!(!is_safe_executable_name("bad\0name"));
        assert!(!is_safe_executable_name("./runtime"));
        assert!(!is_safe_executable_name("../runtime"));
    }

    #[test]
    fn constructor_rejects_invalid_names() {
        let verified = fixture_verified_runtime();
        assert!(matches!(
            RuntimeManifestV1::from_verified_http_runtime("", &verified),
            Err(RuntimeManifestConstructionError::InvalidExecutableName)
        ));
        assert!(matches!(
            RuntimeManifestV1::from_verified_http_runtime(".", &verified),
            Err(RuntimeManifestConstructionError::InvalidExecutableName)
        ));
        assert!(matches!(
            RuntimeManifestV1::from_verified_http_runtime("../runtime", &verified),
            Err(RuntimeManifestConstructionError::InvalidExecutableName)
        ));
    }

    #[test]
    fn manifest_json_has_no_temp_path_and_lowercase_digest() {
        let verified = fixture_verified_runtime();
        let manifest = RuntimeManifestV1::from_verified_http_runtime("example-source-get-raw-data", &verified).unwrap();
        let json = manifest.to_json().unwrap();

        assert!(!json.contains(verified.artifact().path().to_string_lossy().as_ref()));
        assert!(json.contains(&format!("\"sha256\":\"{}\"", manifest.executable_sha256())));
        assert_eq!(json.matches("\"sha256\":\"").count(), 1);
        assert!(manifest.executable_sha256().to_string().chars().all(|ch| ch.is_ascii_hexdigit() && ch.is_ascii_lowercase() || ch.is_ascii_digit()));
        assert_eq!(manifest.executable_sha256().to_string().len(), 64);
    }

    #[test]
    fn manifest_json_round_trip_preserves_equality() {
        let verified = fixture_verified_runtime();
        let original = RuntimeManifestV1::from_verified_http_runtime("example-source-get-raw-data", &verified).unwrap();
        let json = original.to_json().unwrap();
        let decoded = RuntimeManifestV1::from_json(&json).unwrap();

        assert_eq!(decoded, original);
    }

    #[test]
    fn invalid_json_is_rejected() {
        assert!(matches!(
            RuntimeManifestV1::from_json("{not valid json}"),
            Err(RuntimeManifestDecodingError::Json(_))
        ));
    }

    #[test]
    fn unknown_fields_are_rejected() {
        let json = r#"{"schema_version":1,"artifact":{"executable":"x","size":1,"sha256":"0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef","extra":true},"runtime_information":{"schema_version":1,"identity":{"source":"example-source","protocol":"http","operation":"acquisition","source_contract_version":1},"descriptor":{"contract_version":1,"required_capabilities":[],"resume_handler_registered":false},"runtime":{"available_capabilities":[]}}}"#;
        assert!(matches!(
            RuntimeManifestV1::from_json(json),
            Err(RuntimeManifestDecodingError::Json(_))
        ));
    }

    #[test]
    fn missing_fields_are_rejected() {
        assert!(matches!(
            RuntimeManifestV1::from_json(r#"{"schema_version":1,"artifact":{"executable":"x","size":1}}"#),
            Err(RuntimeManifestDecodingError::Json(_))
        ));
    }

    #[test]
    fn unknown_schema_version_is_rejected() {
        let runtime = RuntimeInformationV1::from_http_source(
            RuntimeIdentity::http_acquisition("example-source", 1),
            &HttpSourceContractV1::new(|_, _| Ok(())),
            HttpCapabilitySet::empty(),
        );
        let valid_runtime = runtime.to_json().unwrap();
        let json = format!(
            "{{\"schema_version\":2,\"artifact\":{{\"executable\":\"x\",\"size\":1,\"sha256\":\"{}\"}},\"runtime_information\":{}}}",
            "0".repeat(64),
            valid_runtime
        );
        assert!(matches!(
            RuntimeManifestV1::from_json(&json),
            Err(RuntimeManifestDecodingError::UnknownSchemaVersion(2))
        ));
    }

    #[test]
    fn zero_size_is_rejected() {
        let runtime = RuntimeInformationV1::from_http_source(
            RuntimeIdentity::http_acquisition("example-source", 1),
            &HttpSourceContractV1::new(|_, _| Ok(())),
            HttpCapabilitySet::empty(),
        );
        let valid_runtime = runtime.to_json().unwrap();
        let json = format!(
            "{{\"schema_version\":1,\"artifact\":{{\"executable\":\"x\",\"size\":0,\"sha256\":\"{}\"}},\"runtime_information\":{}}}",
            "0".repeat(64),
            valid_runtime
        );
        assert!(matches!(
            RuntimeManifestV1::from_json(&json),
            Err(RuntimeManifestDecodingError::InvalidExecutableSize(0))
        ));
    }

    #[test]
    fn invalid_sha_values_are_rejected() {
        let runtime = RuntimeInformationV1::from_http_source(
            RuntimeIdentity::http_acquisition("example-source", 1),
            &HttpSourceContractV1::new(|_, _| Ok(())),
            HttpCapabilitySet::empty(),
        );
        let valid_runtime = runtime.to_json().unwrap();

        let short = format!(
            "{{\"schema_version\":1,\"artifact\":{{\"executable\":\"x\",\"size\":1,\"sha256\":\"{}\"}},\"runtime_information\":{}}}",
            "abc",
            valid_runtime
        );
        assert!(matches!(
            RuntimeManifestV1::from_json(&short),
            Err(RuntimeManifestDecodingError::InvalidSha256(_))
        ));

        let uppercase = format!(
            "{{\"schema_version\":1,\"artifact\":{{\"executable\":\"x\",\"size\":1,\"sha256\":\"{}\"}},\"runtime_information\":{}}}",
            "A".repeat(64),
            valid_runtime
        );
        assert!(matches!(
            RuntimeManifestV1::from_json(&uppercase),
            Err(RuntimeManifestDecodingError::InvalidSha256(_))
        ));

        let non_hex = format!(
            "{{\"schema_version\":1,\"artifact\":{{\"executable\":\"x\",\"size\":1,\"sha256\":\"{}\"}},\"runtime_information\":{}}}",
            "z".repeat(64),
            valid_runtime
        );
        assert!(matches!(
            RuntimeManifestV1::from_json(&non_hex),
            Err(RuntimeManifestDecodingError::InvalidSha256(_))
        ));
    }

    #[test]
    fn malformed_runtime_information_is_rejected() {
        let json = format!(
            "{{\"schema_version\":{},\"artifact\":{{\"executable\":\"x\",\"size\":1,\"sha256\":\"{}\"}},\"runtime_information\":{{\"schema_version\":999}}}}",
            RUNTIME_MANIFEST_SCHEMA_VERSION,
            "0".repeat(64)
        );
        assert!(matches!(
            RuntimeManifestV1::from_json(&json),
            Err(RuntimeManifestDecodingError::MalformedRuntimeInformation(_))
        ));
    }

    #[test]
    fn structurally_valid_incompatible_runtime_information_may_decode() {
        let info = RuntimeInformationV1::from_http_source(
            RuntimeIdentity::http_acquisition("example-source", 1),
            &HttpSourceContractV1::new(|_, _| Ok(())),
            HttpCapabilitySet::empty(),
        );
        let valid_runtime = info.to_json().unwrap();
        let json = format!(
            "{{\"schema_version\":1,\"artifact\":{{\"executable\":\"x\",\"size\":1,\"sha256\":\"{}\"}},\"runtime_information\":{}}}",
            "0".repeat(64),
            valid_runtime.replace("\"source\":\"example-source\"", "\"source\":\"different-source\"")
        );
        assert!(RuntimeManifestV1::from_json(&json).is_ok());
    }

    #[test]
    fn duplicate_keys_are_rejected() {
        let json = r#"{"schema_version":1,"schema_version":2,"artifact":{"executable":"x","size":1,"sha256":"0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"},"runtime_information":{"schema_version":1,"identity":{"source":"example-source","protocol":"http","operation":"acquisition","source_contract_version":1},"descriptor":{"contract_version":1,"required_capabilities":[],"resume_handler_registered":false},"runtime":{"available_capabilities":[]}}}"#;
        assert!(matches!(
            RuntimeManifestV1::from_json(json),
            Err(RuntimeManifestDecodingError::Json(_))
        ));
    }
}
