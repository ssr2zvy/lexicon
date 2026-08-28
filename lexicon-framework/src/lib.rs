use std::env;
use std::fmt;
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::process::Command;

use serde::{Deserialize, Serialize};

pub mod build;
pub mod data;
pub mod publication;
pub mod session;
pub use publication::{
    PublishedRuntimePair, RuntimePairCleanupWarning, RuntimePairPublicationError,
    publish_runtime_pair,
};

const MANAGED_RUNNER_TEMPLATE_VERSION: u32 = 1;
const MAX_MANAGED_RUNNER_ERROR_DISPLAY_BYTES: usize = 4096;

#[derive(Debug)]
pub enum ManagedWorkspaceValidationError {
    MissingManifest(String),
    ManifestParseFailed(String),
    InvalidMembers {
        expected: Vec<String>,
        found: Vec<String>,
    },
    MissingImplementation(String),
    MissingRunner(String),
    MissingLibrarySource(String),
    MissingRunnerSource(String),
    ImplNameMismatch {
        expected: String,
        found: String,
    },
    RunnerNameMismatch {
        expected: String,
        found: String,
    },
    BinaryNameMismatch {
        expected: String,
        found: String,
    },
    InvalidRunnerTemplate(String),
    LegacyLayout(String),
    ExtraWorkspaceMembers(Vec<String>),
}

impl fmt::Display for ManagedWorkspaceValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingManifest(message)
            | Self::ManifestParseFailed(message)
            | Self::InvalidRunnerTemplate(message)
            | Self::LegacyLayout(message) => formatter.write_str(message),
            Self::InvalidMembers { expected, found } => write!(
                formatter,
                "managed workspace has incorrect members: expected {:?}, found {:?}",
                expected, found
            ),
            Self::MissingImplementation(operation) => {
                write!(
                    formatter,
                    "missing managed {operation} implementation manifest"
                )
            }
            Self::MissingRunner(operation) => {
                write!(formatter, "missing managed {operation} runner manifest")
            }
            Self::MissingLibrarySource(operation) => {
                write!(
                    formatter,
                    "missing managed {operation} implementation library"
                )
            }
            Self::MissingRunnerSource(operation) => {
                write!(formatter, "missing managed {operation} runner source")
            }
            Self::ImplNameMismatch { expected, found } => write!(
                formatter,
                "managed implementation package name mismatch: expected '{expected}', found '{found}'"
            ),
            Self::RunnerNameMismatch { expected, found } => write!(
                formatter,
                "managed runner package name mismatch: expected '{expected}', found '{found}'"
            ),
            Self::BinaryNameMismatch { expected, found } => write!(
                formatter,
                "managed runner binary name mismatch: expected '{expected}', found '{found}'"
            ),
            Self::ExtraWorkspaceMembers(members) => write!(
                formatter,
                "managed workspace contains unexpected extra members: {:?}",
                members
            ),
        }
    }
}

impl std::error::Error for ManagedWorkspaceValidationError {}

#[derive(Debug)]
pub enum ManagedWorkspaceMetadataError {
    CommandFailed(String),
    OutputInvalid(String),
    PackageNotFound { name: String },
}

impl fmt::Display for ManagedWorkspaceMetadataError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CommandFailed(message) | Self::OutputInvalid(message) => {
                formatter.write_str(message)
            }
            Self::PackageNotFound { name } => {
                write!(formatter, "workspace metadata package not found: {name}")
            }
        }
    }
}

impl std::error::Error for ManagedWorkspaceMetadataError {}

#[derive(Debug)]
pub enum ManagedRunnerArtifactSelectionError {
    MetadataCommand(String),
    MetadataOutput(String),
    PackageNotFound {
        name: String,
    },
    NoMatchingArtifact {
        package_id: String,
        binary_name: String,
    },
    MultipleMatchingArtifacts {
        package_id: String,
        binary_name: String,
    },
    MissingExecutablePath {
        package_id: String,
        binary_name: String,
    },
    MalformedJsonLine {
        line: String,
    },
}

impl fmt::Display for ManagedRunnerArtifactSelectionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MetadataCommand(message) | Self::MetadataOutput(message) => {
                formatter.write_str(message)
            }
            Self::PackageNotFound { name } => {
                write!(
                    formatter,
                    "workspace package not found for managed runner build: {name}"
                )
            }
            Self::NoMatchingArtifact {
                package_id,
                binary_name,
            } => write!(
                formatter,
                "no matching managed runner artifact for package '{package_id}' binary '{binary_name}'"
            ),
            Self::MultipleMatchingArtifacts {
                package_id,
                binary_name,
            } => write!(
                formatter,
                "multiple managed runner artifacts matched package '{package_id}' binary '{binary_name}'"
            ),
            Self::MissingExecutablePath {
                package_id,
                binary_name,
            } => write!(
                formatter,
                "managed runner artifact for package '{package_id}' binary '{binary_name}' did not include an executable path"
            ),
            Self::MalformedJsonLine { line } => {
                write!(formatter, "malformed cargo JSON line: {line}")
            }
        }
    }
}

impl std::error::Error for ManagedRunnerArtifactSelectionError {}

#[derive(Debug)]
pub enum ManagedRunnerBuildError {
    ArtifactSelection(ManagedRunnerArtifactSelectionError),
    CommandFailed { operation: String, stderr: Vec<u8> },
    ExecutableNotFile(PathBuf),
}

fn managed_runner_stderr_excerpt(stderr: &[u8]) -> String {
    let retained = &stderr[..stderr.len().min(MAX_MANAGED_RUNNER_ERROR_DISPLAY_BYTES)];
    let mut message = String::from_utf8_lossy(retained).into_owned();
    if stderr.len() > MAX_MANAGED_RUNNER_ERROR_DISPLAY_BYTES {
        message.push_str("… [truncated]");
    }
    message
}

impl fmt::Display for ManagedRunnerBuildError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ArtifactSelection(error) => write!(
                formatter,
                "managed runner artifact selection failed: {error}"
            ),
            Self::CommandFailed { operation, stderr } => {
                let rendered = managed_runner_stderr_excerpt(stderr);
                if rendered.trim().is_empty() {
                    write!(formatter, "{operation} managed runner build failed")
                } else {
                    write!(
                        formatter,
                        "{operation} managed runner build failed: {rendered}"
                    )
                }
            }
            Self::ExecutableNotFile(path) => write!(
                formatter,
                "managed runner build did not yield a regular executable file: {}",
                path.display()
            ),
        }
    }
}

impl std::error::Error for ManagedRunnerBuildError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::ArtifactSelection(error) => Some(error),
            Self::CommandFailed { .. } | Self::ExecutableNotFile(_) => None,
        }
    }
}

#[derive(Debug)]
pub enum ManagedSourceBuildError {
    WorkspaceValidation(ManagedWorkspaceValidationError),
    Metadata(ManagedWorkspaceMetadataError),
    CargoBuild(ManagedRunnerBuildError),
    AcquisitionVerification(crate::build::HttpRuntimeVerificationError),
    ProcessingVerification(crate::build::ProcessingRuntimeVerificationError),
    AcquisitionStaging(crate::build::RuntimeBundleStagingError),
    ProcessingStaging(crate::build::ProcessingRuntimeBundleStagingError),
    Publication(crate::publication::RuntimePairPublicationError),
    MissingLockfile(PathBuf),
    LockfileModified(PathBuf),
}

impl fmt::Display for ManagedSourceBuildError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WorkspaceValidation(error) => {
                write!(formatter, "managed workspace validation failed: {error}")
            }
            Self::Metadata(error) => {
                write!(formatter, "managed workspace metadata failed: {error}")
            }
            Self::CargoBuild(error) => write!(formatter, "managed runner build failed: {error}"),
            Self::AcquisitionVerification(error) => {
                write!(formatter, "HTTP runtime verification failed: {error}")
            }
            Self::ProcessingVerification(error) => {
                write!(formatter, "processing runtime verification failed: {error}")
            }
            Self::AcquisitionStaging(error) => {
                write!(formatter, "HTTP bundle staging failed: {error}")
            }
            Self::ProcessingStaging(error) => {
                write!(formatter, "processing bundle staging failed: {error}")
            }
            Self::Publication(error) => {
                write!(formatter, "paired runtime publication failed: {error}")
            }
            Self::MissingLockfile(path) => {
                write!(
                    formatter,
                    "missing Cargo.lock for managed workspace: {}",
                    path.display()
                )
            }
            Self::LockfileModified(path) => {
                write!(
                    formatter,
                    "managed workspace lockfile changed during build: {}",
                    path.display()
                )
            }
        }
    }
}

impl std::error::Error for ManagedSourceBuildError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::WorkspaceValidation(error) => Some(error),
            Self::Metadata(error) => Some(error),
            Self::CargoBuild(error) => Some(error),
            Self::AcquisitionVerification(error) => Some(error),
            Self::ProcessingVerification(error) => Some(error),
            Self::AcquisitionStaging(error) => Some(error),
            Self::ProcessingStaging(error) => Some(error),
            Self::Publication(error) => Some(error),
            Self::MissingLockfile(_) | Self::LockfileModified(_) => None,
        }
    }
}

#[derive(Debug, Deserialize)]
struct CargoMetadataDocument {
    workspace_members: Vec<String>,
    packages: Vec<CargoMetadataPackage>,
}

#[derive(Debug, Deserialize)]
struct CargoMetadataPackage {
    id: String,
    name: String,
    manifest_path: PathBuf,
    targets: Vec<CargoMetadataTarget>,
}

#[derive(Debug, Deserialize)]
struct CargoMetadataTarget {
    name: String,
    kind: Vec<String>,
    src_path: PathBuf,
}

#[derive(Debug, Deserialize)]
struct LexiconProjectConfig {
    schema_version: Option<u32>,
    project: Option<ProjectSection>,
}

#[derive(Debug, Serialize, Deserialize)]
struct SourceTomlDocument {
    schema_version: u32,
    source: SourceTomlSection,
}

#[derive(Debug, Serialize, Deserialize)]
struct SourceTomlSection {
    name: String,
    protocol: String,
}

#[derive(Debug, Deserialize)]
struct ProjectSection {
    name: Option<String>,
    sources_directory: Option<String>,
}

#[derive(Debug)]
pub enum ProjectRootDiscoveryError {
    CurrentDirectoryMetadata {
        path: PathBuf,
        source: std::io::Error,
    },
    ParentTraversal,
    ProjectNotFound,
    NestedProjectConflict {
        outer: PathBuf,
        nested: PathBuf,
    },
}

impl fmt::Display for ProjectRootDiscoveryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CurrentDirectoryMetadata { path, source } => {
                write!(formatter, "failed to read {}: {source}", path.display())
            }
            Self::ParentTraversal => {
                formatter.write_str("failed to traverse project ancestors")
            }
            Self::ProjectNotFound => formatter.write_str(
                "No Lexicon project found. The current directory is not inside a Lexicon project.",
            ),
            Self::NestedProjectConflict { outer, nested } => write!(
                formatter,
                "Nested Lexicon project detected.\nOuter project: {}\nNested project: {}\nMove the nested project outside the outer project, or remove its lexicon.toml if it should belong to the outer project, then rerun.\nNo changes were made.",
                outer.display(),
                nested.display()
            ),
        }
    }
}

impl std::error::Error for ProjectRootDiscoveryError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::CurrentDirectoryMetadata { source, .. } => Some(source),
            _ => None,
        }
    }
}

#[derive(Debug)]
pub enum ProjectConfigLoadError {
    Read {
        path: PathBuf,
        source: std::io::Error,
    },
    DecodeToml {
        path: PathBuf,
        source: toml::de::Error,
    },
    UnsupportedSchemaVersion {
        actual: u32,
    },
    InvalidProjectIdentity(lexicon_core::runtime::invocation::RuntimeInvocationValueError),
    InvalidSourcesDirectory,
    SourcesDirectoryTraversal,
}

impl fmt::Display for ProjectConfigLoadError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Read { path, source } => {
                write!(formatter, "failed to read {}: {source}", path.display())
            }
            Self::DecodeToml { path, source } => {
                write!(formatter, "failed to parse {}: {source}", path.display())
            }
            Self::UnsupportedSchemaVersion { actual } => write!(
                formatter,
                "unsupported schema_version in lexicon.toml: expected 1 but found {actual}"
            ),
            Self::InvalidProjectIdentity(err) => {
                write!(formatter, "invalid project.name in lexicon.toml: {err}")
            }
            Self::InvalidSourcesDirectory => formatter.write_str(
                "invalid sources_directory in lexicon.toml: must be a relative path",
            ),
            Self::SourcesDirectoryTraversal => formatter.write_str(
                "invalid sources_directory in lexicon.toml: must remain within the project root",
            ),
        }
    }
}

impl std::error::Error for ProjectConfigLoadError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Read { source, .. } => Some(source),
            Self::DecodeToml { source, .. } => Some(source),
            Self::InvalidProjectIdentity(err) => Some(err),
            _ => None,
        }
    }
}

pub mod commands {
    use super::*;
    use std::path::{Path, PathBuf};

    #[derive(Debug)]
    pub struct InitResult {
        pub project_directory: PathBuf,
    }

    #[derive(Debug)]
    pub struct SourceCreateResult {
        pub source_name: String,
        pub protocol: String,
        pub protocol_dir: PathBuf,
        pub created_files: Vec<PathBuf>,
    }

    #[derive(Debug)]
    pub struct SourceBuildResult {
        pub source_name: String,
        pub protocol: String,
        pub get_runtime: PathBuf,
        pub process_runtime: PathBuf,
    }

    pub fn init(parent_path: &Path, project_name: &str) -> Result<InitResult, String> {
        let project_directory = initialize_project(parent_path, project_name)?;
        Ok(InitResult { project_directory })
    }

