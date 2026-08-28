use std::path::{Component, Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::runtime::invocation::{ProjectInvocationIdentity, SessionInvocationIdentity};
use crate::runtime::{OwnedRuntimeIdentity, RuntimeOperation, RuntimeProtocol};
use crate::session::error::{
    RuntimeContextDecodingError, RuntimeContextEncodingError, RuntimeContextError,
    SessionDecodingError,
};
use crate::session::model::{ProjectIdentity, SessionIdentity, SESSION_SCHEMA_VERSION};

/// Environment variable that carries the JSON runtime context document.
pub const RUNTIME_CONTEXT_ENVIRONMENT_VARIABLE: &str = "LEXICON_RUNTIME_CONTEXT_V1";

// ---------------------------------------------------------------------------
// RuntimeContextPaths
// ---------------------------------------------------------------------------

/// Validated filesystem paths bound to a single runtime invocation.
///
/// The constructor validates structural relationships between paths and rejects
/// relative roots, path traversal, and operation/protocol/session disagreements.
#[derive(Debug, Clone)]
pub struct RuntimeContextPaths {
    project_root: PathBuf,
    protocol_root: PathBuf,
    operation_root: PathBuf,
    session_directory: PathBuf,
    raw_data_directory: PathBuf,
    processed_data_directory: PathBuf,
    source_state_directory: Option<PathBuf>,
}

impl RuntimeContextPaths {
    /// Construct and validate `RuntimeContextPaths`.
    ///
    /// Required relationships for HTTP acquisition:
    /// ```text
    /// protocol_root          = sources/<source>/http
    /// operation_root         = protocol_root/get-raw-data
    /// session_directory      = operation_root/sessions/<session-id>
    /// raw_data_directory     = protocol_root/data/raw
    /// processed_data_directory = protocol_root/data/processed
    /// ```
    ///
    /// Required relationships for processing:
    /// ```text
    /// protocol_root          = sources/<source>/http
    /// operation_root         = protocol_root/process-data
    /// session_directory      = operation_root/sessions/<session-id>
    /// raw_data_directory     = protocol_root/data/raw
    /// processed_data_directory = protocol_root/data/processed
    /// ```
    ///
    /// `source_state_directory` is the contract-reserved durable-state boundary for an
    /// acquisition source (contract.md §9). It must be `Some(operation_root/state)` when
    /// `operation` is [`RuntimeOperation::Acquisition`], and `None` for
    /// [`RuntimeOperation::Processing`], which has no source-state directory.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        project_root: PathBuf,
        protocol_root: PathBuf,
        operation_root: PathBuf,
        session_directory: PathBuf,
        raw_data_directory: PathBuf,
        processed_data_directory: PathBuf,
        source_state_directory: Option<PathBuf>,
        operation: RuntimeOperation,
        session: &SessionIdentity,
    ) -> Result<Self, RuntimeContextError> {
        if project_root.is_relative() {
            return Err(RuntimeContextError::RelativeProjectRoot);
        }

        reject_traversal(&protocol_root, "protocol_root")?;
        reject_traversal(&operation_root, "operation_root")?;
        reject_traversal(&session_directory, "session_directory")?;
        reject_traversal(&raw_data_directory, "raw_data_directory")?;
        reject_traversal(&processed_data_directory, "processed_data_directory")?;
        if let Some(ref source_state_directory) = source_state_directory {
            reject_traversal(source_state_directory, "source_state_directory")?;
        }

        // Validate operation root against protocol root
        let expected_op_name = match operation {
            RuntimeOperation::Acquisition => "get-raw-data",
            RuntimeOperation::Processing => "process-data",
        };
        let expected_op_root = protocol_root.join(expected_op_name);
        if operation_root != expected_op_root {
            return Err(RuntimeContextError::OperationRootDisagreement);
        }

        // Validate raw / processed data directories against protocol root
        let expected_raw = protocol_root.join("data/raw");
        if raw_data_directory != expected_raw {
            return Err(RuntimeContextError::PathMismatch {
                field: "raw_data_directory",
                expected: expected_raw.display().to_string(),
                actual: raw_data_directory.display().to_string(),
            });
        }
        let expected_processed = protocol_root.join("data/processed");
        if processed_data_directory != expected_processed {
            return Err(RuntimeContextError::PathMismatch {
                field: "processed_data_directory",
                expected: expected_processed.display().to_string(),
                actual: processed_data_directory.display().to_string(),
            });
        }

        // Validate session directory contains the session identity
        let expected_session_dir = operation_root.join("sessions").join(session.id());
        if session_directory != expected_session_dir {
            return Err(RuntimeContextError::SessionDirectoryDisagreement);
        }

        // Validate the durable source-state directory: reserved for acquisition only.
        let expected_source_state_directory = match operation {
            RuntimeOperation::Acquisition => Some(operation_root.join("state")),
            RuntimeOperation::Processing => None,
        };
        if source_state_directory != expected_source_state_directory {
            return Err(RuntimeContextError::SourceStateDirectoryDisagreement);
        }

        Ok(Self {
            project_root,
            protocol_root,
            operation_root,
            session_directory,
            raw_data_directory,
            processed_data_directory,
            source_state_directory,
        })
    }

    pub fn project_root(&self) -> &Path { &self.project_root }
    pub fn protocol_root(&self) -> &Path { &self.protocol_root }
    pub fn operation_root(&self) -> &Path { &self.operation_root }
    pub fn session_directory(&self) -> &Path { &self.session_directory }
    pub fn raw_data_directory(&self) -> &Path { &self.raw_data_directory }
    pub fn processed_data_directory(&self) -> &Path { &self.processed_data_directory }
    /// The contract-reserved durable-state directory for an acquisition source
    /// (`get-raw-data/state/`). `None` for processing, which has no source-state directory.
    pub fn source_state_directory(&self) -> Option<&Path> { self.source_state_directory.as_deref() }
}

