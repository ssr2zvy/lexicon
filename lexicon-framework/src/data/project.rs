use std::env;
use std::path::{Component, Path, PathBuf};

use crate::data::error::{
    ForegroundDataExecutionError, PathContainmentError, PathKind, ProjectConfigurationError,
    ProjectDiscoveryError, RuntimeProjectLayoutError, SourcesRootValidationError,
};
use crate::data::request::DataOperation;
use crate::{find_project_root, load_project_config, validate_protocol, validate_source_name};

// ---------------------------------------------------------------------------
// RuntimeProjectLayout
// ---------------------------------------------------------------------------

/// Session-independent validated project/source/protocol layout.
///
/// All fields are absolute paths. Structural containment is verified at
/// construction time:
/// ```text
/// sources_root = project_root/<configured-sources-directory>
/// protocol_root = sources_root/<source_name>/<protocol>
/// ```
pub struct RuntimeProjectLayout {
    project_root: PathBuf,
    sources_root: PathBuf,
    source_name: String,
    protocol: String,
    protocol_root: PathBuf,
}

impl RuntimeProjectLayout {
    /// Absolute project root directory.
    pub fn project_root(&self) -> &Path {
        &self.project_root
    }

    /// Absolute sources root directory (`project_root/<configured-sources-directory>`).
    pub fn sources_root(&self) -> &Path {
        &self.sources_root
    }

    /// Validated source name.
    pub fn source_name(&self) -> &str {
        &self.source_name
    }

    /// Validated protocol name.
    pub fn protocol(&self) -> &str {
        &self.protocol
    }

    /// Absolute protocol root: `sources_root/<source_name>/<protocol>`.
    pub fn protocol_root(&self) -> &Path {
        &self.protocol_root
    }

    // -----------------------------------------------------------------------
    // Derived paths
    // -----------------------------------------------------------------------

    /// `protocol_root/data/raw`
    pub fn raw_data_directory(&self) -> PathBuf {
        self.protocol_root.join("data/raw")
    }

    /// `protocol_root/data/processed`
    pub fn processed_data_directory(&self) -> PathBuf {
        self.protocol_root.join("data/processed")
    }

    /// `protocol_root/get-raw-data`
    pub fn acquisition_operation_root(&self) -> PathBuf {
        self.protocol_root.join("get-raw-data")
    }

    /// `protocol_root/process-data`
    pub fn processing_operation_root(&self) -> PathBuf {
        self.protocol_root.join("process-data")
    }

    /// Operation root for the given operation.
    pub fn operation_root(&self, operation: DataOperation) -> PathBuf {
        match operation {
            DataOperation::Acquisition => self.acquisition_operation_root(),
            DataOperation::Processing => self.processing_operation_root(),
        }
    }

    /// `protocol_root/get-raw-data/runtime`
    pub fn acquisition_bundle_directory(&self) -> PathBuf {
        self.protocol_root.join("get-raw-data/runtime")
    }

    /// `protocol_root/process-data/runtime`
    pub fn processing_bundle_directory(&self) -> PathBuf {
        self.protocol_root.join("process-data/runtime")
    }

    /// Runtime bundle directory for the given operation.
    pub fn bundle_directory(&self, operation: DataOperation) -> PathBuf {
        match operation {
            DataOperation::Acquisition => self.acquisition_bundle_directory(),
            DataOperation::Processing => self.processing_bundle_directory(),
        }
    }

    /// Session directory: `operation_root/sessions/<session_id>`.
    pub fn session_directory(&self, operation: DataOperation, session_id: &str) -> PathBuf {
        self.operation_root(operation)
            .join("sessions")
            .join(session_id)
    }
}

// ---------------------------------------------------------------------------
// Discovery and construction
// ---------------------------------------------------------------------------

