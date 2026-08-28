//! Shared `#[cfg(test)]` fixtures for constructing a real, on-disk project
//! layout, an admitted HTTP runtime bundle, and a `SessionCoordinator`
//! against a temporary directory.
//!
//! This module exists because `select_and_prepare_session`, the background
//! handoff, and the operator-host entrypoint all require a real
//! `RuntimeProjectLayout` (independently re-derived from the process's
//! current working directory) and a real `AdmittedBundle` (independently
//! re-derived from an on-disk manifest and executable, hashed and verified
//! exactly like production admission does). There is no lightweight or mock
//! constructor for either type by design: admission must always go through
//! the same validation path production code uses. This module builds the
//! minimal fixture data required to satisfy that path, once, for reuse
//! across the `session`, `data::session`, and `data::background` test
//! modules.
#![cfg(test)]

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use lexicon_core::protocols::http::HttpSourceContractV1;
use lexicon_core::runtime::OwnedRuntimeIdentity;
use lexicon_core::session::{ProjectIdentity, SessionOperationRoot, SessionStore};

use crate::build::{admit_http_runtime_bundle_owned, hash_runtime_executable};
use crate::data::project::RuntimeProjectLayout;
use crate::data::request::DataOperation;
use crate::data::runtime::{AdmittedAcquisitionBundle, AdmittedBundle};
use crate::session::SessionCoordinator;

/// Serializes tests that change the process-global current working directory.
///
/// `RuntimeProjectLayout` discovery (`resolve_project_layout`) always reads
/// `std::env::current_dir()`; there is no override seam, so tests that need a
/// real layout must temporarily change the process cwd and must not run
/// concurrently with any other test doing the same.
pub(crate) static TEST_CWD_LOCK: Mutex<()> = Mutex::new(());

/// Run `body` with the process cwd temporarily set to `dir`, holding
/// [`TEST_CWD_LOCK`] for the duration.
pub(crate) fn with_test_cwd<T>(dir: &Path, body: impl FnOnce() -> T) -> T {
    let _guard = TEST_CWD_LOCK.lock().unwrap();
    let original = std::env::current_dir().expect("current dir");
    std::env::set_current_dir(dir).expect("set current dir");
    let result = body();
    std::env::set_current_dir(&original).expect("restore current dir");
    result
}

/// A fully scaffolded fake project on disk, ready for `resolve_project_layout`
/// (called with the process cwd set to `project_root`) and `admit_bundle`.
pub(crate) struct FakeProject {
    pub(crate) _root: tempfile::TempDir,
    pub(crate) project_root: PathBuf,
    pub(crate) source_name: String,
}

/// Build a fake project on disk for `source_name`, with a real, correctly
/// hashed, admissible acquisition runtime bundle staged under
/// `get-raw-data/runtime`, and empty (but present) `process-data/runtime`
/// staging so processing-operation layouts resolve too.
///
/// The admitted bundle is always HTTP-acquisition-flavored. Tests that only
/// need a `RuntimeProjectLayout`/`SessionCoordinator` for the `Processing`
/// operation may still pass this fixture's `AdmittedBundle` value to
/// `select_and_prepare_session`: `select_and_prepare_processing` never
/// inspects it.
pub(crate) fn build_fake_project(source_name: &str) -> FakeProject {
    let root = tempfile::tempdir().expect("tempdir");
    let project_root = root.path().to_path_buf();

    fs::write(
        project_root.join("lexicon.toml"),
        "schema_version = 1\n[project]\nname = \"test-project\"\nsources_directory = \"sources\"\n",
    )
    .expect("write lexicon.toml");

    let protocol_root = project_root.join("sources").join(source_name).join("http");
    fs::create_dir_all(protocol_root.join("data").join("raw")).expect("create data/raw");
    fs::create_dir_all(protocol_root.join("data").join("processed")).expect("create data/processed");
    fs::create_dir_all(protocol_root.join("get-raw-data").join("sessions"))
        .expect("create get-raw-data/sessions");
    fs::create_dir_all(protocol_root.join("process-data").join("sessions"))
        .expect("create process-data/sessions");

    let acquisition_bundle_dir = protocol_root.join("get-raw-data").join("runtime");
    let processing_bundle_dir = protocol_root.join("process-data").join("runtime");
    write_fake_http_bundle(&acquisition_bundle_dir, source_name);
    // Processing operation layout resolution only requires the directory to exist;
    // no test in this milestone admits a processing bundle.
    fs::create_dir_all(&processing_bundle_dir).expect("create process-data/runtime");

    FakeProject {
        _root: root,
        project_root,
        source_name: source_name.to_string(),
    }
}

