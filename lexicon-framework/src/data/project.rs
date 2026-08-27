use std::env;
use std::fs;
use std::path::{Component, Path, PathBuf};

use crate::data::error::ForegroundDataExecutionError;
use crate::data::request::DataOperation;
use crate::{find_project_root, load_project_config, validate_source_name};

// ---------------------------------------------------------------------------
// RuntimeProjectLayout
// ---------------------------------------------------------------------------

/// Session-independent validated project/source/protocol layout.
///
/// All fields are absolute paths. Structural containment is verified at
/// construction time:
/// ```text
/// sources_root = project_root/<configured-sources-directory>
/// protocol_root = sources_root/<source_name>/http
/// ```
pub struct RuntimeProjectLayout {
    project_root: PathBuf,
    sources_root: PathBuf,
    source_name: String,
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

    /// Absolute HTTP protocol root: `sources_root/<source_name>/http`.
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
/// construct a `RuntimeProjectLayout` for the given source and operation.
pub fn resolve_project_layout(
    source_name: &str,
    operation: DataOperation,
) -> Result<(RuntimeProjectLayout, String), ForegroundDataExecutionError> {
    let cwd = env::current_dir().map_err(|e| {
        ForegroundDataExecutionError::ProjectDiscovery(format!(
            "failed to determine current directory: {e}"
        ))
    })?;

    let project_root =
        find_project_root(&cwd).map_err(ForegroundDataExecutionError::ProjectDiscovery)?;

    let config = load_project_config(&project_root)
        .map_err(ForegroundDataExecutionError::ProjectConfiguration)?;

    validate_sources_root_containment(&project_root, &config.sources_root)?;

    validate_source_name(source_name)
        .map_err(ForegroundDataExecutionError::InvalidSourceIdentity)?;

    let source_dir = config.sources_root.join(source_name);
    if !source_dir.exists() {
        return Err(ForegroundDataExecutionError::MissingSource {
            source_name: source_name.to_owned(),
            path: source_dir,
        });
    }

    let protocol_root = source_dir.join("http");
    if !protocol_root.exists() {
        return Err(ForegroundDataExecutionError::MissingProtocolLayout {
            source_name: source_name.to_owned(),
            path: protocol_root,
        });
    }

    // Validate containment of sources_root within project_root (lexical).
    let layout = build_layout(
        project_root.clone(),
        config.sources_root.clone(),
        source_name.to_owned(),
        protocol_root.clone(),
    )?;

    // Validate operation workspace.
    let op_root = layout.operation_root(operation);
    if !op_root.exists() {
        return Err(ForegroundDataExecutionError::MissingOperationLayout {
            operation: operation.display_name().to_owned(),
            path: op_root,
        });
    }

    // Validate required data directories.
    let raw_dir = layout.raw_data_directory();
    if !raw_dir.exists() {
        return Err(ForegroundDataExecutionError::MissingOperationLayout {
            operation: "data/raw".to_owned(),
            path: raw_dir,
        });
    }
    let processed_dir = layout.processed_data_directory();
    if !processed_dir.exists() {
        return Err(ForegroundDataExecutionError::MissingOperationLayout {
            operation: "data/processed".to_owned(),
            path: processed_dir,
        });
    }

    // Validate runtime bundle directory exists.
    let bundle_dir = layout.bundle_directory(operation);
    if !bundle_dir.exists() {
        return Err(ForegroundDataExecutionError::MissingRuntimeBundle {
            operation: operation.display_name().to_owned(),
            path: bundle_dir,
        });
    }

    let project_name = config.name;
    Ok((layout, project_name))
}

/// Construct the layout and verify lexical containment invariants.
fn build_layout(
    project_root: PathBuf,
    sources_root: PathBuf,
    source_name: String,
    protocol_root: PathBuf,
) -> Result<RuntimeProjectLayout, ForegroundDataExecutionError> {
    // Lexical containment: sources_root must start with project_root.
    if !sources_root.starts_with(&project_root) {
        return Err(ForegroundDataExecutionError::TrustedPathConstruction(
            format!(
                "sources root {} is not contained within project root {}",
                sources_root.display(),
                project_root.display()
            ),
        ));
    }

    // Reject .. traversal in source name components.
    for component in Path::new(&source_name).components() {
        if matches!(component, Component::ParentDir | Component::RootDir | Component::Prefix(_)) {
            return Err(ForegroundDataExecutionError::TrustedPathConstruction(format!(
                "source name '{}' contains path traversal",
                source_name
            )));
        }
    }

    // protocol_root must be sources_root/<source_name>/http.
    let expected_protocol = sources_root.join(&source_name).join("http");
    if protocol_root != expected_protocol {
        return Err(ForegroundDataExecutionError::TrustedPathConstruction(format!(
            "protocol root {} does not equal expected {}",
            protocol_root.display(),
            expected_protocol.display()
        )));
    }

    Ok(RuntimeProjectLayout {
        project_root,
        sources_root,
        source_name,
        protocol_root,
    })
}

/// Verify that the configured sources root is lexically inside the project root.
fn validate_sources_root_containment(
    project_root: &Path,
    sources_root: &Path,
) -> Result<(), ForegroundDataExecutionError> {
    // Use canonicalized paths for reliable containment check.
    let canonical_project = fs::canonicalize(project_root).unwrap_or_else(|_| project_root.to_path_buf());
    let canonical_sources = if sources_root.exists() {
        fs::canonicalize(sources_root).unwrap_or_else(|_| sources_root.to_path_buf())
    } else {
        sources_root.to_path_buf()
    };

    if !canonical_sources.starts_with(&canonical_project) {
        return Err(ForegroundDataExecutionError::ConfiguredSourcesRoot(format!(
            "sources root {} is not contained within project root {}",
            sources_root.display(),
            project_root.display()
        )));
    }

    if sources_root.exists() && !sources_root.is_dir() {
        return Err(ForegroundDataExecutionError::ConfiguredSourcesRoot(format!(
            "sources root {} exists but is not a directory",
            sources_root.display()
        )));
    }

    Ok(())
}