/// Discover the project, load configuration, validate the source layout, and
/// construct a `RuntimeProjectLayout` for the given source, protocol, and operation.
pub fn resolve_project_layout(
    source_name: &str,
    protocol: &str,
    operation: DataOperation,
) -> Result<(RuntimeProjectLayout, String), ForegroundDataExecutionError> {
    let cwd = env::current_dir().map_err(|e| {
        ForegroundDataExecutionError::ProjectDiscovery(
            ProjectDiscoveryError::CurrentDirectory(e),
        )
    })?;

    let project_root = find_project_root(&cwd)
        .map_err(|error| ForegroundDataExecutionError::ProjectDiscovery(
            ProjectDiscoveryError::FindRoot(error),
        ))?;

    let config = load_project_config(&project_root)
        .map_err(|error| ForegroundDataExecutionError::ProjectConfiguration(
            ProjectConfigurationError::Load(error),
        ))?;

    validate_sources_root_containment(&project_root, &config.sources_root)?;

    validate_source_name(source_name)
        .map_err(|msg| ForegroundDataExecutionError::ProjectLayout(
            RuntimeProjectLayoutError::SourceIdentity(
                lexicon_core::runtime::invocation::RuntimeInvocationValueError::invalid(
                    "source_name",
                    msg,
                ),
            ),
        ))?;

    validate_protocol(protocol)
        .map_err(|msg| ForegroundDataExecutionError::UnsupportedProtocol(msg))?;

    let source_dir = config.sources_root.join(source_name);
    require_directory(&source_dir, PathKind::SourceDirectory).map_err(|e| {
        // Map MissingPath to MissingSource for backward-compatible display.
        match &e {
            RuntimeProjectLayoutError::MissingPath { .. } => {
                ForegroundDataExecutionError::MissingSource {
                    source_name: source_name.to_owned(),
                    path: source_dir.clone(),
                }
            }
            _ => ForegroundDataExecutionError::ProjectLayout(e),
        }
    })?;

    let protocol_root = source_dir.join(protocol);
    require_directory(&protocol_root, PathKind::ProtocolRoot).map_err(|e| {
        match &e {
            RuntimeProjectLayoutError::MissingPath { .. } => {
                ForegroundDataExecutionError::MissingProtocolLayout {
                    source_name: source_name.to_owned(),
                    path: protocol_root.clone(),
                }
            }
            _ => ForegroundDataExecutionError::ProjectLayout(e),
        }
    })?;

    let layout = build_layout(
        project_root.clone(),
        config.sources_root.clone(),
        source_name.to_owned(),
        protocol.to_owned(),
        protocol_root.clone(),
    )?;

    let op_root = layout.operation_root(operation);
    require_directory(&op_root, PathKind::OperationRoot).map_err(|e| {
        match &e {
            RuntimeProjectLayoutError::MissingPath { .. } => {
                ForegroundDataExecutionError::MissingOperationLayout {
                    operation: operation.display_name().to_owned(),
                    path: op_root.clone(),
                }
            }
            _ => ForegroundDataExecutionError::ProjectLayout(e),
        }
    })?;

    let raw_dir = layout.raw_data_directory();
    require_directory(&raw_dir, PathKind::RawDataDirectory).map_err(|e| {
        match &e {
            RuntimeProjectLayoutError::MissingPath { .. } => {
                ForegroundDataExecutionError::MissingOperationLayout {
                    operation: "data/raw".to_owned(),
                    path: raw_dir.clone(),
                }
            }
            _ => ForegroundDataExecutionError::ProjectLayout(e),
        }
    })?;

    let processed_dir = layout.processed_data_directory();
    require_directory(&processed_dir, PathKind::ProcessedDataDirectory).map_err(|e| {
        match &e {
            RuntimeProjectLayoutError::MissingPath { .. } => {
                ForegroundDataExecutionError::MissingOperationLayout {
                    operation: "data/processed".to_owned(),
                    path: processed_dir.clone(),
                }
            }
            _ => ForegroundDataExecutionError::ProjectLayout(e),
        }
    })?;

    let bundle_dir = layout.bundle_directory(operation);
    require_directory(&bundle_dir, PathKind::RuntimeBundleDirectory).map_err(|e| {
        match &e {
            RuntimeProjectLayoutError::MissingPath { .. } => {
                ForegroundDataExecutionError::MissingRuntimeBundle {
                    operation: operation.display_name().to_owned(),
                    path: bundle_dir.clone(),
                }
            }
            _ => ForegroundDataExecutionError::ProjectLayout(e),
        }
    })?;

    let project_name = config.name;
    Ok((layout, project_name))
}

