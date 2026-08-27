use std::path::{Component, Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::runtime::invocation::{ProjectInvocationIdentity, SessionInvocationIdentity};
use crate::runtime::{OwnedRuntimeIdentity, RuntimeOperation, RuntimeProtocol};
use crate::session::error::{RuntimeContextError, SessionDecodingError};
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
    pub fn new(
        project_root: PathBuf,
        protocol_root: PathBuf,
        operation_root: PathBuf,
        session_directory: PathBuf,
        raw_data_directory: PathBuf,
        processed_data_directory: PathBuf,
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

        Ok(Self {
            project_root,
            protocol_root,
            operation_root,
            session_directory,
            raw_data_directory,
            processed_data_directory,
        })
    }

    pub fn project_root(&self) -> &Path { &self.project_root }
    pub fn protocol_root(&self) -> &Path { &self.protocol_root }
    pub fn operation_root(&self) -> &Path { &self.operation_root }
    pub fn session_directory(&self) -> &Path { &self.session_directory }
    pub fn raw_data_directory(&self) -> &Path { &self.raw_data_directory }
    pub fn processed_data_directory(&self) -> &Path { &self.processed_data_directory }
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
    project_root: String,
    protocol_root: String,
    operation_root: String,
    session_directory: String,
    raw_data_directory: String,
    processed_data_directory: String,
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
        project_root: paths.project_root().display().to_string(),
        protocol_root: paths.protocol_root().display().to_string(),
        operation_root: paths.operation_root().display().to_string(),
        session_directory: paths.session_directory().display().to_string(),
        raw_data_directory: paths.raw_data_directory().display().to_string(),
        processed_data_directory: paths.processed_data_directory().display().to_string(),
    };

    serde_json::to_string(&doc)
        .map_err(|e| RuntimeContextError::Decoding(SessionDecodingError::JsonSyntax(e.to_string())))
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
        RuntimeContextError::Decoding(if e.is_syntax() || e.is_eof() {
            SessionDecodingError::JsonSyntax(e.to_string())
        } else {
            SessionDecodingError::StructuralDocument(e.to_string())
        })
    })?;

    if doc.schema_version != RUNTIME_CONTEXT_SCHEMA_VERSION {
        return Err(RuntimeContextError::Decoding(
            SessionDecodingError::UnknownSchemaVersion(doc.schema_version),
        ));
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
        RuntimeContextError::Decoding(SessionDecodingError::InvalidInvariant(
            format!("invalid project name: {e}"),
        ))
    })?;

    let protocol = RuntimeProtocol::from_identifier(&doc.runtime.protocol)
        .map_err(|_| RuntimeContextError::Decoding(SessionDecodingError::UnknownField {
            field: "protocol",
            value: doc.runtime.protocol.clone(),
        }))?;

    let op = RuntimeOperation::from_identifier(&doc.runtime.operation)
        .map_err(|_| RuntimeContextError::Decoding(SessionDecodingError::UnknownField {
            field: "operation",
            value: doc.runtime.operation.clone(),
        }))?;

    let runtime = match (protocol, op) {
        (RuntimeProtocol::Http, RuntimeOperation::Acquisition) => {
            OwnedRuntimeIdentity::http_acquisition(&doc.runtime.source, doc.runtime.source_contract_version)
        }
        (RuntimeProtocol::Http, RuntimeOperation::Processing) => {
            OwnedRuntimeIdentity::http_processing(&doc.runtime.source, doc.runtime.source_contract_version)
        }
    };

    let session = SessionInvocationIdentity::new(&doc.session.id).map_err(|e| {
        RuntimeContextError::Decoding(SessionDecodingError::InvalidInvariant(
            format!("invalid session id: {e}"),
        ))
    })?;

    let project_root = PathBuf::from(&doc.project_root);
    let protocol_root = PathBuf::from(&doc.protocol_root);
    let operation_root = PathBuf::from(&doc.operation_root);
    let session_directory = PathBuf::from(&doc.session_directory);
    let raw_data_directory = PathBuf::from(&doc.raw_data_directory);
    let processed_data_directory = PathBuf::from(&doc.processed_data_directory);

    let paths = RuntimeContextPaths::new(
        project_root,
        protocol_root,
        operation_root,
        session_directory,
        raw_data_directory,
        processed_data_directory,
        op,
        &session,
    )
    .map_err(|e| e)?;

    Ok(DecodedRuntimeContext { project, runtime, session, paths })
}