/// Write a real, admissible HTTP acquisition runtime bundle into `bundle_dir`.
fn write_fake_http_bundle(bundle_dir: &Path, source_name: &str) {
    fs::create_dir_all(bundle_dir).expect("create bundle dir");

    let executable_name = "fake-runtime";
    let executable_path = bundle_dir.join(executable_name);
    fs::write(&executable_path, b"fake-executable-bytes-for-tests").expect("write executable");

    let artifact = hash_runtime_executable(&executable_path).expect("hash executable");

    let contract_version = HttpSourceContractV1::CONTRACT_VERSION;
    let runtime_information = serde_json::json!({
        "schema_version": 1,
        "identity": {
            "source": source_name,
            "protocol": "http",
            "operation": "acquisition",
            "source_contract_version": contract_version,
        },
        "descriptor": {
            "contract_version": contract_version,
            "required_capabilities": [],
            "resume_handler_registered": false,
        },
        "runtime": {
            "available_capabilities": [],
        },
    });

    let manifest = serde_json::json!({
        "schema_version": 1,
        "artifact": {
            "executable": executable_name,
            "size": artifact.size(),
            "sha256": artifact.sha256(),
        },
        "runtime_information": runtime_information,
    });

    // The real admission path's manifest-boundary validation
    // (`validate_manifest_text` in `runtime_bundle_admission.rs`) requires exactly one
    // trailing `\n`, no `\r`, and no leading/trailing whitespace inside the payload.
    // `serde_json::to_vec` alone produces no trailing newline at all, which
    // `validate_manifest_text` rejects as `InvalidBoundary`.
    let mut manifest_bytes = serde_json::to_vec(&manifest).expect("serialize manifest");
    manifest_bytes.push(b'\n');
    fs::write(bundle_dir.join("runtime.json"), manifest_bytes).expect("write runtime.json");
}

/// Admit the fixture's acquisition bundle, matching production `admit_bundle`.
pub(crate) fn admit_fake_bundle(project: &FakeProject) -> AdmittedBundle {
    let layout = build_layout_for(project);
    let bundle_dir = layout.acquisition_bundle_directory();
    let expected_identity =
        OwnedRuntimeIdentity::http_acquisition(&project.source_name, HttpSourceContractV1::CONTRACT_VERSION);
    let bundle = admit_http_runtime_bundle_owned(&bundle_dir, &expected_identity)
        .expect("admit fake bundle");
    AdmittedBundle::Acquisition(AdmittedAcquisitionBundle {
        bundle,
        identity: expected_identity,
    })
}

/// Resolve a `RuntimeProjectLayout` for the fixture without going through
/// `resolve_project_layout` (and therefore without needing to hold
/// [`TEST_CWD_LOCK`]), for callers that only need the layout's derived paths.
fn build_layout_for(project: &FakeProject) -> RuntimeProjectLayout {
    let previous = with_test_cwd(&project.project_root, || {
        crate::data::project::resolve_project_layout(&project.source_name, DataOperation::Acquisition)
    });
    previous.expect("resolve fake project layout").0
}

/// Build a `SessionCoordinator` for `operation` against the fixture project.
pub(crate) fn build_fake_coordinator(
    project: &FakeProject,
    operation: DataOperation,
) -> SessionCoordinator {
    let layout = build_layout_for(project);
    let project_identity = ProjectIdentity::new("test-project").expect("project identity");
    let runtime_identity = match operation {
        DataOperation::Acquisition => {
            OwnedRuntimeIdentity::http_acquisition(&project.source_name, HttpSourceContractV1::CONTRACT_VERSION)
        }
        DataOperation::Processing => OwnedRuntimeIdentity::http_processing(
            &project.source_name,
            lexicon_core::processing::ProcessingSourceContractV1::CONTRACT_VERSION,
        ),
    };
    let session_operation = crate::data::session::data_operation_to_session_operation(operation);
    let operation_root_path = layout.operation_root(operation);
    let operation_root = SessionOperationRoot::new(operation_root_path).expect("operation root");

    SessionCoordinator::new(
        project_identity,
        runtime_identity,
        session_operation,
        operation_root,
        layout.project_root().to_path_buf(),
        layout.protocol_root().to_path_buf(),
    )
    .expect("build fake coordinator")
}

/// Open the raw `SessionStore` backing `coordinator`'s operation, for
/// assertions that need direct store access (e.g. lease inspection).
pub(crate) fn open_store(project: &FakeProject, operation: DataOperation) -> SessionStore {
    let layout = build_layout_for(project);
    let operation_root =
        SessionOperationRoot::new(layout.operation_root(operation)).expect("operation root");
    SessionStore::open(operation_root).expect("open store")
}