    pub fn source_create(source_name: &str, protocol: &str) -> Result<SourceCreateResult, String> {
        generate_source_scaffold(source_name, protocol)
    }

    pub fn source_build(source_name: &str, protocol: &str) -> Result<SourceBuildResult, String> {
        build_source(source_name, protocol).map_err(|error| error.to_string())
    }
}

fn validate_project_name(project_name: &str) -> Result<(), String> {
    if project_name.trim().is_empty() {
        return Err("project name cannot be empty".to_string());
    }

    if project_name == "." || project_name == ".." {
        return Err(format!(
            "invalid project name '{}': use a simple directory name",
            project_name
        ));
    }

    let path = Path::new(project_name);
    if path.is_absolute()
        || path.components().any(|c| {
            matches!(
                c,
                Component::RootDir | Component::ParentDir | Component::Prefix(_)
            )
        })
    {
        return Err(format!(
            "invalid project name '{}': use a single directory name without separators or parent traversal",
            project_name
        ));
    }

    if path.components().any(|c| matches!(c, Component::CurDir)) {
        return Err(format!(
            "invalid project name '{}': use a single directory name without separators or parent traversal",
            project_name
        ));
    }

    if project_name.contains(['/', '\\']) {
        return Err(format!(
            "invalid project name '{}': use a single directory name without separators or parent traversal",
            project_name
        ));
    }

    Ok(())
}

fn initialize_project(parent_path: &Path, project_name: &str) -> Result<PathBuf, String> {
    validate_project_name(project_name)?;

    if !parent_path.exists() {
        return Err(format!(
            "parent path '{}' does not exist",
            parent_path.display()
        ));
    }
    if !parent_path.is_dir() {
        return Err(format!(
            "parent path '{}' is not a directory",
            parent_path.display()
        ));
    }

    let canonical_parent = parent_path.canonicalize().map_err(|error| {
        format!(
            "failed to canonicalize parent path '{}': {error}",
            parent_path.display()
        )
    })?;

    let mut existing_marker = None;
    for ancestor in canonical_parent.ancestors() {
        let marker = ancestor.join("lexicon.toml");
        if marker.is_file() {
            existing_marker = Some(ancestor.to_path_buf());
            break;
        }
    }

    if let Some(marker_root) = existing_marker {
        return Err(format!(
            "Nested Lexicon project detected.\nOuter project: {}\nNested project: {}\nMove the nested project outside the outer project, or remove its lexicon.toml if it should belong to the outer project, then rerun.\nNo changes were made.",
            marker_root.display(),
            canonical_parent.join(project_name).display()
        ));
    }

    let project_directory = canonical_parent.join(project_name);
    if project_directory.exists() {
        return Err(format!(
            "project '{}' already exists at {}",
            project_name,
            project_directory.display()
        ));
    }

    let staging = tempfile::Builder::new()
        .prefix(&format!(".{project_name}.tmp-"))
        .tempdir_in(&canonical_parent)
        .map_err(|error| format!("failed to create temporary project: {error}"))?;

    fs::create_dir(staging.path().join("sources"))
        .map_err(|error| format!("failed to create sources directory: {error}"))?;

    let config = toml::Value::Table({
        let mut root = toml::map::Map::new();
        root.insert("schema_version".to_string(), toml::Value::Integer(1));

        let mut project = toml::map::Map::new();
        project.insert(
            "name".to_string(),
            toml::Value::String(project_name.to_string()),
        );
        project.insert(
            "sources_directory".to_string(),
            toml::Value::String("sources".to_string()),
        );
        root.insert("project".to_string(), toml::Value::Table(project));

        root
    });

    let toml_text = toml::to_string_pretty(&config)
        .map_err(|error| format!("failed to serialize project config: {error}"))?;

    fs::write(staging.path().join("lexicon.toml"), toml_text)
        .map_err(|error| format!("failed to write lexicon.toml: {error}"))?;

    let staging_path = staging.keep();
    if let Err(error) = fs::rename(&staging_path, &project_directory) {
        let _ = fs::remove_dir_all(&staging_path);
        return Err(format!(
            "failed to finalize project '{}': {error}",
            project_directory.display()
        ));
    }

    Ok(project_directory)
}

fn generate_source_scaffold(
    source_name: &str,
    protocol: &str,
) -> Result<commands::SourceCreateResult, String> {
    validate_source_name(source_name)?;
    validate_protocol(protocol)?;

    let project_root = find_project_root(
        &env::current_dir()
            .map_err(|error| format!("failed to determine current directory: {error}"))?,
    )
    .map_err(|error| error.to_string())?;
    let source_root = configured_sources_directory(&project_root).map_err(|error| error.to_string())?;
    let source_dir = source_root.join(source_name);
    let protocol_dir = source_dir.join(protocol);

    if source_dir.exists() {
        return Err(format!(
            "source '{}' already exists at {}",
            source_name,
            source_dir.display()
        ));
    }

    fs::create_dir_all(&source_root)
        .map_err(|error| format!("failed to create {}: {error}", source_root.display()))?;

    let staging = tempfile::Builder::new()
        .prefix(&format!("{source_name}-"))
        .tempdir_in(&source_root)
        .map_err(|error| {
            format!(
                "failed to create staging directory in {}: {error}",
                source_root.display()
            )
        })?;
    let staging_path = staging.path().to_path_buf();

    let directories = [
        Path::new("http/data/raw"),
        Path::new("http/data/processed"),
        Path::new("http/get-raw-data/sessions"),
        Path::new("http/get-raw-data/get-raw-data-impl/src"),
        Path::new("http/get-raw-data/lexicon-runner/src"),
        Path::new("http/get-raw-data/runtime"),
        Path::new("http/process-data/sessions"),
        Path::new("http/process-data/process-data-impl/src"),
        Path::new("http/process-data/lexicon-runner/src"),
        Path::new("http/process-data/runtime"),
    ];

    for directory in &directories {
        let path = staging_path.join(directory);
        fs::create_dir_all(&path)
            .map_err(|error| format!("failed to create directory {}: {error}", path.display()))?;
    }

    let get_name = format!("{source_name}-get-raw-data");
    let process_name = format!("{source_name}-process-data");
    let get_runner_name = format!("{source_name}-get-raw-data-runner");
    let process_runner_name = format!("{source_name}-process-data-runner");
    let get_binary_name = format!("{source_name}-get-raw-data");
    let process_binary_name = format!("{source_name}-process-data");

    let files = [
        (
            "http/source.toml",
            format_source_toml(source_name, protocol),
        ),
        ("http/discovery.md", format_discovery_markdown(source_name)),
        (
            "http/data/raw/.gitkeep",
            "# generated by lexicon source create\n".to_string(),
        ),
        (
            "http/data/processed/.gitkeep",
            "# generated by lexicon source create\n".to_string(),
        ),
        (
            "http/get-raw-data/Cargo.toml",
            format_workspace_cargo_toml("get-raw-data", &["get-raw-data-impl", "lexicon-runner"]),
        ),
        (
            "http/get-raw-data/get-raw-data-impl/Cargo.toml",
            format_implementation_cargo_toml(&get_name),
        ),
        (
            "http/get-raw-data/get-raw-data-impl/src/lib.rs",
            format_http_implementation_library(source_name),
        ),
        (
            "http/get-raw-data/lexicon-runner/Cargo.toml",
            format_runner_cargo_toml(
                &get_runner_name,
                &get_binary_name,
                &get_name,
                "../get-raw-data-impl",
            ),
        ),
        (
            "http/get-raw-data/lexicon-runner/src/main.rs",
            format_http_managed_runner_main(source_name),
        ),
        (
            "http/get-raw-data/runtime/.gitignore",
            "*\n!.gitignore\n".to_string(),
        ),
        (
            "http/process-data/Cargo.toml",
            format_workspace_cargo_toml("process-data", &["process-data-impl", "lexicon-runner"]),
        ),
        (
            "http/process-data/process-data-impl/Cargo.toml",
            format_implementation_cargo_toml(&process_name),
        ),
        (
            "http/process-data/process-data-impl/src/lib.rs",
            format_processing_implementation_library(source_name),
        ),
        (
            "http/process-data/lexicon-runner/Cargo.toml",
            format_runner_cargo_toml(
                &process_runner_name,
                &process_binary_name,
                &process_name,
                "../process-data-impl",
            ),
        ),
        (
            "http/process-data/lexicon-runner/src/main.rs",
            format_processing_managed_runner_main(source_name),
        ),
        (
            "http/process-data/runtime/.gitignore",
            "*\n!.gitignore\n".to_string(),
        ),
    ];

    for (relative_path, contents) in &files {
        let path = staging_path.join(relative_path);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|error| {
                format!(
                    "failed to create parent directory for {}: {error}",
                    path.display()
                )
            })?;
        }
        fs::write(&path, contents)
            .map_err(|error| format!("failed to write {}: {error}", path.display()))?;
    }

    let get_workspace = staging_path.join("http/get-raw-data");
    let process_workspace = staging_path.join("http/process-data");
    generate_workspace_lockfile(&get_workspace)?;
    generate_workspace_lockfile(&process_workspace)?;

    finalize_source_staging(staging, &source_dir)?;

    let output_files = [
        "http/source.toml",
        "http/discovery.md",
        "http/get-raw-data/Cargo.toml",
        "http/get-raw-data/Cargo.lock",
        "http/get-raw-data/get-raw-data-impl/src/lib.rs",
        "http/get-raw-data/lexicon-runner/src/main.rs",
        "http/process-data/Cargo.toml",
        "http/process-data/Cargo.lock",
        "http/process-data/process-data-impl/src/lib.rs",
        "http/process-data/lexicon-runner/src/main.rs",
    ];
    let created_files: Vec<PathBuf> = output_files.iter().map(|f| source_dir.join(f)).collect();

    Ok(commands::SourceCreateResult {
        source_name: source_name.to_string(),
        protocol: protocol.to_string(),
        protocol_dir,
        created_files,
    })
}