/// Construct the layout and verify lexical containment invariants.
fn build_layout(
    project_root: PathBuf,
    sources_root: PathBuf,
    source_name: String,
    protocol: String,
    protocol_root: PathBuf,
) -> Result<RuntimeProjectLayout, ForegroundDataExecutionError> {
    // Lexical containment: sources_root must start with project_root.
    if !sources_root.starts_with(&project_root) {
        return Err(ForegroundDataExecutionError::ProjectLayout(
            RuntimeProjectLayoutError::PathContainment(
                PathContainmentError::SourcesRootOutsideProject {
                    sources_root,
                    project_root,
                },
            ),
        ));
    }

    // Reject .. traversal in source name components.
    for component in Path::new(&source_name).components() {
        if matches!(component, Component::ParentDir | Component::RootDir | Component::Prefix(_)) {
            return Err(ForegroundDataExecutionError::ProjectLayout(
                RuntimeProjectLayoutError::PathContainment(
                    PathContainmentError::SourceNameTraversal(source_name.clone()),
                ),
            ));
        }
    }

    // protocol_root must be sources_root/<source_name>/<protocol>.
    let expected_protocol = sources_root.join(&source_name).join(&protocol);
    if protocol_root != expected_protocol {
        return Err(ForegroundDataExecutionError::ProjectLayout(
            RuntimeProjectLayoutError::PathContainment(
                PathContainmentError::ProtocolRootMismatch {
                    actual: protocol_root,
                    expected: expected_protocol,
                },
            ),
        ));
    }

    Ok(RuntimeProjectLayout {
        project_root,
        sources_root,
        source_name,
        protocol,
        protocol_root,
    })
}

/// Verify that the configured sources root is lexically inside the project root,
/// and that it is a real directory (not a symlink or regular file).
fn validate_sources_root_containment(
    project_root: &Path,
    sources_root: &Path,
) -> Result<(), ForegroundDataExecutionError> {
    // Use canonicalized paths for reliable containment check.
    let canonical_project =
        std::fs::canonicalize(project_root).unwrap_or_else(|_| project_root.to_path_buf());

    // Check symlink_metadata first; if the path exists it must be a real directory.
    match std::fs::symlink_metadata(sources_root) {
        Ok(meta) => {
            if meta.file_type().is_symlink() {
                return Err(ForegroundDataExecutionError::ProjectLayout(
                    RuntimeProjectLayoutError::SymlinkNotPermitted {
                        path: sources_root.to_path_buf(),
                    },
                ));
            }
            if !meta.is_dir() {
                return Err(ForegroundDataExecutionError::ProjectLayout(
                    RuntimeProjectLayoutError::SourcesRoot(
                        SourcesRootValidationError::NotADirectory(sources_root.to_path_buf()),
                    ),
                ));
            }
            let canonical_sources = std::fs::canonicalize(sources_root)
                .unwrap_or_else(|_| sources_root.to_path_buf());
            if !canonical_sources.starts_with(&canonical_project) {
                return Err(ForegroundDataExecutionError::ProjectLayout(
                    RuntimeProjectLayoutError::SourcesRoot(
                        SourcesRootValidationError::OutsideProjectRoot {
                            sources_root: sources_root.to_path_buf(),
                            project_root: project_root.to_path_buf(),
                        },
                    ),
                ));
            }
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            // Sources root doesn't exist yet; only check lexical containment.
            if !sources_root.starts_with(&canonical_project) {
                return Err(ForegroundDataExecutionError::ProjectLayout(
                    RuntimeProjectLayoutError::SourcesRoot(
                        SourcesRootValidationError::OutsideProjectRoot {
                            sources_root: sources_root.to_path_buf(),
                            project_root: project_root.to_path_buf(),
                        },
                    ),
                ));
            }
        }
        Err(e) => {
            return Err(ForegroundDataExecutionError::ProjectLayout(
                RuntimeProjectLayoutError::SourcesRoot(
                    SourcesRootValidationError::MetadataIo {
                        path: sources_root.to_path_buf(),
                        source: e,
                    },
                ),
            ));
        }
    }

    Ok(())
}

/// Require that `path` is a real directory (not a symlink, not a file, not missing).
///
/// Uses `symlink_metadata` so symlinks are rejected even if they point to directories.
fn require_directory(
    path: &Path,
    kind: PathKind,
) -> Result<(), RuntimeProjectLayoutError> {
    match std::fs::symlink_metadata(path) {
        Ok(meta) => {
            if meta.file_type().is_symlink() {
                return Err(RuntimeProjectLayoutError::SymlinkNotPermitted {
                    path: path.to_path_buf(),
                });
            }
            if !meta.is_dir() {
                return Err(RuntimeProjectLayoutError::NotADirectory {
                    path: path.to_path_buf(),
                    kind,
                });
            }
            Ok(())
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            Err(RuntimeProjectLayoutError::MissingPath {
                path: path.to_path_buf(),
                kind,
            })
        }
        Err(e) => Err(RuntimeProjectLayoutError::MetadataIo {
            path: path.to_path_buf(),
            source: e,
        }),
    }
}
