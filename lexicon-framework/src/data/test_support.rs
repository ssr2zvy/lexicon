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
// Re-exports of the Core-side version constants used by hand-authored test
// runtime_information below; sourcing them from Core keeps the fixture in
// lockstep with the spec §22 representative probe response.
use lexicon_core::protocols::http::HTTPS_SOURCE_CONTRACT_IDENTIFIER;
use lexicon_core::MANAGED_RUNNER_TEMPLATE_VERSION;
use lexicon_core::CORE_CONTRACT_VERSION;

/// Serializes every test in this crate's test binary that changes the
/// process-global current working directory.
///
/// `RuntimeProjectLayout` discovery (`resolve_project_layout`) always reads
/// `std::env::current_dir()`; there is no override seam, so tests that need a
/// real layout must temporarily change the process cwd. Process cwd is shared
/// by every thread in this test binary, so this must be the *only* lock any
/// test in this crate uses to gate such a change (a second, disjoint lock
/// provides no protection against a concurrent change made through this one,
/// and can let an unrelated test that spawns a subprocess momentarily observe
/// an invalid working directory). `lexicon_framework::tests` (in `lib.rs`)
/// reuses this same lock via `with_test_cwd` rather than declaring its own.
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
    fs::create_dir_all(&protocol_root).expect("create protocol root");
    let source_toml = crate::format_source_toml(source_name, "http");
    fs::write(protocol_root.join("source.toml"), source_toml).expect("write source.toml");

    fs::create_dir_all(protocol_root.join("data").join("raw")).expect("create data/raw");
    fs::create_dir_all(protocol_root.join("data").join("processed")).expect("create data/processed");
    fs::create_dir_all(protocol_root.join("get-raw-data").join("sessions"))
        .expect("create get-raw-data/sessions");
    fs::create_dir_all(protocol_root.join("process-data").join("sessions"))
        .expect("create process-data/sessions");

    let acquisition_bundle_dir = protocol_root.join("get-raw-data").join("runtime");
    let processing_bundle_dir = protocol_root.join("process-data").join("runtime");
    write_fake_http_bundle(&acquisition_bundle_dir, source_name);
    write_fake_processing_bundle(&processing_bundle_dir, source_name);

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
        // Spec §22 metadata fields required by the new `RuntimeInformationV1`
        // v1 document; pulled from Core constants so the test fixture can
        // never drift from what the runtime information probe actually emits.
        "source_contract": HTTPS_SOURCE_CONTRACT_IDENTIFIER,
        "core_contract": CORE_CONTRACT_VERSION,
        "runner_template": MANAGED_RUNNER_TEMPLATE_VERSION,
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

/// Write a real, admissible processing runtime bundle into `bundle_dir`.
fn write_fake_processing_bundle(bundle_dir: &Path, source_name: &str) {
    fs::create_dir_all(bundle_dir).expect("create processing bundle dir");

    let executable_name = "fake-processing-runtime";
    let executable_path = bundle_dir.join(executable_name);
    fs::write(&executable_path, b"fake-processing-executable-bytes-for-tests").expect("write executable");

    let artifact = hash_runtime_executable(&executable_path).expect("hash executable");

    let contract_version = lexicon_core::processing::ProcessingSourceContractV1::CONTRACT_VERSION;
    let runtime_information = serde_json::json!({
        "schema_version": 1,
        "identity": {
            "source": source_name,
            "protocol": "http",
            "operation": "processing",
            "source_contract_version": contract_version,
        },
        "descriptor": {
            "contract_version": contract_version,
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
        crate::data::project::resolve_project_layout(&project.source_name, "http", DataOperation::Acquisition)
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