fn build_source(
    source_name: &str,
    protocol: &str,
) -> Result<commands::SourceBuildResult, ManagedSourceBuildError> {
    validate_source_name(source_name).map_err(|error| {
        ManagedSourceBuildError::WorkspaceValidation(ManagedWorkspaceValidationError::LegacyLayout(
            error,
        ))
    })?;
    validate_protocol(protocol).map_err(|error| {
        ManagedSourceBuildError::WorkspaceValidation(ManagedWorkspaceValidationError::LegacyLayout(
            error,
        ))
    })?;

    let current_dir = env::current_dir().map_err(|error| {
        ManagedSourceBuildError::WorkspaceValidation(ManagedWorkspaceValidationError::LegacyLayout(
            format!("failed to determine current directory: {error}"),
        ))
    })?;
    let project_root = find_project_root(&current_dir).map_err(|error| {
        ManagedSourceBuildError::WorkspaceValidation(ManagedWorkspaceValidationError::LegacyLayout(
            error.to_string(),
        ))
    })?;
    let sources_root = configured_sources_directory(&project_root).map_err(|error| {
        ManagedSourceBuildError::WorkspaceValidation(ManagedWorkspaceValidationError::LegacyLayout(
            error.to_string(),
        ))
    })?;
    let source_root = sources_root.join(source_name);
    let protocol_root = source_root.join(protocol);

    if !source_root.is_dir() {
        return Err(ManagedSourceBuildError::WorkspaceValidation(
            ManagedWorkspaceValidationError::LegacyLayout(format!(
                "source '{}' does not exist",
                source_name
            )),
        ));
    }
    if !protocol_root.is_dir() {
        return Err(ManagedSourceBuildError::WorkspaceValidation(
            ManagedWorkspaceValidationError::LegacyLayout(format!(
                "protocol '{}' does not exist for source '{}'",
                protocol, source_name
            )),
        ));
    }

    let source_toml = protocol_root.join("source.toml");
    let _source_doc =
        load_source_metadata(&source_toml, source_name, protocol).map_err(|error| {
            ManagedSourceBuildError::WorkspaceValidation(
                ManagedWorkspaceValidationError::LegacyLayout(error),
            )
        })?;

    let get_workspace = protocol_root.join("get-raw-data");
    let process_workspace = protocol_root.join("process-data");
    let get_workspace_manifest = get_workspace.join("Cargo.toml");
    let process_workspace_manifest = process_workspace.join("Cargo.toml");
    if !get_workspace_manifest.is_file() {
        return Err(ManagedSourceBuildError::WorkspaceValidation(
            ManagedWorkspaceValidationError::MissingManifest(
                "missing managed acquisition workspace manifest".to_owned(),
            ),
        ));
    }
    if !process_workspace_manifest.is_file() {
        return Err(ManagedSourceBuildError::WorkspaceValidation(
            ManagedWorkspaceValidationError::MissingManifest(
                "missing managed processing workspace manifest".to_owned(),
            ),
        ));
    }

    validate_managed_workspace_layout(
        &get_workspace,
        source_name,
        "get-raw-data",
        &format!("{source_name}-get-raw-data"),
        &format!("{source_name}-get-raw-data-runner"),
        &format!("{source_name}-get-raw-data"),
    )
    .map_err(ManagedSourceBuildError::WorkspaceValidation)?;
    validate_managed_workspace_metadata(
        &get_workspace,
        "get-raw-data",
        &format!("{source_name}-get-raw-data"),
        &format!("{source_name}-get-raw-data-runner"),
        &format!("{source_name}-get-raw-data"),
    )
    .map_err(ManagedSourceBuildError::Metadata)?;
    validate_managed_workspace_layout(
        &process_workspace,
        source_name,
        "process-data",
        &format!("{source_name}-process-data"),
        &format!("{source_name}-process-data-runner"),
        &format!("{source_name}-process-data"),
    )
    .map_err(ManagedSourceBuildError::WorkspaceValidation)?;
    validate_managed_workspace_metadata(
        &process_workspace,
        "process-data",
        &format!("{source_name}-process-data"),
        &format!("{source_name}-process-data-runner"),
        &format!("{source_name}-process-data"),
    )
    .map_err(ManagedSourceBuildError::Metadata)?;

    let get_lockfile = get_workspace.join("Cargo.lock");
    let process_lockfile = process_workspace.join("Cargo.lock");
    let get_lockfile_before = read_lockfile_snapshot(&get_lockfile)?;
    let process_lockfile_before = read_lockfile_snapshot(&process_lockfile)?;

    let get_runner = build_managed_runner(
        &get_workspace_manifest,
        &format!("{source_name}-get-raw-data-runner"),
        &format!("{source_name}-get-raw-data"),
        "get-raw-data",
    )?;
    let process_runner = build_managed_runner(
        &process_workspace_manifest,
        &format!("{source_name}-process-data-runner"),
        &format!("{source_name}-process-data"),
        "process-data",
    )?;

    ensure_lockfile_unchanged(&get_lockfile, &get_lockfile_before)?;
    ensure_lockfile_unchanged(&process_lockfile, &process_lockfile_before)?;

    let get_expected_identity = lexicon_core::runtime::OwnedRuntimeIdentity::http_acquisition(
        source_name,
        lexicon_core::protocols::http::HttpSourceContractV1::CONTRACT_VERSION,
    );
    let process_expected_identity = lexicon_core::runtime::OwnedRuntimeIdentity::http_processing(
        source_name,
        lexicon_core::processing::ProcessingSourceContractV1::CONTRACT_VERSION,
    );

    let get_verified = crate::build::verify_http_runtime_candidate_owned(
        get_runner.executable(),
        &get_expected_identity,
    )
    .map_err(ManagedSourceBuildError::AcquisitionVerification)?;
    let process_verified = crate::build::verify_processing_runtime_candidate_owned(
        process_runner.executable(),
        &process_expected_identity,
    )
    .map_err(ManagedSourceBuildError::ProcessingVerification)?;

    let get_runtime_dir = protocol_root.join("get-raw-data/runtime");
    let process_runtime_dir = protocol_root.join("process-data/runtime");
    fs::create_dir_all(&get_runtime_dir).map_err(|error| {
        ManagedSourceBuildError::WorkspaceValidation(ManagedWorkspaceValidationError::LegacyLayout(
            format!("failed to create {}: {error}", get_runtime_dir.display()),
        ))
    })?;
    fs::create_dir_all(&process_runtime_dir).map_err(|error| {
        ManagedSourceBuildError::WorkspaceValidation(ManagedWorkspaceValidationError::LegacyLayout(
            format!(
                "failed to create {}: {error}",
                process_runtime_dir.display()
            ),
        ))
    })?;

    let get_bundle = crate::build::stage_verified_http_runtime_bundle(
        &get_workspace,
        &format!("{source_name}-get-raw-data"),
        &get_verified,
    )
    .map_err(ManagedSourceBuildError::AcquisitionStaging)?;
    let process_bundle = crate::build::stage_verified_processing_runtime_bundle(
        &process_workspace,
        &format!("{source_name}-process-data"),
        &process_verified,
    )
    .map_err(ManagedSourceBuildError::ProcessingStaging)?;

    let published = crate::publication::publish_runtime_pair(
        get_bundle,
        process_bundle,
        &get_runtime_dir,
        &process_runtime_dir,
        get_verified.information().identity(),
        process_verified.information().identity(),
    )
    .map_err(ManagedSourceBuildError::Publication)?;

    Ok(commands::SourceBuildResult {
        source_name: source_name.to_string(),
        protocol: protocol.to_string(),
        get_runtime: published.acquisition_directory().to_path_buf(),
        process_runtime: published.processing_directory().to_path_buf(),
    })
}

fn load_source_metadata(
    path: &Path,
    expected_name: &str,
    expected_protocol: &str,
) -> Result<SourceTomlDocument, String> {
    if !path.is_file() {
        return Err("source metadata does not match the requested source and protocol".to_owned());
    }

    let contents = fs::read_to_string(path)
        .map_err(|error| format!("failed to read {}: {error}", path.display()))?;
    let parsed: SourceTomlDocument = toml::from_str(&contents)
        .map_err(|error| format!("failed to parse {}: {error}", path.display()))?;

    if parsed.schema_version != 1 {
        return Err("unsupported schema version".to_owned());
    }
    if parsed.source.name != expected_name {
        return Err("source metadata does not match the requested source and protocol".to_owned());
    }
    if parsed.source.protocol != expected_protocol {
        return Err("source metadata does not match the requested source and protocol".to_owned());
    }
    Ok(parsed)
}

pub struct BuiltManagedRunner {
    executable: PathBuf,
    #[allow(dead_code)]
    target_directory: tempfile::TempDir,
}

impl BuiltManagedRunner {
    pub fn executable(&self) -> &Path {
        &self.executable
    }
}

fn read_lockfile_snapshot(lockfile: &Path) -> Result<Vec<u8>, ManagedSourceBuildError> {
    if !lockfile.is_file() {
        return Err(ManagedSourceBuildError::MissingLockfile(
            lockfile.to_path_buf(),
        ));
    }
    fs::read(lockfile).map_err(|error| {
        ManagedSourceBuildError::WorkspaceValidation(ManagedWorkspaceValidationError::LegacyLayout(
            format!("failed to read {}: {error}", lockfile.display()),
        ))
    })
}

fn ensure_lockfile_unchanged(
    lockfile: &Path,
    original: &[u8],
) -> Result<(), ManagedSourceBuildError> {
    let current = read_lockfile_snapshot(lockfile)?;
    if current != original {
        return Err(ManagedSourceBuildError::LockfileModified(
            lockfile.to_path_buf(),
        ));
    }
    Ok(())
}