fn reject_traversal(path: &Path, field: &'static str) -> Result<(), RuntimeContextError> {
    for component in path.components() {
        if matches!(component, Component::ParentDir) {
            return Err(RuntimeContextError::PathTraversal { field });
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// RuntimeContextDocument (serde representation)
// ---------------------------------------------------------------------------

const RUNTIME_CONTEXT_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct RuntimeContextDocumentV1 {
    schema_version: u32,
    project: ContextProjectDocument,
    runtime: ContextRuntimeDocument,
    session: ContextSessionDocument,
    project_root: EncodedNativePathDocument,
    protocol_root: EncodedNativePathDocument,
    operation_root: EncodedNativePathDocument,
    session_directory: EncodedNativePathDocument,
    raw_data_directory: EncodedNativePathDocument,
    processed_data_directory: EncodedNativePathDocument,
    /// The contract-reserved durable-state directory (`get-raw-data/state/`).
    /// `None` for processing invocations, which have no source-state directory.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    source_state_directory: Option<EncodedNativePathDocument>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct EncodedNativePathDocument {
    encoding: String,
    value: serde_json::Value,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ContextProjectDocument {
    name: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ContextRuntimeDocument {
    source: String,
    protocol: String,
    operation: String,
    source_contract_version: u32,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ContextSessionDocument {
    id: String,
}

// ---------------------------------------------------------------------------
// Encode
// ---------------------------------------------------------------------------

/// Encode a runtime context document for transport in the environment variable.
///
/// The document must not contain source arguments or credentials.
pub fn encode_runtime_context(
    project: &ProjectIdentity,
    runtime: &OwnedRuntimeIdentity,
    session: &SessionIdentity,
    paths: &RuntimeContextPaths,
) -> Result<String, RuntimeContextError> {
    let protocol_str = match runtime.protocol() {
        RuntimeProtocol::Http => "http",
    };
    let doc = RuntimeContextDocumentV1 {
        schema_version: RUNTIME_CONTEXT_SCHEMA_VERSION,
        project: ContextProjectDocument { name: project.name().to_string() },
        runtime: ContextRuntimeDocument {
            source: runtime.source_name().to_string(),
            protocol: protocol_str.to_string(),
            operation: runtime.operation().identifier().to_string(),
            source_contract_version: runtime.source_contract_version(),
        },
        session: ContextSessionDocument { id: session.id().to_string() },
        project_root: encode_native_path(paths.project_root())?,
        protocol_root: encode_native_path(paths.protocol_root())?,
        operation_root: encode_native_path(paths.operation_root())?,
        session_directory: encode_native_path(paths.session_directory())?,
        raw_data_directory: encode_native_path(paths.raw_data_directory())?,
        processed_data_directory: encode_native_path(paths.processed_data_directory())?,
        source_state_directory: paths
            .source_state_directory()
            .map(encode_native_path)
            .transpose()?,
    };

    serde_json::to_string(&doc).map_err(|e| {
        RuntimeContextError::Encoding(RuntimeContextEncodingError::Serialization(e))
    })
}

// ---------------------------------------------------------------------------
// SessionDataPaths
// ---------------------------------------------------------------------------

/// Typed path bundle for one session, derived from validated roots and identities.
///
/// Constructed only from a validated `RuntimeContextPaths`; no raw path arithmetic
/// is exposed to callers. Fields are private; use the provided accessors.
#[derive(Debug, Clone)]
pub struct SessionDataPaths {
    protocol_root: std::path::PathBuf,
    raw_data_directory: std::path::PathBuf,
    processed_data_directory: std::path::PathBuf,
    operation_root: std::path::PathBuf,
    session_directory: std::path::PathBuf,
    source_state_directory: Option<std::path::PathBuf>,
}

impl SessionDataPaths {
    /// Derive paths from an already-validated `RuntimeContextPaths`.
    pub fn from_context_paths(paths: &RuntimeContextPaths) -> Self {
        Self {
            protocol_root: paths.protocol_root().to_path_buf(),
            raw_data_directory: paths.raw_data_directory().to_path_buf(),
            processed_data_directory: paths.processed_data_directory().to_path_buf(),
            operation_root: paths.operation_root().to_path_buf(),
            session_directory: paths.session_directory().to_path_buf(),
            source_state_directory: paths.source_state_directory().map(|p| p.to_path_buf()),
        }
    }

    pub fn from_legacy_parts(
        protocol_root: std::path::PathBuf,
        operation_root: std::path::PathBuf,
        session_directory: std::path::PathBuf,
        raw_data_directory: std::path::PathBuf,
        processed_data_directory: std::path::PathBuf,
    ) -> Self {
        Self {
            protocol_root,
            raw_data_directory,
            processed_data_directory,
            operation_root,
            session_directory,
            source_state_directory: None,
        }
    }

    pub fn protocol_root(&self) -> &std::path::Path { &self.protocol_root }
    pub fn raw_data_directory(&self) -> &std::path::Path { &self.raw_data_directory }
    pub fn processed_data_directory(&self) -> &std::path::Path { &self.processed_data_directory }
    pub fn operation_root(&self) -> &std::path::Path { &self.operation_root }
    pub fn session_directory(&self) -> &std::path::Path { &self.session_directory }
    /// The contract-reserved durable-state directory for an acquisition source
    /// (`get-raw-data/state/`). `None` for processing and for the legacy path.
    pub fn source_state_directory(&self) -> Option<&std::path::Path> {
        self.source_state_directory.as_deref()
    }
}

// ---------------------------------------------------------------------------
// Decoded runtime context
// ---------------------------------------------------------------------------

/// The result of decoding a runtime context document from the environment.
#[derive(Debug)]
pub struct DecodedRuntimeContext {
    pub project: ProjectIdentity,
    pub runtime: OwnedRuntimeIdentity,
    pub session: SessionIdentity,
    pub paths: RuntimeContextPaths,
}

/// Decode and validate the runtime context from the environment.
///
/// The child process calls this after admission to obtain trusted filesystem paths.
/// The `admitted_project`, `admitted_runtime`, and `admitted_session` are compared
/// against the document's identities before the context is constructed.
pub fn decode_runtime_context_from_env(
    admitted_project: &ProjectIdentity,
    admitted_runtime: &OwnedRuntimeIdentity,
    admitted_session: &SessionIdentity,
) -> Result<DecodedRuntimeContext, RuntimeContextError> {
    let raw = std::env::var(RUNTIME_CONTEXT_ENVIRONMENT_VARIABLE)
        .map_err(|_| RuntimeContextError::MissingEnvironmentVariable)?;

    decode_runtime_context(&raw, admitted_project, admitted_runtime, admitted_session)
}

/// Decode and validate a runtime context document string.
pub fn decode_runtime_context(
    json: &str,
    admitted_project: &ProjectIdentity,
    admitted_runtime: &OwnedRuntimeIdentity,
    admitted_session: &SessionIdentity,
) -> Result<DecodedRuntimeContext, RuntimeContextError> {
    let doc: RuntimeContextDocumentV1 = serde_json::from_str(json).map_err(|e| {
        RuntimeContextError::Decoding(RuntimeContextDecodingError::Json(e))
    })?;

    if doc.schema_version != RUNTIME_CONTEXT_SCHEMA_VERSION {
        return Err(RuntimeContextError::Decoding(RuntimeContextDecodingError::Session(
            SessionDecodingError::UnknownSchemaVersion(doc.schema_version),
        )));
    }

    // Compare identities against admitted invocation
    if doc.project.name != admitted_project.name() {
        return Err(RuntimeContextError::IdentityMismatch {
            field: "project",
            expected: admitted_project.name().to_string(),
            actual: doc.project.name,
        });
    }

    if doc.runtime.source != admitted_runtime.source_name() {
        return Err(RuntimeContextError::IdentityMismatch {
            field: "runtime.source",
            expected: admitted_runtime.source_name().to_string(),
            actual: doc.runtime.source,
        });
    }

    if doc.runtime.protocol != admitted_runtime.protocol().identifier() {
        return Err(RuntimeContextError::IdentityMismatch {
            field: "runtime.protocol",
            expected: admitted_runtime.protocol().identifier().to_string(),
            actual: doc.runtime.protocol,
        });
    }

    if doc.runtime.operation != admitted_runtime.operation().identifier() {
        return Err(RuntimeContextError::IdentityMismatch {
            field: "runtime.operation",
            expected: admitted_runtime.operation().identifier().to_string(),
            actual: doc.runtime.operation,
        });
    }

    if doc.runtime.source_contract_version != admitted_runtime.source_contract_version() {
        return Err(RuntimeContextError::IdentityMismatch {
            field: "runtime.source_contract_version",
            expected: admitted_runtime.source_contract_version().to_string(),
            actual: doc.runtime.source_contract_version.to_string(),
        });
    }

    if doc.session.id != admitted_session.id() {
        return Err(RuntimeContextError::IdentityMismatch {
            field: "session",
            expected: admitted_session.id().to_string(),
            actual: doc.session.id,
        });
    }

    // Re-parse identity objects from the document
    let project = ProjectInvocationIdentity::new(&doc.project.name).map_err(|e| {
        RuntimeContextError::Decoding(RuntimeContextDecodingError::Session(SessionDecodingError::InvalidInvariant(
            format!("invalid project name: {e}"),
        )))
    })?;

    let protocol = RuntimeProtocol::from_identifier(&doc.runtime.protocol)
    .map_err(|_| RuntimeContextError::Decoding(RuntimeContextDecodingError::Session(SessionDecodingError::UnknownField {
            field: "protocol",
            value: doc.runtime.protocol.clone(),
    })))?;

    let op = RuntimeOperation::from_identifier(&doc.runtime.operation)
        .map_err(|_| RuntimeContextError::Decoding(RuntimeContextDecodingError::Session(SessionDecodingError::UnknownField {
            field: "operation",
            value: doc.runtime.operation.clone(),
        })))?;

    let runtime = match (protocol, op) {
        (RuntimeProtocol::Http, RuntimeOperation::Acquisition) => {
            OwnedRuntimeIdentity::http_acquisition(&doc.runtime.source, doc.runtime.source_contract_version)
        }
        (RuntimeProtocol::Http, RuntimeOperation::Processing) => {
            OwnedRuntimeIdentity::http_processing(&doc.runtime.source, doc.runtime.source_contract_version)
        }
    };

    let session = SessionInvocationIdentity::new(&doc.session.id).map_err(|e| {
        RuntimeContextError::Decoding(RuntimeContextDecodingError::Session(SessionDecodingError::InvalidInvariant(
            format!("invalid session id: {e}"),
        )))
    })?;

    let project_root = decode_native_path(&doc.project_root)?;
    let protocol_root = decode_native_path(&doc.protocol_root)?;
    let operation_root = decode_native_path(&doc.operation_root)?;
    let session_directory = decode_native_path(&doc.session_directory)?;
    let raw_data_directory = decode_native_path(&doc.raw_data_directory)?;
    let processed_data_directory = decode_native_path(&doc.processed_data_directory)?;
    let source_state_directory = doc
        .source_state_directory
        .as_ref()
        .map(decode_native_path)
        .transpose()?;

    let paths = RuntimeContextPaths::new(
        project_root,
        protocol_root,
        operation_root,
        session_directory,
        raw_data_directory,
        processed_data_directory,
        source_state_directory,
        op,
        &session,
    )
    .map_err(|e| e)?;

    Ok(DecodedRuntimeContext { project, runtime, session, paths })
}

#[cfg(unix)]
fn encode_native_path(path: &Path) -> Result<EncodedNativePathDocument, RuntimeContextError> {
    use std::os::unix::ffi::OsStrExt;
    let bytes = path.as_os_str().as_bytes();
    Ok(EncodedNativePathDocument {
        encoding: "unix-bytes-base64".to_string(),
        value: serde_json::Value::String(base64::encode(bytes)),
    })
}

#[cfg(windows)]
fn encode_native_path(path: &Path) -> Result<EncodedNativePathDocument, RuntimeContextError> {
    use std::os::windows::ffi::OsStrExt;
    let units: Vec<u16> = path.as_os_str().encode_wide().collect();
    let value = serde_json::to_value(units).map_err(|e| {
        RuntimeContextError::Encoding(RuntimeContextEncodingError::Serialization(e))
    })?;
    Ok(EncodedNativePathDocument {
        encoding: "windows-utf16".to_string(),
        value,
    })
}

#[cfg(unix)]
fn decode_native_path(doc: &EncodedNativePathDocument) -> Result<PathBuf, RuntimeContextError> {
    use std::os::unix::ffi::OsStringExt;
    if doc.encoding != "unix-bytes-base64" {
        return Err(RuntimeContextError::Decoding(
            RuntimeContextDecodingError::NativePathEncodingMismatch,
        ));
    }
    let value = doc.value.as_str().ok_or_else(|| {
        RuntimeContextError::Decoding(RuntimeContextDecodingError::Session(
            SessionDecodingError::StructuralDocument("invalid encoded path payload".to_string()),
        ))
    })?;
    let bytes = base64::decode(value).map_err(|_| {
        RuntimeContextError::Decoding(RuntimeContextDecodingError::Session(
            SessionDecodingError::StructuralDocument("invalid encoded path payload".to_string()),
        ))
    })?;
    Ok(PathBuf::from(std::ffi::OsString::from_vec(bytes)))
}

#[cfg(windows)]
fn decode_native_path(doc: &EncodedNativePathDocument) -> Result<PathBuf, RuntimeContextError> {
    use std::os::windows::ffi::OsStringExt;
    if doc.encoding != "windows-utf16" {
        return Err(RuntimeContextError::Decoding(
            RuntimeContextDecodingError::NativePathEncodingMismatch,
        ));
    }
    let units: Vec<u16> = serde_json::from_value(doc.value.clone()).map_err(|e| {
        RuntimeContextError::Decoding(RuntimeContextDecodingError::Json(e))
    })?;
    Ok(PathBuf::from(std::ffi::OsString::from_wide(&units)))
}

#[cfg(test)]
mod source_state_directory_tests {
    use super::*;

    fn project() -> ProjectIdentity {
        ProjectInvocationIdentity::new("example-project").unwrap()
    }

    fn session() -> SessionIdentity {
        SessionInvocationIdentity::new("session-abc").unwrap()
    }

    fn roots() -> (PathBuf, PathBuf) {
        (PathBuf::from("/tmp/example-project"), PathBuf::from("/tmp/example-project/sources/example-source/http"))
    }

    /// Build a valid, fully-consistent `RuntimeContextPaths` for `operation`, with
    /// `source_state_directory` overridden to `override_state_dir` instead of the
    /// operation-correct default, so tests can probe validation independently.
    fn build_paths(
        operation: RuntimeOperation,
        override_state_dir: Option<Option<PathBuf>>,
    ) -> Result<RuntimeContextPaths, RuntimeContextError> {
        let (project_root, protocol_root) = roots();
        let session = session();
        let op_name = match operation {
            RuntimeOperation::Acquisition => "get-raw-data",
            RuntimeOperation::Processing => "process-data",
        };
        let operation_root = protocol_root.join(op_name);
        let session_directory = operation_root.join("sessions").join(session.id());
        let raw_data_directory = protocol_root.join("data/raw");
        let processed_data_directory = protocol_root.join("data/processed");
        let default_state_dir = match operation {
            RuntimeOperation::Acquisition => Some(operation_root.join("state")),
            RuntimeOperation::Processing => None,
        };
        let source_state_directory = override_state_dir.unwrap_or(default_state_dir);

        RuntimeContextPaths::new(
            project_root,
            protocol_root,
            operation_root,
            session_directory,
            raw_data_directory,
            processed_data_directory,
            source_state_directory,
            operation,
            &session,
        )
    }

    #[test]
    fn acquisition_source_state_directory_is_operation_root_join_state() {
        let paths = build_paths(RuntimeOperation::Acquisition, None).expect("valid paths");
        assert_eq!(
            paths.source_state_directory(),
            Some(paths.operation_root().join("state").as_path())
        );
    }

    #[test]
    fn processing_has_no_source_state_directory() {
        let paths = build_paths(RuntimeOperation::Processing, None).expect("valid paths");
        assert_eq!(paths.source_state_directory(), None);
    }

    #[test]
    fn rejects_disagreeing_source_state_directory_for_acquisition() {
        let (_, protocol_root) = roots();
        let wrong = Some(protocol_root.join("get-raw-data").join("not-state"));
        let error = build_paths(RuntimeOperation::Acquisition, Some(wrong)).unwrap_err();
        assert!(matches!(error, RuntimeContextError::SourceStateDirectoryDisagreement));
    }

    #[test]
    fn rejects_missing_source_state_directory_for_acquisition() {
        let error = build_paths(RuntimeOperation::Acquisition, Some(None)).unwrap_err();
        assert!(matches!(error, RuntimeContextError::SourceStateDirectoryDisagreement));
    }

    #[test]
    fn rejects_present_source_state_directory_for_processing() {
        let (_, protocol_root) = roots();
        let present = Some(Some(protocol_root.join("process-data").join("state")));
        let error = build_paths(RuntimeOperation::Processing, present).unwrap_err();
        assert!(matches!(error, RuntimeContextError::SourceStateDirectoryDisagreement));
    }

    #[test]
    fn rejects_traversal_in_source_state_directory() {
        let (project_root, protocol_root) = roots();
        let session = session();
        let operation_root = protocol_root.join("get-raw-data");
        let error = RuntimeContextPaths::new(
            project_root,
            protocol_root.clone(),
            operation_root.clone(),
            operation_root.join("sessions").join(session.id()),
            protocol_root.join("data/raw"),
            protocol_root.join("data/processed"),
            Some(operation_root.join("..").join("state")),
            RuntimeOperation::Acquisition,
            &session,
        )
        .unwrap_err();
        assert!(matches!(
            error,
            RuntimeContextError::PathTraversal { field: "source_state_directory" }
        ));
    }

    #[test]
    fn encode_decode_round_trips_source_state_directory_for_acquisition() {
        let paths = build_paths(RuntimeOperation::Acquisition, None).expect("valid paths");
        let project = project();
        let session = session();
        let runtime = OwnedRuntimeIdentity::http_acquisition("example-source", 1);

        let json = encode_runtime_context(&project, &runtime, &session, &paths).expect("encode");
        let decoded = decode_runtime_context(&json, &project, &runtime, &session).expect("decode");

        assert_eq!(
            decoded.paths.source_state_directory(),
            paths.source_state_directory()
        );
        assert!(decoded.paths.source_state_directory().is_some());
    }

    #[test]
    fn encode_decode_round_trips_absent_source_state_directory_for_processing() {
        let paths = build_paths(RuntimeOperation::Processing, None).expect("valid paths");
        let project = project();
        let session = session();
        let runtime = OwnedRuntimeIdentity::http_processing("example-source", 1);

        let json = encode_runtime_context(&project, &runtime, &session, &paths).expect("encode");
        assert!(
            !json.contains("source_state_directory"),
            "processing envelope must not encode a source_state_directory field: {json}"
        );

        let decoded = decode_runtime_context(&json, &project, &runtime, &session).expect("decode");
        assert_eq!(decoded.paths.source_state_directory(), None);
    }

    #[test]
    fn session_data_paths_from_context_paths_preserves_source_state_directory() {
        let acquisition = build_paths(RuntimeOperation::Acquisition, None).expect("valid paths");
        let processing = build_paths(RuntimeOperation::Processing, None).expect("valid paths");

        assert_eq!(
            SessionDataPaths::from_context_paths(&acquisition).source_state_directory(),
            acquisition.source_state_directory()
        );
        assert_eq!(
            SessionDataPaths::from_context_paths(&processing).source_state_directory(),
            None
        );
    }
}