fn load_managed_workspace_metadata(
    workspace_manifest: &Path,
) -> Result<CargoMetadataDocument, ManagedWorkspaceMetadataError> {
    let output = Command::new("cargo")
        .arg("metadata")
        .arg("--manifest-path")
        .arg(workspace_manifest)
        .arg("--locked")
        .arg("--no-deps")
        .arg("--format-version")
        .arg("1")
        .output()
        .map_err(|error| {
            ManagedWorkspaceMetadataError::CommandFailed(format!(
                "failed to run cargo metadata for {}: {error}",
                workspace_manifest.display()
            ))
        })?;

    if !output.status.success() {
        return Err(ManagedWorkspaceMetadataError::CommandFailed(format!(
            "cargo metadata failed for {}: {}",
            workspace_manifest.display(),
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }

    serde_json::from_slice(&output.stdout).map_err(|error| {
        ManagedWorkspaceMetadataError::OutputInvalid(format!(
            "failed to parse cargo metadata for {}: {error}",
            workspace_manifest.display()
        ))
    })
}

fn target_has_kind(target: &CargoMetadataTarget, expected_kind: &str) -> bool {
    target.kind.iter().any(|kind| kind == expected_kind)
}

fn validate_managed_workspace_metadata(
    workspace_root: &Path,
    operation_name: &str,
    expected_impl_name: &str,
    expected_runner_name: &str,
    expected_binary_name: &str,
) -> Result<(), ManagedWorkspaceMetadataError> {
    let metadata = load_managed_workspace_metadata(&workspace_root.join("Cargo.toml"))?;
    if metadata.packages.len() != 2 {
        return Err(ManagedWorkspaceMetadataError::OutputInvalid(format!(
            "managed {operation_name} workspace metadata must contain exactly two packages, found {}",
            metadata.packages.len()
        )));
    }

    let implementation = metadata
        .packages
        .iter()
        .find(|package| package.name == expected_impl_name)
        .ok_or_else(|| ManagedWorkspaceMetadataError::PackageNotFound {
            name: expected_impl_name.to_owned(),
        })?;
    let runner = metadata
        .packages
        .iter()
        .find(|package| package.name == expected_runner_name)
        .ok_or_else(|| ManagedWorkspaceMetadataError::PackageNotFound {
            name: expected_runner_name.to_owned(),
        })?;

    let mut expected_members = vec![implementation.id.clone(), runner.id.clone()];
    expected_members.sort();
    let mut actual_members = metadata.workspace_members;
    actual_members.sort();
    if actual_members != expected_members {
        return Err(ManagedWorkspaceMetadataError::OutputInvalid(format!(
            "managed {operation_name} workspace metadata has incorrect members: expected {:?}, found {:?}",
            expected_members, actual_members
        )));
    }

    let expected_impl_manifest = workspace_root.join(format!("{operation_name}-impl/Cargo.toml"));
    if implementation.manifest_path != expected_impl_manifest {
        return Err(ManagedWorkspaceMetadataError::OutputInvalid(format!(
            "managed {operation_name} implementation manifest path mismatch: expected {}, found {}",
            expected_impl_manifest.display(),
            implementation.manifest_path.display()
        )));
    }
    let expected_runner_manifest = workspace_root.join("lexicon-runner/Cargo.toml");
    if runner.manifest_path != expected_runner_manifest {
        return Err(ManagedWorkspaceMetadataError::OutputInvalid(format!(
            "managed {operation_name} runner manifest path mismatch: expected {}, found {}",
            expected_runner_manifest.display(),
            runner.manifest_path.display()
        )));
    }

    let implementation_lib = implementation
        .targets
        .iter()
        .find(|target| target_has_kind(target, "lib"))
        .ok_or_else(|| {
            ManagedWorkspaceMetadataError::OutputInvalid(format!(
                "managed {operation_name} implementation metadata has no library target"
            ))
        })?;
    if implementation_lib.src_path
        != workspace_root.join(format!("{operation_name}-impl/src/lib.rs"))
    {
        return Err(ManagedWorkspaceMetadataError::OutputInvalid(format!(
            "managed {operation_name} implementation library target path mismatch: expected {}, found {}",
            workspace_root
                .join(format!("{operation_name}-impl/src/lib.rs"))
                .display(),
            implementation_lib.src_path.display()
        )));
    }
    if implementation
        .targets
        .iter()
        .any(|target| target_has_kind(target, "bin"))
    {
        return Err(ManagedWorkspaceMetadataError::OutputInvalid(format!(
            "managed {operation_name} implementation metadata unexpectedly exposes a binary target"
        )));
    }

    let runner_bins: Vec<&CargoMetadataTarget> = runner
        .targets
        .iter()
        .filter(|target| target_has_kind(target, "bin"))
        .collect();
    if runner_bins.len() != 1 {
        return Err(ManagedWorkspaceMetadataError::OutputInvalid(format!(
            "managed {operation_name} runner metadata must expose exactly one binary target, found {}",
            runner_bins.len()
        )));
    }
    let runner_bin = runner_bins[0];
    if runner_bin.name != expected_binary_name {
        return Err(ManagedWorkspaceMetadataError::OutputInvalid(format!(
            "managed {operation_name} runner metadata binary mismatch: expected '{expected_binary_name}', found '{}'",
            runner_bin.name
        )));
    }
    if runner_bin.src_path != workspace_root.join("lexicon-runner/src/main.rs") {
        return Err(ManagedWorkspaceMetadataError::OutputInvalid(format!(
            "managed {operation_name} runner binary target path mismatch: expected {}, found {}",
            workspace_root.join("lexicon-runner/src/main.rs").display(),
            runner_bin.src_path.display()
        )));
    }

    Ok(())
}

fn resolve_managed_package_id(
    workspace_manifest: &Path,
    package_name: &str,
) -> Result<String, ManagedRunnerArtifactSelectionError> {
    let metadata =
        load_managed_workspace_metadata(workspace_manifest).map_err(|error| match error {
            ManagedWorkspaceMetadataError::CommandFailed(message) => {
                ManagedRunnerArtifactSelectionError::MetadataCommand(message)
            }
            ManagedWorkspaceMetadataError::OutputInvalid(message) => {
                ManagedRunnerArtifactSelectionError::MetadataOutput(message)
            }
            ManagedWorkspaceMetadataError::PackageNotFound { name } => {
                ManagedRunnerArtifactSelectionError::PackageNotFound { name }
            }
        })?;

    metadata
        .packages
        .into_iter()
        .find(|package| package.name == package_name)
        .map(|package| package.id)
        .ok_or_else(|| ManagedRunnerArtifactSelectionError::PackageNotFound {
            name: package_name.to_owned(),
        })
}

fn select_artifact_from_cargo_output(
    cargo_output: &str,
    expected_package_id: &str,
    expected_binary_name: &str,
) -> Result<PathBuf, ManagedRunnerArtifactSelectionError> {
    let mut matches = Vec::new();
    let mut missing_executable_path = false;

    for line in cargo_output.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        let value = serde_json::from_str::<serde_json::Value>(trimmed).map_err(|_| {
            ManagedRunnerArtifactSelectionError::MalformedJsonLine {
                line: trimmed.to_owned(),
            }
        })?;
        if value.get("reason").and_then(|item| item.as_str()) != Some("compiler-artifact") {
            continue;
        }
        if value.get("package_id").and_then(|item| item.as_str()) != Some(expected_package_id) {
            continue;
        }

        let target = value.get("target").cloned().unwrap_or_default();
        let is_bin = target
            .get("kind")
            .and_then(|item| item.as_array())
            .is_some_and(|kinds| kinds.iter().any(|item| item.as_str() == Some("bin")));
        if !is_bin {
            continue;
        }
        if target.get("name").and_then(|item| item.as_str()) != Some(expected_binary_name) {
            continue;
        }

        match value.get("executable").and_then(|item| item.as_str()) {
            Some(path) => matches.push(PathBuf::from(path)),
            None => missing_executable_path = true,
        }
    }

    match matches.len() {
        0 if missing_executable_path => {
            Err(ManagedRunnerArtifactSelectionError::MissingExecutablePath {
                package_id: expected_package_id.to_owned(),
                binary_name: expected_binary_name.to_owned(),
            })
        }
        0 => Err(ManagedRunnerArtifactSelectionError::NoMatchingArtifact {
            package_id: expected_package_id.to_owned(),
            binary_name: expected_binary_name.to_owned(),
        }),
        1 => Ok(matches.remove(0)),
        _ => Err(
            ManagedRunnerArtifactSelectionError::MultipleMatchingArtifacts {
                package_id: expected_package_id.to_owned(),
                binary_name: expected_binary_name.to_owned(),
            },
        ),
    }
}

pub fn select_managed_runner_executable(
    cargo_output: &str,
    workspace_manifest: &Path,
    expected_package_name: &str,
    expected_binary_name: &str,
) -> Result<PathBuf, ManagedRunnerArtifactSelectionError> {
    let package_id = resolve_managed_package_id(workspace_manifest, expected_package_name)?;
    select_artifact_from_cargo_output(cargo_output, &package_id, expected_binary_name)
}

pub fn move_to_backup(path: &Path) -> Result<PathBuf, String> {
    let unique = format!(
        ".backup-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
    );
    let backup = path.parent().unwrap().join(unique);
    fs::rename(path, &backup)
        .map_err(|error| format!("failed to create backup for {}: {error}", path.display()))?;
    Ok(backup)
}

fn finalize_source_staging(staging: tempfile::TempDir, source_dir: &Path) -> Result<(), String> {
    let staging_path = staging.path().to_path_buf();
    let source_parent = source_dir.parent().ok_or_else(|| {
        format!(
            "failed to resolve parent directory for {}",
            source_dir.display()
        )
    })?;

    if !source_parent.exists() {
        fs::create_dir_all(source_parent)
            .map_err(|error| format!("failed to create {}: {error}", source_parent.display()))?;
    }

    let rename_result = fs::rename(&staging_path, source_dir);

    if let Err(error) = rename_result {
        let _ = fs::remove_dir_all(&staging_path);
        let _ = fs::remove_dir(source_parent);
        drop(staging);
        return Err(format!(
            "failed to rename {} to {}: {error}",
            staging_path.display(),
            source_dir.display()
        ));
    }

    drop(staging);
    Ok(())
}

pub(crate) fn validate_source_name(source_name: &str) -> Result<(), String> {
    if source_name.trim().is_empty() {
        return Err("source name cannot be empty".to_string());
    }
    if source_name == "." || source_name == ".." {
        return Err(format!(
            "invalid source name '{}': use a simple source identifier",
            source_name
        ));
    }
    if Path::new(source_name).is_absolute() {
        return Err(format!(
            "invalid source name '{}': source names must be relative and not absolute",
            source_name
        ));
    }
    if source_name.contains(['/', '\\'])
        || source_name
            .split(['/', '\\'])
            .any(|part| part.is_empty() || part == "." || part == "..")
    {
        return Err(format!(
            "invalid source name '{}': source names must be a single path segment",
            source_name
        ));
    }
    Ok(())
}

fn validate_protocol(protocol: &str) -> Result<(), String> {
    if protocol.eq_ignore_ascii_case("http") {
        Ok(())
    } else {
        Err(format!(
            "unsupported protocol '{}'; only 'http' is currently supported",
            protocol
        ))
    }
}

pub(crate) fn find_project_root(start_dir: &Path) -> Result<PathBuf, ProjectRootDiscoveryError> {
    let mut current = start_dir.to_path_buf();
    let mut ancestors = Vec::new();

    loop {
        let config_path = current.join("lexicon.toml");
        if config_path.is_file() {
            ancestors.push(current.clone());
        }

        match current.parent() {
            Some(parent) => current = parent.to_path_buf(),
            None => break,
        }
    }

    if ancestors.is_empty() {
        return Err(ProjectRootDiscoveryError::ProjectNotFound);
    }

    if ancestors.len() > 1 {
        let outer = ancestors
            .last()
            .cloned()
            .expect("ancestor list should have outermost root");
        let nested = ancestors
            .first()
            .cloned()
            .expect("ancestor list should at least contain one project root");
        return Err(ProjectRootDiscoveryError::NestedProjectConflict {
            outer,
            nested,
        });
    }

    let root = ancestors[0].clone();
    let descendant = find_descendant_project_root(&root)?;
    if let Some(nested_root) = descendant {
        return Err(ProjectRootDiscoveryError::NestedProjectConflict {
            outer: root.clone(),
            nested: nested_root,
        });
    }

    Ok(root)
}

fn find_descendant_project_root(root: &Path) -> Result<Option<PathBuf>, ProjectRootDiscoveryError> {
    let mut found = None;
    visit_descendants(root, &mut found, root)?;
    Ok(found)
}

// Lexicon project roots are only legal under user-managed project trees. Generated and
// data-heavy directories are pruned before descendant discovery to avoid walking unbounded
// build or cache trees and to keep nested-root detection deterministic.
fn should_prune_descendant_directory(path: &Path) -> bool {
    let Some(name) = path.file_name().and_then(|value| value.to_str()) else {
        return false;
    };

    if matches!(name, ".git" | "target" | "artifacts" | "bundles" | "mza") {
        return true;
    }

    if matches!(name, "raw" | "processed") {
        return path
            .parent()
            .and_then(Path::file_name)
            .and_then(|value| value.to_str())
            == Some("data");
    }

    false
}

fn visit_descendants(
    root: &Path,
    found: &mut Option<PathBuf>,
    current: &Path,
) -> Result<(), ProjectRootDiscoveryError> {
    let mut entries = fs::read_dir(current)
        .map_err(|source| ProjectRootDiscoveryError::CurrentDirectoryMetadata {
            path: current.to_path_buf(),
            source,
        })?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|source| ProjectRootDiscoveryError::CurrentDirectoryMetadata {
            path: current.to_path_buf(),
            source,
        })?;
    entries.sort_by_key(|entry| entry.file_name());

    for entry in entries {
        let path = entry.path();

        if path.is_symlink() {
            continue;
        }

        if should_prune_descendant_directory(&path) {
            continue;
        }

        if path.is_dir() {
            let marker = path.join("lexicon.toml");
            if marker.is_file() && path != root {
                if found.is_none() {
                    *found = Some(path.clone());
                }
                return Ok(());
            }
            visit_descendants(root, found, &path)?;
            if found.is_some() {
                return Ok(());
            }
        }
    }

    Ok(())
}

fn resolve_project_directory(project_root: &Path, configured: &str) -> Result<PathBuf, String> {
    if configured.trim().is_empty() {
        return Err("sources_directory must not be empty".to_string());
    }

    let canonical_root = project_root
        .canonicalize()
        .map_err(|error| format!("failed to resolve project root: {error}"))?;

    let mut resolved = canonical_root.clone();
    for component in Path::new(configured).components() {
        match component {
            Component::Normal(name) => {
                let next = resolved.join(name);
                match fs::symlink_metadata(&next) {
                    Ok(metadata) if metadata.file_type().is_symlink() => {
                        let target = next.canonicalize().map_err(|error| {
                            format!("failed to resolve '{}': {error}", next.display())
                        })?;
                        if !target.starts_with(&canonical_root) {
                            return Err(format!(
                                "sources_directory '{}' escapes the project root",
                                configured
                            ));
                        }
                        resolved = target;
                    }
                    Ok(_) => {
                        resolved = next;
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                        resolved = next;
                    }
                    Err(error) => {
                        return Err(format!("failed to inspect '{}': {error}", next.display()));
                    }
                }
            }
            Component::CurDir => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                return Err(format!(
                    "sources_directory '{}' must be a relative project path",
                    configured
                ));
            }
        }
    }

    if !resolved.starts_with(&canonical_root) {
        return Err(format!(
            "sources_directory '{}' escapes the project root",
            configured
        ));
    }
    if resolved.exists() && !resolved.is_dir() {
        return Err(format!(
            "sources_directory '{}' is not a directory",
            resolved.display()
        ));
    }
    Ok(resolved)
}

fn configured_sources_directory(project_root: &Path) -> Result<PathBuf, ProjectConfigLoadError> {
    load_project_config(project_root).map(|config| config.sources_root)
}

/// Project configuration data loaded from `lexicon.toml`.
pub(crate) struct ProjectConfigData {
    /// The project name from `[project].name`.
    pub name: String,
    /// The absolute, resolved sources root directory.
    pub sources_root: PathBuf,
}

/// Load the project name and resolved sources root from `lexicon.toml`.
///
/// Reuses the existing schema validation, name validation, and path resolution
/// logic from `configured_sources_directory`.
pub(crate) fn load_project_config(
    project_root: &Path,
) -> Result<ProjectConfigData, ProjectConfigLoadError> {
    let config_path = project_root.join("lexicon.toml");
    let contents = fs::read_to_string(&config_path)
        .map_err(|source| ProjectConfigLoadError::Read {
            path: config_path.clone(),
            source,
        })?;
    let parsed: LexiconProjectConfig =
        toml::from_str(&contents).map_err(|source| ProjectConfigLoadError::DecodeToml {
            path: config_path.clone(),
            source,
        })?;

    if parsed.schema_version != Some(1) {
        return Err(ProjectConfigLoadError::UnsupportedSchemaVersion {
            actual: parsed.schema_version.unwrap_or_default(),
        });
    }

    let project = parsed
        .project
        .as_ref();
    let Some(project) = project else {
        return Err(ProjectConfigLoadError::InvalidProjectIdentity(
            lexicon_core::runtime::invocation::RuntimeInvocationValueError::invalid(
                "project.name",
                "missing [project] section",
            ),
        ));
    };
    let project_name = project
        .name
        .as_deref()
        .ok_or_else(|| {
            ProjectConfigLoadError::InvalidProjectIdentity(
                lexicon_core::runtime::invocation::RuntimeInvocationValueError::invalid(
                    "project.name",
                    "missing project.name",
                ),
            )
        })?
        .trim();
    lexicon_core::runtime::invocation::ProjectInvocationIdentity::new(project_name.to_owned())
        .map_err(ProjectConfigLoadError::InvalidProjectIdentity)?;

    let configured = project.sources_directory.as_deref().unwrap_or("sources");

    let path = Path::new(configured);
    if path.is_absolute() {
        return Err(ProjectConfigLoadError::InvalidSourcesDirectory);
    }
    if path.components().any(|c| {
        matches!(
            c,
            Component::ParentDir | Component::RootDir | Component::Prefix(_)
        )
    }) {
        return Err(ProjectConfigLoadError::SourcesDirectoryTraversal);
    }

    let sources_root = resolve_project_directory(project_root, configured)
        .map_err(|_| ProjectConfigLoadError::InvalidSourcesDirectory)?;

    Ok(ProjectConfigData {
        name: project_name.to_owned(),
        sources_root,
    })
}

fn format_source_toml(source_name: &str, protocol: &str) -> String {
    let document = SourceTomlDocument {
        schema_version: 1,
        source: SourceTomlSection {
            name: source_name.to_owned(),
            protocol: protocol.to_owned(),
        },
    };

    toml::to_string_pretty(&document)
        .unwrap_or_else(|error| panic!("failed to serialize source.toml: {error}"))
}

fn format_discovery_markdown(source_name: &str) -> String {
    let mut out = String::new();
    out.push_str(&format!("# {source_name}\n\n"));
    out.push_str("## Source description\n\n");
    out.push_str("Describe the source and the data it produces.\n\n");
    out.push_str("## Discovery method\n\n");
    out.push_str("Document how this source was discovered and why it belongs in this project.\n\n");
    out.push_str("## Acquisition endpoint or location\n\n");
    out.push_str("Record the upstream endpoint, dataset, or location used for acquisition.\n\n");
    out.push_str("## Why HTTP is the correct acquisition protocol\n\n");
    out.push_str("Explain why HTTP is the correct protocol for this source and how it matches the project contract.\n\n");
    out.push_str("## Required authentication or access conditions\n\n");
    out.push_str("List any required credentials, access restrictions, or network constraints.\n\n");
    out.push_str("## Attribution and usage notes\n\n");
    out.push_str("Capture attribution, licensing, and usage guidance for this source.\n\n");
    out.push_str("## Operational observations\n\n");
    out.push_str("Record operational notes, expected cadence, and troubleshooting observations.\n");
    out
}

fn current_lexicon_git_rev() -> Result<String, String> {
    let repo_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .ok_or_else(|| {
            "failed to resolve workspace root for generated dependency pin".to_owned()
        })?;

    let output = Command::new("git")
        .arg("-C")
        .arg(repo_root)
        .arg("rev-parse")
        .arg("HEAD")
        .output()
        .map_err(|error| format!("failed to resolve the repository revision: {error}"))?;

    if !output.status.success() {
        return Err(
            "failed to resolve the repository revision for generated Cargo manifests".to_owned(),
        );
    }

    String::from_utf8(output.stdout)
        .map(|value| value.trim().to_owned())
        .map_err(|error| format!("generated revision is not valid UTF-8: {error}"))
}

fn format_workspace_cargo_toml(operation_name: &str, members: &[&str]) -> String {
    let rev = current_lexicon_git_rev().unwrap_or_else(|error| {
        panic!("failed to resolve the current repository revision for generated manifests: {error}")
    });
    let mut out = String::new();
    out.push_str("[workspace]\n");
    out.push_str("resolver = \"2\"\n");
    out.push_str("members = [\n");
    for member in members {
        out.push_str(&format!("    \"{member}\",\n"));
    }
    out.push_str("]\n\n");
    out.push_str("[workspace.dependencies]\n");
    out.push_str(
        "lexicon_core = { package = \"lexicon-core\", git = \"https://github.com/ssr2zvy/lexicon\", rev = \"",
    );
    out.push_str(&rev);
    out.push_str("\" }\n");
    let _ = operation_name;
    out
}

fn format_implementation_cargo_toml(package_name: &str) -> String {
    let mut out = String::new();
    out.push_str("[package]\n");
    out.push_str(&format!("name = \"{package_name}\"\n"));
    out.push_str("version = \"0.1.0\"\n");
    out.push_str("edition = \"2024\"\n\n");
    out.push_str("[lib]\n");
    out.push_str("path = \"src/lib.rs\"\n\n");
    out.push_str("[dependencies]\n");
    out.push_str("lexicon_core = { workspace = true }\n");
    out
}

fn format_runner_cargo_toml(
    package_name: &str,
    binary_name: &str,
    implementation_package_name: &str,
    implementation_path: &str,
) -> String {
    let mut out = String::new();
    out.push_str("[package]\n");
    out.push_str(&format!("name = \"{package_name}\"\n"));
    out.push_str("version = \"0.1.0\"\n");
    out.push_str("edition = \"2024\"\n\n");
    out.push_str("[[bin]]\n");
    out.push_str(&format!("name = \"{binary_name}\"\n"));
    out.push_str("path = \"src/main.rs\"\n\n");
    out.push_str("[dependencies]\n");
    out.push_str(&format!(
        "source_implementation = {{ package = \"{implementation_package_name}\", path = \"{implementation_path}\" }}\n"
    ));
    out.push_str("lexicon_core = { workspace = true }\n");
    out
}

fn format_http_implementation_library(source_name: &str) -> String {
    let _ = source_name;
    let mut out = String::new();
    out.push_str("use std::ffi::OsString;\n");
    out.push_str("use lexicon_core::http::{\n");
    out.push_str("    AcquisitionResult,\n");
    out.push_str("    HttpAcquisitionContext,\n");
    out.push_str("    HttpSourceContractV1,\n");
    out.push_str("};\n\n");
    out.push_str("pub const SOURCE: HttpSourceContractV1 =\n");
    out.push_str("    HttpSourceContractV1::new(acquire);\n\n");
    out.push_str("pub fn acquire(\n");
    out.push_str("    context: &mut HttpAcquisitionContext,\n");
    out.push_str("    arguments: &[OsString],\n");
    out.push_str(") -> AcquisitionResult<()> {\n");
    out.push_str("    let _ = (context, arguments);\n");
    out.push_str("    todo!(\"implement HTTP acquisition\")\n");
    out.push_str("}\n");
    out
}

fn format_processing_implementation_library(source_name: &str) -> String {
    let _ = source_name;
    let mut out = String::new();
    out.push_str("use std::ffi::OsString;\n");
    out.push_str("use lexicon_core::processing::{\n");
    out.push_str("    ProcessingContext,\n");
    out.push_str("    ProcessingResult,\n");
    out.push_str("    ProcessingSourceContractV1,\n");
    out.push_str("};\n\n");
    out.push_str("pub const SOURCE: ProcessingSourceContractV1 =\n");
    out.push_str("    ProcessingSourceContractV1::new(process);\n\n");
    out.push_str("pub fn process(\n");
    out.push_str("    context: &mut ProcessingContext,\n");
    out.push_str("    arguments: &[OsString],\n");
    out.push_str(") -> ProcessingResult<()> {\n");
    out.push_str("    let _ = arguments;\n");
    out.push_str("    for transaction in context.transactions().iter() {\n");
    out.push_str("        let _ = transaction.transaction();\n");
    out.push_str("        // Inspect admitted metadata and recorded body paths.\n");
    out.push_str("    }\n");
    out.push_str("    let database = context.database();\n");
    out.push_str("    let _ = database;\n");
    out.push_str("    // Source owns schema and SQL.\n");
    out.push_str("    Ok(())\n");
    out.push_str("}\n");
    out
}

fn format_http_managed_runner_main(source_name: &str) -> String {
    let mut out = String::new();
    out.push_str(
        "use std::env;
",
    );
    out.push_str(
        "use std::ffi::OsString;
",
    );
    out.push_str(
        "use std::io::{self, Write};
",
    );
    out.push_str(
        "use std::process::ExitCode;

",
    );
    out.push_str(
        "use lexicon_core::http::{
",
    );
    out.push_str(
        "    HttpCapabilitySet,
",
    );
    out.push_str(
        "    HttpSourceContractV1,
",
    );
    out.push_str(
        "    RuntimeInformationProbeOutcome,
",
    );
    out.push_str(
        "};
",
    );
    out.push_str("use lexicon_core::protocols::http::runner::{run_http_runtime_invocation, try_write_runtime_information_probe};
");
    out.push_str(
        "use lexicon_core::runtime::RuntimeIdentity;

",
    );
    out.push_str(
        "use source_implementation::SOURCE;

",
    );
    out.push_str(&format!(
        "const IDENTITY: RuntimeIdentity = RuntimeIdentity::http_acquisition(\"{source_name}\", HttpSourceContractV1::CONTRACT_VERSION);\n"
    ));
    out.push_str(&format!(
        "const LEXICON_MANAGED_RUNNER_TEMPLATE_VERSION: u32 = {};\n\n",
        MANAGED_RUNNER_TEMPLATE_VERSION
    ));
    out.push_str(
        "fn main() -> ExitCode {
",
    );
    out.push_str(
        "    let arguments: Vec<OsString> = env::args_os().skip(1).collect();
",
    );
    out.push_str(
        "    let stdout = io::stdout();
",
    );
    out.push_str(
        "    let mut stdout = stdout.lock();
",
    );
    out.push_str(
        "    let stderr = io::stderr();
",
    );
    out.push_str(
        "    let mut stderr = stderr.lock();
",
    );
    out.push_str("    match try_write_runtime_information_probe(IDENTITY, &SOURCE, HttpCapabilitySet::empty(), &arguments, &mut stdout) {
");
    out.push_str(
        "        Ok(RuntimeInformationProbeOutcome::Written) => return ExitCode::SUCCESS,
",
    );
    out.push_str(
        "        Ok(RuntimeInformationProbeOutcome::NotRequested) => {}
",
    );
    out.push_str(
        "        Err(error) => {
",
    );
    out.push_str(
        "            let _ = writeln!(stderr, \"[lexicon] ERROR: {error}\");
",
    );
    out.push_str(
        "            return ExitCode::FAILURE;
",
    );
    out.push_str(
        "        }
",
    );
    out.push_str(
        "    }

",
    );
    out.push_str("    if let Err(error) = run_http_runtime_invocation(&arguments, IDENTITY, &SOURCE, HttpCapabilitySet::empty()) {
");
    out.push_str(
        "        let _ = writeln!(stderr, \"[lexicon] ERROR: {error}\");
",
    );
    out.push_str(
        "        return ExitCode::FAILURE;
",
    );
    out.push_str(
        "    }

",
    );
    out.push_str(
        "    ExitCode::SUCCESS
",
    );
    out.push_str(
        "}
",
    );
    out
}

fn format_processing_managed_runner_main(source_name: &str) -> String {
    let mut out = String::new();
    out.push_str(
        "use std::env;
",
    );
    out.push_str(
        "use std::ffi::OsString;
",
    );
    out.push_str(
        "use std::io::{self, Write};
",
    );
    out.push_str(
        "use std::process::ExitCode;

",
    );
    out.push_str(
        "use lexicon_core::processing::{
",
    );
    out.push_str(
        "    ProcessingSourceContractV1,
",
    );
    out.push_str(
        "    ProcessingRuntimeInformationProbeOutcome,
",
    );
    out.push_str(
        "};
",
    );
    out.push_str("use lexicon_core::processing::runner::{run_processing_runtime_invocation, try_write_runtime_information_probe};
");
    out.push_str(
        "use lexicon_core::runtime::RuntimeIdentity;

",
    );
    out.push_str(
        "use source_implementation::SOURCE;

",
    );
    out.push_str(&format!(
        "const IDENTITY: RuntimeIdentity = RuntimeIdentity::http_processing(\"{source_name}\", ProcessingSourceContractV1::CONTRACT_VERSION);\n"
    ));
    out.push_str(&format!(
        "const LEXICON_MANAGED_RUNNER_TEMPLATE_VERSION: u32 = {};\n\n",
        MANAGED_RUNNER_TEMPLATE_VERSION
    ));
    out.push_str(
        "fn main() -> ExitCode {
",
    );
    out.push_str(
        "    let arguments: Vec<OsString> = env::args_os().skip(1).collect();
",
    );
    out.push_str(
        "    let stdout = io::stdout();
",
    );
    out.push_str(
        "    let mut stdout = stdout.lock();
",
    );
    out.push_str(
        "    let stderr = io::stderr();
",
    );
    out.push_str(
        "    let mut stderr = stderr.lock();
",
    );
    out.push_str("    match try_write_runtime_information_probe(IDENTITY, &SOURCE, &arguments, &mut stdout) {
");
    out.push_str(
        "        Ok(ProcessingRuntimeInformationProbeOutcome::Written) => return ExitCode::SUCCESS,
",
    );
    out.push_str(
        "        Ok(ProcessingRuntimeInformationProbeOutcome::NotRequested) => {}
",
    );
    out.push_str(
        "        Err(error) => {
",
    );
    out.push_str(
        "            let _ = writeln!(stderr, \"[lexicon] ERROR: {error}\");
",
    );
    out.push_str(
        "            return ExitCode::FAILURE;
",
    );
    out.push_str(
        "        }
",
    );
    out.push_str(
        "    }

",
    );
    out.push_str("    if let Err(error) = run_processing_runtime_invocation(&arguments, IDENTITY, &SOURCE) {
");
    out.push_str(
        "        let _ = writeln!(stderr, \"[lexicon] ERROR: {error}\");
",
    );
    out.push_str(
        "        return ExitCode::FAILURE;
",
    );
    out.push_str(
        "    }

",
    );
    out.push_str(
        "    ExitCode::SUCCESS
",
    );
    out.push_str(
        "}
",
    );
    out
}

fn generate_workspace_lockfile(workspace_root: &Path) -> Result<(), String> {
    let manifest = workspace_root.join("Cargo.toml");
    if !manifest.is_file() {
        return Err(format!(
            "missing managed workspace manifest at {}",
            manifest.display()
        ));
    }

    let status = Command::new("cargo")
        .arg("generate-lockfile")
        .arg("--manifest-path")
        .arg(&manifest)
        .status()
        .map_err(|error| {
            format!(
                "failed to generate a lockfile for {}: {error}",
                workspace_root.display()
            )
        })?;

    if !status.success() {
        return Err(format!(
            "failed to generate a real Cargo lockfile for {}",
            workspace_root.display()
        ));
    }

    let lockfile = workspace_root.join("Cargo.lock");
    if !lockfile.is_file() {
        return Err(format!(
            "Cargo did not produce a lockfile for {}",
            workspace_root.display()
        ));
    }

    Ok(())
}

fn validate_managed_workspace_layout(
    workspace_root: &Path,
    source_name: &str,
    operation_name: &str,
    expected_impl_name: &str,
    expected_runner_name: &str,
    expected_binary_name: &str,
) -> Result<(), ManagedWorkspaceValidationError> {
    let manifest_path = workspace_root.join("Cargo.toml");
    if !manifest_path.is_file() {
        return Err(ManagedWorkspaceValidationError::MissingManifest(format!(
            "missing managed workspace manifest for {} at {}",
            operation_name,
            workspace_root.display()
        )));
    }

    let lockfile = workspace_root.join("Cargo.lock");
    if !lockfile.is_file() {
        return Err(ManagedWorkspaceValidationError::LegacyLayout(
            "missing Cargo.lock for managed workspace".to_owned(),
        ));
    }

    let contents = fs::read_to_string(&manifest_path).map_err(|error| {
        ManagedWorkspaceValidationError::ManifestParseFailed(format!(
            "failed to read {}: {error}",
            manifest_path.display()
        ))
    })?;
    let parsed: toml::Value = toml::from_str(&contents).map_err(|error| {
        ManagedWorkspaceValidationError::ManifestParseFailed(format!(
            "failed to parse {}: {error}",
            manifest_path.display()
        ))
    })?;

    let members = parsed
        .get("workspace")
        .and_then(|value| value.get("members"))
        .and_then(toml::Value::as_array)
        .ok_or_else(|| ManagedWorkspaceValidationError::InvalidMembers {
            expected: vec![
                format!("{operation_name}-impl"),
                "lexicon-runner".to_string(),
            ],
            found: Vec::new(),
        })?;
    let member_names: Vec<String> = members
        .iter()
        .filter_map(|value| value.as_str().map(str::to_owned))
        .collect();
    let expected_members = vec![
        format!("{operation_name}-impl"),
        "lexicon-runner".to_string(),
    ];
    let extra_members: Vec<String> = member_names
        .iter()
        .filter(|member| !expected_members.contains(member))
        .cloned()
        .collect();
    if !extra_members.is_empty() {
        return Err(ManagedWorkspaceValidationError::ExtraWorkspaceMembers(
            extra_members,
        ));
    }
    if member_names != expected_members {
        return Err(ManagedWorkspaceValidationError::InvalidMembers {
            expected: expected_members,
            found: member_names,
        });
    }

    let impl_manifest = workspace_root.join(format!("{operation_name}-impl/Cargo.toml"));
    let runner_manifest = workspace_root.join("lexicon-runner/Cargo.toml");
    let impl_lib = workspace_root.join(format!("{operation_name}-impl/src/lib.rs"));
    let impl_main = workspace_root.join(format!("{operation_name}-impl/src/main.rs"));
    let runner_main = workspace_root.join("lexicon-runner/src/main.rs");

    if !impl_manifest.is_file() {
        return Err(ManagedWorkspaceValidationError::MissingImplementation(
            operation_name.to_owned(),
        ));
    }
    if !runner_manifest.is_file() {
        return Err(ManagedWorkspaceValidationError::MissingRunner(
            operation_name.to_owned(),
        ));
    }
    if !impl_lib.is_file() {
        return Err(ManagedWorkspaceValidationError::MissingLibrarySource(
            operation_name.to_owned(),
        ));
    }
    if !runner_main.is_file() {
        return Err(ManagedWorkspaceValidationError::MissingRunnerSource(
            operation_name.to_owned(),
        ));
    }
    if impl_main.is_file() {
        return Err(ManagedWorkspaceValidationError::LegacyLayout(format!(
            "managed {} implementation still includes a source-owned main entrypoint at {}",
            operation_name,
            impl_main.display()
        )));
    }

    let impl_doc: toml::Value =
        toml::from_str(&fs::read_to_string(&impl_manifest).map_err(|error| {
            ManagedWorkspaceValidationError::ManifestParseFailed(format!(
                "failed to read {}: {error}",
                impl_manifest.display()
            ))
        })?)
        .map_err(|error| {
            ManagedWorkspaceValidationError::ManifestParseFailed(format!(
                "failed to parse {}: {error}",
                impl_manifest.display()
            ))
        })?;
    let impl_name = impl_doc
        .get("package")
        .and_then(|value| value.get("name"))
        .and_then(toml::Value::as_str)
        .ok_or_else(|| {
            ManagedWorkspaceValidationError::ManifestParseFailed(format!(
                "managed {} implementation manifest has no package name",
                operation_name
            ))
        })?;
    if impl_name != expected_impl_name {
        return Err(ManagedWorkspaceValidationError::ImplNameMismatch {
            expected: expected_impl_name.to_owned(),
            found: impl_name.to_owned(),
        });
    }
    let impl_lib_path = impl_doc
        .get("lib")
        .and_then(|value| value.get("path"))
        .and_then(toml::Value::as_str)
        .ok_or_else(|| {
            ManagedWorkspaceValidationError::ManifestParseFailed(format!(
                "managed {} implementation manifest has no library target path",
                operation_name
            ))
        })?;
    if impl_lib_path != "src/lib.rs" {
        return Err(ManagedWorkspaceValidationError::ManifestParseFailed(
            format!(
                "managed {} implementation library path mismatch: expected 'src/lib.rs', found '{}'",
                operation_name, impl_lib_path
            ),
        ));
    }
    if impl_doc
        .get("bin")
        .and_then(toml::Value::as_array)
        .is_some_and(|items| !items.is_empty())
    {
        return Err(ManagedWorkspaceValidationError::LegacyLayout(format!(
            "managed {} implementation manifest unexpectedly exposes a binary target",
            operation_name
        )));
    }
    let impl_dependencies = impl_doc
        .get("dependencies")
        .and_then(toml::Value::as_table)
        .ok_or_else(|| {
            ManagedWorkspaceValidationError::ManifestParseFailed(format!(
                "managed {} implementation manifest has no dependencies table",
                operation_name
            ))
        })?;
    let impl_lexicon_core = impl_dependencies
        .get("lexicon_core")
        .and_then(toml::Value::as_table)
        .ok_or_else(|| {
            ManagedWorkspaceValidationError::ManifestParseFailed(format!(
                "managed {} implementation manifest must depend on workspace lexicon_core",
                operation_name
            ))
        })?;
    if impl_lexicon_core
        .get("workspace")
        .and_then(toml::Value::as_bool)
        != Some(true)
    {
        return Err(ManagedWorkspaceValidationError::ManifestParseFailed(
            format!(
                "managed {} implementation manifest must resolve lexicon_core through the workspace dependency",
                operation_name
            ),
        ));
    }

    let runner_doc: toml::Value =
        toml::from_str(&fs::read_to_string(&runner_manifest).map_err(|error| {
            ManagedWorkspaceValidationError::ManifestParseFailed(format!(
                "failed to read {}: {error}",
                runner_manifest.display()
            ))
        })?)
        .map_err(|error| {
            ManagedWorkspaceValidationError::ManifestParseFailed(format!(
                "failed to parse {}: {error}",
                runner_manifest.display()
            ))
        })?;
    let runner_name = runner_doc
        .get("package")
        .and_then(|value| value.get("name"))
        .and_then(toml::Value::as_str)
        .ok_or_else(|| {
            ManagedWorkspaceValidationError::ManifestParseFailed(format!(
                "managed {} runner manifest has no package name",
                operation_name
            ))
        })?;
    if runner_name != expected_runner_name {
        return Err(ManagedWorkspaceValidationError::RunnerNameMismatch {
            expected: expected_runner_name.to_owned(),
            found: runner_name.to_owned(),
        });
    }

    let bins = runner_doc
        .get("bin")
        .and_then(|array| array.as_array())
        .ok_or_else(|| {
            ManagedWorkspaceValidationError::ManifestParseFailed(format!(
                "managed {} runner manifest has no binary target",
                operation_name
            ))
        })?;
    if bins.len() != 1 {
        return Err(ManagedWorkspaceValidationError::ManifestParseFailed(
            format!(
                "managed {} runner manifest must define exactly one binary target",
                operation_name
            ),
        ));
    }
    let runner_bin = &bins[0];
    let bin_name = runner_bin
        .get("name")
        .and_then(toml::Value::as_str)
        .ok_or_else(|| {
            ManagedWorkspaceValidationError::ManifestParseFailed(format!(
                "managed {} runner manifest has no binary target name",
                operation_name
            ))
        })?;
    if bin_name != expected_binary_name {
        return Err(ManagedWorkspaceValidationError::BinaryNameMismatch {
            expected: expected_binary_name.to_owned(),
            found: bin_name.to_owned(),
        });
    }
    let runner_bin_path = runner_bin
        .get("path")
        .and_then(toml::Value::as_str)
        .ok_or_else(|| {
            ManagedWorkspaceValidationError::ManifestParseFailed(format!(
                "managed {} runner manifest has no binary target path",
                operation_name
            ))
        })?;
    if runner_bin_path != "src/main.rs" {
        return Err(ManagedWorkspaceValidationError::ManifestParseFailed(
            format!(
                "managed {} runner binary path mismatch: expected 'src/main.rs', found '{}'",
                operation_name, runner_bin_path
            ),
        ));
    }
    let runner_dependencies = runner_doc
        .get("dependencies")
        .and_then(toml::Value::as_table)
        .ok_or_else(|| {
            ManagedWorkspaceValidationError::ManifestParseFailed(format!(
                "managed {} runner manifest has no dependencies table",
                operation_name
            ))
        })?;
    let runner_implementation_dependency = runner_dependencies
        .get("source_implementation")
        .and_then(toml::Value::as_table)
        .ok_or_else(|| {
            ManagedWorkspaceValidationError::ManifestParseFailed(format!(
                "managed {} runner manifest must depend on source_implementation",
                operation_name
            ))
        })?;
    let expected_implementation_path = format!("../{operation_name}-impl");
    if runner_implementation_dependency
        .get("path")
        .and_then(toml::Value::as_str)
        != Some(expected_implementation_path.as_str())
    {
        return Err(ManagedWorkspaceValidationError::ManifestParseFailed(
            format!(
                "managed {} runner manifest must depend on the implementation at '{}'",
                operation_name, expected_implementation_path
            ),
        ));
    }
    let runner_lexicon_core = runner_dependencies
        .get("lexicon_core")
        .and_then(toml::Value::as_table)
        .ok_or_else(|| {
            ManagedWorkspaceValidationError::ManifestParseFailed(format!(
                "managed {} runner manifest must depend on workspace lexicon_core",
                operation_name
            ))
        })?;
    if runner_lexicon_core
        .get("workspace")
        .and_then(toml::Value::as_bool)
        != Some(true)
    {
        return Err(ManagedWorkspaceValidationError::ManifestParseFailed(
            format!(
                "managed {} runner manifest must resolve lexicon_core through the workspace dependency",
                operation_name
            ),
        ));
    }
    let workspace_dependencies = parsed
        .get("workspace")
        .and_then(|value| value.get("dependencies"))
        .and_then(toml::Value::as_table)
        .ok_or_else(|| {
            ManagedWorkspaceValidationError::ManifestParseFailed(format!(
                "managed {} workspace manifest has no workspace dependencies table",
                operation_name
            ))
        })?;
    if !workspace_dependencies.contains_key("lexicon_core") {
        return Err(ManagedWorkspaceValidationError::ManifestParseFailed(
            format!(
                "managed {} workspace manifest must define workspace dependency lexicon_core",
                operation_name
            ),
        ));
    }
    let workspace_lexicon_core = workspace_dependencies
        .get("lexicon_core")
        .and_then(toml::Value::as_table)
        .ok_or_else(|| {
            ManagedWorkspaceValidationError::ManifestParseFailed(format!(
                "managed {} workspace dependency lexicon_core must be a table",
                operation_name
            ))
        })?;
    if workspace_lexicon_core
        .get("package")
        .and_then(toml::Value::as_str)
        != Some("lexicon-core")
    {
        return Err(ManagedWorkspaceValidationError::ManifestParseFailed(
            format!(
                "managed {} workspace dependency lexicon_core must target package 'lexicon-core'",
                operation_name
            ),
        ));
    }

    let runner_src = fs::read_to_string(&runner_main).map_err(|error| {
        ManagedWorkspaceValidationError::ManifestParseFailed(format!(
            "failed to read {}: {error}",
            runner_main.display()
        ))
    })?;
    let expected_template_marker = format!(
        "const LEXICON_MANAGED_RUNNER_TEMPLATE_VERSION: u32 = {};",
        MANAGED_RUNNER_TEMPLATE_VERSION
    );
    if !runner_src.contains("const LEXICON_MANAGED_RUNNER_TEMPLATE_VERSION: u32 =") {
        return Err(ManagedWorkspaceValidationError::InvalidRunnerTemplate(
            format!(
                "managed {} runner is missing the template version marker",
                operation_name
            ),
        ));
    }
    if !runner_src.contains(&expected_template_marker) {
        return Err(ManagedWorkspaceValidationError::InvalidRunnerTemplate(
            format!(
                "managed {} runner template version mismatch: expected {}",
                operation_name, MANAGED_RUNNER_TEMPLATE_VERSION
            ),
        ));
    }
    let expected_runner_src = if operation_name == "get-raw-data" {
        format_http_managed_runner_main(source_name)
    } else {
        format_processing_managed_runner_main(source_name)
    };
    if runner_src != expected_runner_src {
        return Err(ManagedWorkspaceValidationError::InvalidRunnerTemplate(
            format!(
                "managed {} runner template content differs from the managed canonical template",
                operation_name
            ),
        ));
    }

    if impl_manifest.exists()
        && fs::read_to_string(&impl_manifest)
            .map(|text| text.contains("src/main.rs"))
            .unwrap_or(false)
    {
        return Err(ManagedWorkspaceValidationError::LegacyLayout(format!(
            "managed {} implementation manifest still references a source-owned main entrypoint",
            operation_name
        )));
    }
    if runner_implementation_dependency
        .get("package")
        .and_then(toml::Value::as_str)
        != Some(expected_impl_name)
    {
        return Err(ManagedWorkspaceValidationError::ManifestParseFailed(
            format!(
                "managed {} runner manifest must rename source_implementation from package '{}'",
                operation_name, expected_impl_name
            ),
        ));
    }

    Ok(())
}

fn build_managed_runner(
    workspace_manifest: &Path,
    expected_package: &str,
    expected_binary: &str,
    operation_name: &str,
) -> Result<BuiltManagedRunner, ManagedSourceBuildError> {
    let tempdir = tempfile::Builder::new()
        .prefix(&format!("lexicon-{operation_name}-runner-build-"))
        .tempdir()
        .map_err(|error| {
            ManagedSourceBuildError::WorkspaceValidation(
                ManagedWorkspaceValidationError::LegacyLayout(format!(
                    "failed to create temporary build directory: {error}"
                )),
            )
        })?;
    let target_dir = tempdir.path().to_path_buf();

    let output = Command::new("cargo")
        .arg("build")
        .arg("--manifest-path")
        .arg(workspace_manifest)
        .arg("--package")
        .arg(expected_package)
        .arg("--bin")
        .arg(expected_binary)
        .arg("--release")
        .arg("--locked")
        .arg("--message-format=json-render-diagnostics")
        .arg("--target-dir")
        .arg(&target_dir)
        .output()
        .map_err(|error| {
            ManagedSourceBuildError::CargoBuild(ManagedRunnerBuildError::CommandFailed {
                operation: operation_name.to_owned(),
                stderr: format!(
                    "[lexicon] ERROR: source build requires Cargo and a Rust development toolchain: {error}"
                )
                .into_bytes(),
            })
        })?;

    if !output.status.success() {
        return Err(ManagedSourceBuildError::CargoBuild(
            ManagedRunnerBuildError::CommandFailed {
                operation: operation_name.to_owned(),
                stderr: output.stderr,
            },
        ));
    }

    let cargo_json = String::from_utf8_lossy(&output.stdout);
    let artifact = select_managed_runner_executable(
        &cargo_json,
        workspace_manifest,
        expected_package,
        expected_binary,
    )
    .map_err(|error| {
        ManagedSourceBuildError::CargoBuild(ManagedRunnerBuildError::ArtifactSelection(error))
    })?;
    if !artifact.is_file() {
        return Err(ManagedSourceBuildError::CargoBuild(
            ManagedRunnerBuildError::ExecutableNotFile(artifact),
        ));
    }

    Ok(BuiltManagedRunner {
        executable: artifact,
        target_directory: tempdir,
    })
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::sync::Mutex;

    use lexicon_core::runtime::{OwnedRuntimeIdentity, RuntimeOperation, RuntimeProtocol};

    use super::commands::{init, source_create};
    use super::{
        BuiltManagedRunner, MANAGED_RUNNER_TEMPLATE_VERSION, ManagedRunnerArtifactSelectionError,
        ManagedRunnerBuildError, ManagedSourceBuildError, ManagedWorkspaceMetadataError,
        ManagedWorkspaceValidationError, configured_sources_directory, finalize_source_staging,
        find_descendant_project_root, find_project_root, format_http_managed_runner_main,
        format_implementation_cargo_toml, format_processing_managed_runner_main,
        format_runner_cargo_toml, format_source_toml, format_workspace_cargo_toml,
        select_artifact_from_cargo_output, validate_managed_workspace_layout,
        validate_managed_workspace_metadata, validate_protocol, validate_source_name,
    };

    static TEST_CWD_LOCK: Mutex<()> = Mutex::new(());

    fn unique_test_dir(prefix: &str) -> tempfile::TempDir {
        tempfile::Builder::new().prefix(prefix).tempdir().unwrap()
    }

    fn with_test_cwd<T>(project_root: &std::path::Path, func: impl FnOnce() -> T) -> T {
        let _guard = TEST_CWD_LOCK.lock().unwrap();
        let original = std::env::current_dir().unwrap();
        std::env::set_current_dir(project_root).unwrap();
        let result = func();
        std::env::set_current_dir(&original).unwrap();
        result
    }

    fn write_valid_managed_workspace(root: &Path, source_name: &str, operation_name: &str) {
        let impl_name = if operation_name == "get-raw-data" {
            format!("{source_name}-get-raw-data")
        } else {
            format!("{source_name}-process-data")
        };
        let runner_name = if operation_name == "get-raw-data" {
            format!("{source_name}-get-raw-data-runner")
        } else {
            format!("{source_name}-process-data-runner")
        };
        let binary_name = impl_name.clone();
        let impl_dir = root.join(format!("{operation_name}-impl/src"));
        let runner_dir = root.join("lexicon-runner/src");
        fs::create_dir_all(&impl_dir).unwrap();
        fs::create_dir_all(&runner_dir).unwrap();
        fs::write(
            root.join("Cargo.toml"),
            format_workspace_cargo_toml(
                operation_name,
                &[&format!("{operation_name}-impl"), "lexicon-runner"],
            ),
        )
        .unwrap();
        fs::write(root.join("Cargo.lock"), "# lockfile\n").unwrap();
        fs::write(
            root.join(format!("{operation_name}-impl/Cargo.toml")),
            format_implementation_cargo_toml(&impl_name),
        )
        .unwrap();
        fs::write(
            root.join(format!("{operation_name}-impl/src/lib.rs")),
            "pub fn placeholder() {}\n",
        )
        .unwrap();
        fs::write(
            root.join("lexicon-runner/Cargo.toml"),
            format_runner_cargo_toml(
                &runner_name,
                &binary_name,
                &impl_name,
                &format!("../{operation_name}-impl"),
            ),
        )
        .unwrap();
        let main = if operation_name == "get-raw-data" {
            format_http_managed_runner_main(source_name)
        } else {
            format_processing_managed_runner_main(source_name)
        };
        fs::write(root.join("lexicon-runner/src/main.rs"), main).unwrap();
    }

    #[test]
    fn generated_source_toml_matches_required_contract() {
        let source = format_source_toml("example-source", "http");

        assert!(source.contains("schema_version = 1"));
        assert!(source.contains("[source]"));
        assert!(source.contains("name = \"example-source\""));
        assert!(source.contains("protocol = \"http\""));
        assert!(!source.contains("id = \"example-source\""));
    }

    #[test]
    fn validate_source_name_and_protocol_require_safe_values() {
        assert!(validate_source_name("example-source").is_ok());
        assert!(validate_source_name("/bad").is_err());
        assert!(validate_source_name(".").is_err());
        assert!(validate_source_name("..").is_err());
        assert!(validate_protocol("http").is_ok());
        assert!(validate_protocol("browser").is_err());
    }

    #[test]
    fn generated_acquisition_runner_probe_uses_stdout() {
        let source = format_http_managed_runner_main("example-source");

        assert!(source.contains("let stdout = io::stdout();"));
        assert!(source.contains("let mut stdout = stdout.lock();"));
        assert!(source.contains("&mut stdout)"));
    }

    #[test]
    fn generated_acquisition_runner_probe_stderr_is_not_used_for_probe() {
        let source = format_http_managed_runner_main("example-source");

        assert!(!source.contains("&mut stderr)"));
    }

    #[test]
    fn generated_processing_runner_probe_uses_stdout() {
        let source = format_processing_managed_runner_main("example-source");

        assert!(source.contains("let stdout = io::stdout();"));
        assert!(source.contains("let mut stdout = stdout.lock();"));
        assert!(source.contains("&mut stdout)"));
    }

    #[test]
    fn generated_acquisition_runner_includes_template_version_marker() {
        let source = format_http_managed_runner_main("example-source");

        assert!(source.contains(&format!(
            "const LEXICON_MANAGED_RUNNER_TEMPLATE_VERSION: u32 = {};",
            MANAGED_RUNNER_TEMPLATE_VERSION
        )));
    }

    #[test]
    fn generated_processing_runner_includes_template_version_marker() {
        let source = format_processing_managed_runner_main("example-source");

        assert!(source.contains(&format!(
            "const LEXICON_MANAGED_RUNNER_TEMPLATE_VERSION: u32 = {};",
            MANAGED_RUNNER_TEMPLATE_VERSION
        )));
    }

    #[test]
    fn workspace_manifest_uses_exact_lexicon_core_package_name() {
        let manifest =
            format_workspace_cargo_toml("get-raw-data", &["get-raw-data-impl", "lexicon-runner"]);

        assert!(manifest.contains(
            "lexicon_core = { package = \"lexicon-core\", git = \"https://github.com/ssr2zvy/lexicon\", rev = \""
        ));
    }

    #[test]
    fn configured_sources_directory_rejects_symlink_escape() {
        let outside_dir = unique_test_dir("lexicon-sources-outside-");
        let root_dir = unique_test_dir("lexicon-sources-symlink-");
        let root = root_dir.path().to_path_buf();
        let outside = outside_dir.path().to_path_buf();

        let symlink_path = root.join("sources");
        std::os::unix::fs::symlink(&outside, &symlink_path).unwrap();
        fs::write(
            root.join("lexicon.toml"),
            "schema_version = 1\n[project]\nname = \"demo\"\nsources_directory = \"sources\"\n",
        )
        .unwrap();

        let result = configured_sources_directory(&root);
        assert!(result.is_err(), "symlink escape should be rejected");
    }

    #[test]
    fn configured_sources_directory_rejects_escaping_symlink_then_missing_child() {
        let outside_dir = unique_test_dir("lexicon-sources-escape-outside-");
        let root_dir = unique_test_dir("lexicon-sources-escape-child-");
        let root = root_dir.path().to_path_buf();
        let outside = outside_dir.path().to_path_buf();

        let link = root.join("link");
        std::os::unix::fs::symlink(&outside, &link).unwrap();
        fs::write(
            root.join("lexicon.toml"),
            "schema_version = 1\n[project]\nname = \"demo\"\nsources_directory = \"link/nonexistent-child\"\n",
        )
        .unwrap();

        let result = configured_sources_directory(&root);
        assert!(
            result.is_err(),
            "escaped symlink path with missing child should be rejected"
        );
    }

    #[test]
    fn find_project_root_rejects_descendant_nested_project() {
        let root_dir = unique_test_dir("lexicon-nested-root-");
        let root = root_dir.path().to_path_buf();
        let nested = root.join("tools/inner");
        fs::create_dir_all(&nested).unwrap();
        fs::write(
            root.join("lexicon.toml"),
            "schema_version = 1\n[project]\nname = \"outer\"\nsources_directory = \"sources\"\n",
        )
        .unwrap();
        fs::write(
            nested.join("lexicon.toml"),
            "schema_version = 1\n[project]\nname = \"inner\"\nsources_directory = \"sources\"\n",
        )
        .unwrap();

        let result = find_project_root(&root);
        assert!(
            result.is_err(),
            "nested descendant project should be rejected"
        );
        let text = result.unwrap_err().to_string();
        assert!(text.contains("Outer project:"));
        assert!(text.contains("Nested project:"));
    }

    #[test]
    fn find_descendant_project_root_prunes_excluded_directories() {
        let root_dir = unique_test_dir("lexicon-prune-root-");
        let root = root_dir.path().to_path_buf();
        let raw = root.join("data/raw");
        let processed = root.join("data/processed");
        let nested = root.join("data/nested-project");
        fs::create_dir_all(&raw).unwrap();
        fs::create_dir_all(&processed).unwrap();
        fs::create_dir_all(&nested).unwrap();

        fs::write(
            root.join("lexicon.toml"),
            "schema_version = 1\n[project]\nname = \"outer\"\nsources_directory = \"sources\"\n",
        )
        .unwrap();
        fs::write(
            raw.join("lexicon.toml"),
            "schema_version = 1\n[project]\nname = \"raw\"\nsources_directory = \"sources\"\n",
        )
        .unwrap();
        fs::write(
            processed.join("lexicon.toml"),
            "schema_version = 1\n[project]\nname = \"processed\"\nsources_directory = \"sources\"\n",
        )
        .unwrap();
        fs::write(
            nested.join("lexicon.toml"),
            "schema_version = 1\n[project]\nname = \"nested\"\nsources_directory = \"sources\"\n",
        )
        .unwrap();

        let result = find_descendant_project_root(&root).unwrap();
        assert_eq!(
            result,
            Some(nested),
            "data/raw and data/processed must be ignored while a real nested project under data/ is still detected"
        );
    }

    #[test]
    fn generated_discovery_markdown_contains_required_prompts() {
        let markdown = super::format_discovery_markdown("example-source");
        let required = [
            "# example-source",
            "## Source description",
            "Describe the source and the data it produces.",
            "## Discovery method",
            "Document how this source was discovered and why it belongs in this project.",
            "## Acquisition endpoint or location",
            "Record the upstream endpoint, dataset, or location used for acquisition.",
            "## Why HTTP is the correct acquisition protocol",
            "Explain why HTTP is the correct protocol for this source and how it matches the project contract.",
            "## Required authentication or access conditions",
            "List any required credentials, access restrictions, or network constraints.",
            "## Attribution and usage notes",
            "Capture attribution, licensing, and usage guidance for this source.",
            "## Operational observations",
            "Record operational notes, expected cadence, and troubleshooting observations.",
        ];

        for fragment in &required {
            assert!(
                markdown.contains(fragment),
                "discovery.md is missing required prompt: {fragment}\n---\n{markdown}"
            );
        }
    }

    #[test]
    fn built_managed_runner_struct_keeps_executable_accessible() {
        let runner = BuiltManagedRunner {
            executable: PathBuf::from("/tmp/test-binary"),
            target_directory: tempfile::tempdir().unwrap(),
        };

        assert_eq!(runner.executable(), Path::new("/tmp/test-binary"));
        assert!(runner.target_directory.path().is_dir());
    }

    #[test]
    fn select_managed_runner_executable_rejects_no_artifact() {
        let result = select_artifact_from_cargo_output(
            r#"{"reason":"compiler-artifact","package_id":"pkg 0.1.0","target":{"kind":["bin"],"name":"other"},"executable":"/bin/other"}"#,
            "expected 0.1.0",
            "expected-bin",
        );

        assert!(matches!(
            result,
            Err(ManagedRunnerArtifactSelectionError::NoMatchingArtifact { .. })
        ));
    }

    #[test]
    fn select_managed_runner_executable_rejects_multiple_artifacts() {
        let output = r#"{"reason":"compiler-artifact","package_id":"expected 0.1.0","target":{"kind":["bin"],"name":"expected-bin"},"executable":"/bin/one"}
{"reason":"compiler-artifact","package_id":"expected 0.1.0","target":{"kind":["bin"],"name":"expected-bin"},"executable":"/bin/two"}"#;

        let result = select_artifact_from_cargo_output(output, "expected 0.1.0", "expected-bin");

        assert!(matches!(
            result,
            Err(ManagedRunnerArtifactSelectionError::MultipleMatchingArtifacts { .. })
        ));
    }

    #[test]
    fn select_managed_runner_executable_exact_package_id_required() {
        let output = r#"{"reason":"compiler-artifact","package_id":"expected-similar 0.1.0","target":{"kind":["bin"],"name":"expected-bin"},"executable":"/bin/one"}"#;

        let result = select_artifact_from_cargo_output(output, "expected 0.1.0", "expected-bin");

        assert!(matches!(
            result,
            Err(ManagedRunnerArtifactSelectionError::NoMatchingArtifact { .. })
        ));
    }

    #[test]
    fn select_managed_runner_executable_exact_binary_name_required() {
        let output = r#"{"reason":"compiler-artifact","package_id":"expected 0.1.0","target":{"kind":["bin"],"name":"wrong-bin"},"executable":"/bin/one"}"#;

        let result = select_artifact_from_cargo_output(output, "expected 0.1.0", "expected-bin");

        assert!(matches!(
            result,
            Err(ManagedRunnerArtifactSelectionError::NoMatchingArtifact { .. })
        ));
    }

    #[test]
    fn select_managed_runner_executable_rejects_malformed_json_line() {
        let result = select_artifact_from_cargo_output("{", "expected 0.1.0", "expected-bin");

        assert!(matches!(
            result,
            Err(ManagedRunnerArtifactSelectionError::MalformedJsonLine { .. })
        ));
    }

    #[test]
    fn owned_runtime_identity_http_acquisition_has_correct_fields() {
        let identity = OwnedRuntimeIdentity::http_acquisition("test-source", 1);

        assert_eq!(identity.source_name(), "test-source");
        assert_eq!(identity.protocol(), RuntimeProtocol::Http);
        assert_eq!(identity.operation(), RuntimeOperation::Acquisition);
        assert_eq!(identity.source_contract_version(), 1);
    }

    #[test]
    fn owned_runtime_identity_does_not_require_static_lifetime() {
        let source_name = String::from("dynamic-source");
        let identity = OwnedRuntimeIdentity::http_acquisition(source_name.as_str(), 1);

        assert_eq!(identity.source_name(), "dynamic-source");
    }

    #[test]
    fn managed_source_build_error_displays_without_panic() {
        let validation_errors = vec![
            ManagedWorkspaceValidationError::MissingManifest("missing manifest".to_owned()),
            ManagedWorkspaceValidationError::ManifestParseFailed("bad manifest".to_owned()),
            ManagedWorkspaceValidationError::InvalidMembers {
                expected: vec!["a".to_owned()],
                found: vec!["b".to_owned()],
            },
            ManagedWorkspaceValidationError::MissingImplementation("get-raw-data".to_owned()),
            ManagedWorkspaceValidationError::MissingRunner("get-raw-data".to_owned()),
            ManagedWorkspaceValidationError::MissingLibrarySource("get-raw-data".to_owned()),
            ManagedWorkspaceValidationError::MissingRunnerSource("get-raw-data".to_owned()),
            ManagedWorkspaceValidationError::ImplNameMismatch {
                expected: "expected".to_owned(),
                found: "found".to_owned(),
            },
            ManagedWorkspaceValidationError::RunnerNameMismatch {
                expected: "expected".to_owned(),
                found: "found".to_owned(),
            },
            ManagedWorkspaceValidationError::BinaryNameMismatch {
                expected: "expected".to_owned(),
                found: "found".to_owned(),
            },
            ManagedWorkspaceValidationError::InvalidRunnerTemplate("bad template".to_owned()),
            ManagedWorkspaceValidationError::LegacyLayout("legacy".to_owned()),
            ManagedWorkspaceValidationError::ExtraWorkspaceMembers(vec!["extra".to_owned()]),
        ];
        let metadata_errors = vec![
            ManagedWorkspaceMetadataError::CommandFailed("command failed".to_owned()),
            ManagedWorkspaceMetadataError::OutputInvalid("bad output".to_owned()),
            ManagedWorkspaceMetadataError::PackageNotFound {
                name: "pkg".to_owned(),
            },
        ];
        let artifact_errors = vec![
            ManagedRunnerArtifactSelectionError::MetadataCommand("command failed".to_owned()),
            ManagedRunnerArtifactSelectionError::MetadataOutput("bad output".to_owned()),
            ManagedRunnerArtifactSelectionError::PackageNotFound {
                name: "pkg".to_owned(),
            },
            ManagedRunnerArtifactSelectionError::NoMatchingArtifact {
                package_id: "pkg-id".to_owned(),
                binary_name: "bin".to_owned(),
            },
            ManagedRunnerArtifactSelectionError::MultipleMatchingArtifacts {
                package_id: "pkg-id".to_owned(),
                binary_name: "bin".to_owned(),
            },
            ManagedRunnerArtifactSelectionError::MissingExecutablePath {
                package_id: "pkg-id".to_owned(),
                binary_name: "bin".to_owned(),
            },
            ManagedRunnerArtifactSelectionError::MalformedJsonLine {
                line: "{".to_owned(),
            },
        ];
        let build_errors = vec![
            ManagedRunnerBuildError::ArtifactSelection(
                ManagedRunnerArtifactSelectionError::NoMatchingArtifact {
                    package_id: "pkg-id".to_owned(),
                    binary_name: "bin".to_owned(),
                },
            ),
            ManagedRunnerBuildError::CommandFailed {
                operation: "get-raw-data".to_owned(),
                stderr: b"stderr".to_vec(),
            },
            ManagedRunnerBuildError::ExecutableNotFile(PathBuf::from("not-a-file")),
        ];
        let source_errors = vec![
            ManagedSourceBuildError::WorkspaceValidation(
                ManagedWorkspaceValidationError::MissingManifest("missing manifest".to_owned()),
            ),
            ManagedSourceBuildError::Metadata(ManagedWorkspaceMetadataError::CommandFailed(
                "command failed".to_owned(),
            )),
            ManagedSourceBuildError::CargoBuild(ManagedRunnerBuildError::CommandFailed {
                operation: "get-raw-data".to_owned(),
                stderr: b"stderr".to_vec(),
            }),
            ManagedSourceBuildError::AcquisitionVerification(
                crate::build::HttpRuntimeVerificationError::InitialHash(
                    crate::build::RuntimeArtifactHashError::MissingCandidate {
                        path: PathBuf::from("missing-http"),
                    },
                ),
            ),
            ManagedSourceBuildError::ProcessingVerification(
                crate::build::ProcessingRuntimeVerificationError::InitialHash(
                    crate::build::RuntimeArtifactHashError::MissingCandidate {
                        path: PathBuf::from("missing-processing"),
                    },
                ),
            ),
            ManagedSourceBuildError::AcquisitionStaging(
                crate::build::RuntimeBundleStagingError::InvalidStagingParent {
                    path: PathBuf::from("bad-http-parent"),
                },
            ),
            ManagedSourceBuildError::ProcessingStaging(
                crate::build::ProcessingRuntimeBundleStagingError::InvalidStagingParent {
                    path: PathBuf::from("bad-processing-parent"),
                },
            ),
            ManagedSourceBuildError::Publication(
                crate::publication::RuntimePairPublicationError::InvalidDestinations,
            ),
            ManagedSourceBuildError::MissingLockfile(PathBuf::from("Cargo.lock")),
            ManagedSourceBuildError::LockfileModified(PathBuf::from("Cargo.lock")),
        ];

        for rendered in validation_errors
            .into_iter()
            .map(|error| error.to_string())
            .chain(metadata_errors.into_iter().map(|error| error.to_string()))
            .chain(artifact_errors.into_iter().map(|error| error.to_string()))
            .chain(build_errors.into_iter().map(|error| error.to_string()))
            .chain(source_errors.into_iter().map(|error| error.to_string()))
        {
            assert!(!rendered.is_empty());
        }
    }

    #[test]
    fn workspace_validation_rejects_missing_template_version_marker() {
        let root_dir = unique_test_dir("lexicon-workspace-missing-marker-");
        write_valid_managed_workspace(root_dir.path(), "example-source", "get-raw-data");
        let runner_main = root_dir.path().join("lexicon-runner/src/main.rs");
        let marker = format!(
            "const LEXICON_MANAGED_RUNNER_TEMPLATE_VERSION: u32 = {};\n\n",
            MANAGED_RUNNER_TEMPLATE_VERSION
        );
        let source = fs::read_to_string(&runner_main)
            .unwrap()
            .replace(&marker, "");
        fs::write(&runner_main, source).unwrap();

        let result = validate_managed_workspace_layout(
            root_dir.path(),
            "example-source",
            "get-raw-data",
            "example-source-get-raw-data",
            "example-source-get-raw-data-runner",
            "example-source-get-raw-data",
        );

        assert!(matches!(
            result,
            Err(ManagedWorkspaceValidationError::InvalidRunnerTemplate(_))
        ));
    }

    #[test]
    fn workspace_validation_accepts_correct_template_version_marker() {
        let root_dir = unique_test_dir("lexicon-workspace-valid-marker-");
        write_valid_managed_workspace(root_dir.path(), "example-source", "get-raw-data");

        let result = validate_managed_workspace_layout(
            root_dir.path(),
            "example-source",
            "get-raw-data",
            "example-source-get-raw-data",
            "example-source-get-raw-data-runner",
            "example-source-get-raw-data",
        );

        assert!(result.is_ok(), "result: {result:?}");
    }

    #[test]
    fn workspace_validation_rejects_modified_runner_template_content() {
        let root_dir = unique_test_dir("lexicon-workspace-modified-template-");
        write_valid_managed_workspace(root_dir.path(), "example-source", "get-raw-data");
        let runner_main = root_dir.path().join("lexicon-runner/src/main.rs");
        let source = fs::read_to_string(&runner_main)
            .unwrap()
            .replace("ExitCode::SUCCESS", "ExitCode::from(0)");
        fs::write(&runner_main, source).unwrap();

        let result = validate_managed_workspace_layout(
            root_dir.path(),
            "example-source",
            "get-raw-data",
            "example-source-get-raw-data",
            "example-source-get-raw-data-runner",
            "example-source-get-raw-data",
        );

        assert!(matches!(
            result,
            Err(ManagedWorkspaceValidationError::InvalidRunnerTemplate(_))
        ));
    }

    #[test]
    fn workspace_validation_rejects_source_owned_main_entrypoint_file() {
        let root_dir = unique_test_dir("lexicon-workspace-legacy-main-file-");
        write_valid_managed_workspace(root_dir.path(), "example-source", "get-raw-data");
        fs::write(
            root_dir.path().join("get-raw-data-impl/src/main.rs"),
            "fn main() {}\n",
        )
        .unwrap();

        let result = validate_managed_workspace_layout(
            root_dir.path(),
            "example-source",
            "get-raw-data",
            "example-source-get-raw-data",
            "example-source-get-raw-data-runner",
            "example-source-get-raw-data",
        );

        assert!(matches!(
            result,
            Err(ManagedWorkspaceValidationError::LegacyLayout(_))
        ));
    }

    #[test]
    fn workspace_metadata_validation_accepts_valid_workspace() {
        let root_dir = unique_test_dir("lexicon-workspace-metadata-valid-");
        write_valid_managed_workspace(root_dir.path(), "example-source", "get-raw-data");

        let result = validate_managed_workspace_metadata(
            root_dir.path(),
            "get-raw-data",
            "example-source-get-raw-data",
            "example-source-get-raw-data-runner",
            "example-source-get-raw-data",
        );

        assert!(result.is_ok(), "result: {result:?}");
    }

    #[test]
    fn managed_runner_build_error_captures_stderr() {
        let error = ManagedRunnerBuildError::CommandFailed {
            operation: "get-raw-data".to_owned(),
            stderr: b"captured stderr".to_vec(),
        };

        match error {
            ManagedRunnerBuildError::CommandFailed { stderr, .. } => {
                assert_eq!(stderr, b"captured stderr".to_vec());
            }
            other => panic!("unexpected variant: {other:?}"),
        }
    }

    #[test]
    fn managed_runner_build_error_display_truncates_long_stderr() {
        let error = ManagedRunnerBuildError::CommandFailed {
            operation: "get-raw-data".to_owned(),
            stderr: vec![b'x'; 5000],
        };

        let rendered = error.to_string();

        assert!(
            rendered.len() <= 4200,
            "display too long: {}",
            rendered.len()
        );
        assert!(rendered.contains("get-raw-data managed runner build failed"));
    }

    #[test]
    fn framework_init_returns_typed_result_not_exit() {
        let parent_dir = unique_test_dir("lexicon-fw-init-");
        let parent = parent_dir.path().to_path_buf();

        let result = init(&parent, "my-project");
        assert!(result.is_ok());
        let info = result.unwrap();
        assert_eq!(info.project_directory, parent.join("my-project"));
        assert!(info.project_directory.join("lexicon.toml").is_file());
    }

    #[test]
    fn framework_init_fails_with_error_not_exit_for_bad_name() {
        let parent_dir = unique_test_dir("lexicon-fw-init-bad-");
        let parent = parent_dir.path().to_path_buf();

        let result = init(&parent, "../evil");
        assert!(result.is_err(), "bad project name should return Err");
    }

    #[test]
    fn framework_source_create_fails_with_error_not_exit_for_bad_protocol() {
        let temp_dir = unique_test_dir("lexicon-fw-sc-");
        let temp = temp_dir.path().to_path_buf();
        fs::write(
            temp.join("lexicon.toml"),
            "schema_version = 1\n[project]\nname = \"demo\"\nsources_directory = \"sources\"\n",
        )
        .unwrap();

        let result = with_test_cwd(&temp, || source_create("example-source", "browser"));

        assert!(result.is_err(), "unsupported protocol should return Err");
        assert!(
            result.unwrap_err().contains("unsupported protocol"),
            "error must describe the unsupported protocol"
        );
    }

    #[test]
    fn finalize_source_staging_cleans_up_tempdir_when_rename_fails() {
        let root_dir = unique_test_dir("lexicon-stage-cleanup-");
        let root = root_dir.path().to_path_buf();
        let sources_dir = root.join("sources");
        let source_dir = sources_dir.join("example-source");
        fs::create_dir_all(&sources_dir).unwrap();
        fs::write(
            root.join("lexicon.toml"),
            "schema_version = 1\n[project]\nname = \"demo\"\nsources_directory = \"sources\"\n",
        )
        .unwrap();

        let staging = tempfile::Builder::new()
            .prefix("example-source-")
            .tempdir_in(&sources_dir)
            .unwrap();
        let staging_path = staging.path().to_path_buf();
        fs::write(staging_path.join("source.toml"), "schema_version = 1\n").unwrap();
        fs::create_dir_all(&source_dir).unwrap();
        fs::write(source_dir.join("existing.txt"), "preserve-me\n").unwrap();

        let result = finalize_source_staging(staging, &source_dir);

        assert!(
            result.is_err(),
            "rename should fail when the final directory already exists"
        );
        assert!(
            !staging_path.exists(),
            "staging directory must be removed on rename failure"
        );
        assert!(
            source_dir.join("existing.txt").exists(),
            "existing content must remain untouched"
        );
    }
}
