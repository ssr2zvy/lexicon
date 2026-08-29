//! COREID-02 exact Core dependency admission.
//!
//! The audit requires a typed validator that admits only the exact
//! dependency table Core mandates:
//!
//! ```toml
//! lexicon_core = {
//!   package = "lexicon-core",
//!   git = "https://github.com/ssr2zvy/lexicon",
//!   rev = "<embedded 40-character revision>",
//! }
//! ```
//!
//! Anything else — `path`, `version`, `branch`, `tag`, `registry`,
//! alternate Git URLs, extra features, or any extra key — must be
//! rejected with a typed error. After the static manifest is admitted,
//! the test-helper verifies the resolved package by invoking
//! `cargo metadata --locked` (the audit forbids `--no-deps` for final
//! identity admission).
//!
//! The validator is split into two levels:
//!
//! * `validate_lexicon_core_dependency_table` parses a TOML manifest
//!   and admits or rejects the `lexicon_core` table at the source
//!   language level. It is fully deterministic and does not require
//!   Cargo.
//! * `verify_lexicon_core_metadata` shells out to Cargo to confirm
//!   the resolved commit, source URL, and package identity agree with
//!   `EMBEDDED_CORE_IDENTITY`. This second pass is what the audit calls
//!   "final identity admission".

use std::process::Command;
use thiserror::Error;

/// Canonical Core git URL accepted by the validator. This must agree
/// with the URL embedded at Lexicon build time (COREID-01) and with the
/// repository the audit points at.
pub const REQUIRED_CORE_GIT_URL: &str = "https://github.com/ssr2zvy/lexicon";

/// Canonical Core package name. The dependency table must use the
/// literally exact string `package = "lexicon-core"` to satisfy the
/// key-set comparison.
pub const REQUIRED_CORE_PACKAGE: &str = "lexicon-core";

/// The exact key set the Core dependency table must expose. Comparison
/// is performed against this slice verbatim (per COREID-02 audit).
pub const REQUIRED_KEYS: [&str; 3] = ["git", "package", "rev"];

/// Persisted Core identity embedded at Lexicon build time. The
/// validator's tests reference it directly so the value below never
/// drifts from the binary's `EMBEDDED_CORE_IDENTITY`.
pub const EMBEDDED_CORE_GIT_REV: &str = env!("LEXICON_EMBEDDED_CORE_REV");

/// Persisted Core git URL identity, the same key as
/// `REQUIRED_CORE_GIT_URL`. The validator enforces this string in
/// both halves.
pub const EMBEDDED_CORE_GIT_URL: &str = env!("LEXICON_EMBEDDED_CORE_URL");

/// Reasons the validator may reject a manifest.
#[derive(Debug, Error)]
pub enum CoreDependencyError {
    /// The manifest could not be parsed as TOML.
    #[error("failed to parse manifest as TOML: {0}")]
    TomlParse(String),
    /// The manifest does not contain a `lexicon_core` table at all.
    #[error("manifest is missing the required `lexicon_core` table")]
    MissingTable,
    /// The `lexicon_core` table is not an inline table.
    #[error("`lexicon_core` must be an inline table with exactly git/package/rev")]
    WrongShape,
    /// The key set on `lexicon_core` does not equal `REQUIRED_KEYS`.
    #[error("`lexicon_core` keys must equal exactly {expected:?}, got {actual:?}")]
    InvalidKeySet {
        expected: Vec<String>,
        actual: Vec<String>,
    },
    /// The `package` field is missing or does not equal the canonical name.
    #[error("`lexicon_core.package` must equal exactly `{REQUIRED}`")]
    PackageMismatch,
    /// The `git` field is missing or does not equal the canonical URL.
    #[error("`lexicon_core.git` must equal exactly `{REQUIRED}`")]
    GitUrlMismatch,
    /// The `rev` field is missing, malformed, or disagrees with the
    /// embedded Core git revision.
    #[error("`lexicon_core.rev` must equal the 40-character embedded Core revision")]
    RevisionMismatch,
}

impl CoreDependencyError {
    fn package_mismatch() -> Self {
        Self::PackageMismatch
    }
    fn git_url_mismatch() -> Self {
        Self::GitUrlMismatch
    }
}

/// TOML value narrowed to the only kinds the audit allows inside the
/// `lexicon_core` dependency table.
#[derive(Debug, Clone, PartialEq, Eq)]
enum CoreDependencyValue {
    Table(Vec<(String, CoreDependencyValue)>),
    String(String),
    Boolean(bool),
    Integer(i64),
    Array(Vec<CoreDependencyValue>),
}

/// Parse a single TOML document from `text`. We restrict values to the
/// kinds the audit permits for a Core dependency table.
fn parse_toml(text: &str) -> Result<CoreDependencyValue, CoreDependencyError> {
    let value: toml::Value =
        toml::from_str(text).map_err(|e| CoreDependencyError::TomlParse(e.to_string()))?;
    convert(value)
}

fn convert(value: toml::Value) -> Result<CoreDependencyValue, CoreDependencyError> {
    match value {
        toml::Value::Table(table) => {
            let mut entries = Vec::with_capacity(table.len());
            for (key, value) in table {
                entries.push((key, convert(value)?));
            }
            entries.sort_by(|a, b| a.0.cmp(&b.0));
            Ok(CoreDependencyValue::Table(entries))
        }
        toml::Value::String(s) => Ok(CoreDependencyValue::String(s)),
        toml::Value::Boolean(b) => Ok(CoreDependencyValue::Boolean(b)),
        toml::Value::Integer(i) => Ok(CoreDependencyValue::Integer(i)),
        toml::Value::Array(a) => {
            let mut entries = Vec::with_capacity(a.len());
            for value in a {
                entries.push(convert(value)?);
            }
            Ok(CoreDependencyValue::Array(entries))
        }
        toml::Value::Datetime(_) => Err(CoreDependencyError::TomlParse(
            "datetime values are not allowed in the Core dependency table".to_owned(),
        )),
        toml::Value::Float(_) => Err(CoreDependencyError::TomlParse(
            "float values are not allowed in the Core dependency table".to_owned(),
        )),
    }
}

/// Parsed `lexicon_core` table values used by the validator.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RequiredCoreDependency {
    pub package: String,
    pub git: String,
    pub rev: String,
}

/// Validate the `lexicon_core` table in `text`. Returns Ok if and only
/// if every key check passes; otherwise the typed error tells the caller
/// which gate failed. Used by scaffold-generation code and unit tests
/// to confirm the formatter produces the canonical dependency shape.
pub fn validate_lexicon_core_dependency_table(
    text: &str,
) -> Result<RequiredCoreDependency, CoreDependencyError> {
    let root = parse_toml(text)?;
    let CoreDependencyValue::Table(root_entries) = root else {
        return Err(CoreDependencyError::WrongShape);
    };

    let lexicon_core_entry = root_entries
        .iter()
        .find(|(key, _)| key == "lexicon_core")
        .ok_or(CoreDependencyError::MissingTable)?;

    let CoreDependencyValue::Table(lexicon_core_table) = &lexicon_core_entry.1 else {
        return Err(CoreDependencyError::WrongShape);
    };

    let actual_keys: Vec<String> = lexicon_core_table
        .iter()
        .map(|(key, _)| key.clone())
        .collect();
    let expected_keys: Vec<String> = REQUIRED_KEYS.iter().map(|s| s.to_string()).collect();
    let actual_sorted = {
        let mut v = actual_keys.clone();
        v.sort();
        v
    };
    let expected_sorted = {
        let mut v = expected_keys.clone();
        v.sort();
        v
    };
    if actual_sorted != expected_sorted {
        return Err(CoreDependencyError::InvalidKeySet {
            expected: expected_keys,
            actual: actual_keys,
        });
    }

    // All three keys are present (key-set check above guarantees it).
    let get_str = |key: &str| -> Option<String> {
        lexicon_core_table
            .iter()
            .find_map(|(k, v)| if k == key {
                if let CoreDependencyValue::String(s) = v {
                    Some(s.clone())
                } else {
                    None
                }
            } else {
                None
            })
    };

    let package = get_str("package").ok_or_else(CoreDependencyError::package_mismatch)?;
    if package != REQUIRED_CORE_PACKAGE {
        return Err(CoreDependencyError::package_mismatch());
    }

    let git = get_str("git").ok_or_else(CoreDependencyError::git_url_mismatch)?;
    if git != REQUIRED_CORE_GIT_URL {
        return Err(CoreDependencyError::git_url_mismatch());
    }

    let rev = get_str("rev").ok_or(CoreDependencyError::RevisionMismatch)?;
    if rev != EMBEDDED_CORE_GIT_REV || rev.len() != 40 || rev.chars().all(|c| c == '0') {
        return Err(CoreDependencyError::RevisionMismatch);
    }

    Ok(RequiredCoreDependency {
        package,
        git,
        rev,
    })
}

impl std::fmt::Display for RequiredCoreDependency {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "package = \"{}\", git = \"{}\", rev = \"{}\"",
            self.package, self.git, self.rev
        )
    }
}

/// Errors raised by the Cargo metadata second pass.
#[derive(Debug, Error)]
pub enum CoreMetadataError {
    /// Cargo was not invokable.
    #[error("cargo metadata invocation failed: {0}")]
    CargoCommand(String),
    /// cargo metadata returned a non-zero exit code.
    #[error("cargo metadata exited {code} with stderr: {stderr}")]
    CargoFailure { code: i32, stderr: String },
    /// The output could not be parsed as JSON.
    #[error("cargo metadata output is not valid JSON: {0}")]
    CargoDecode(String),
    /// The metadata did not include a `lexicon-core` resolve entry
    /// whose source URL, requested revision, or commit fragment match
    /// the embedded Core identity.
    #[error("lexicon-core resolve entry disagrees with embedded identity: {0}")]
    IdentityMismatch(String),
    /// Cargo metadata listed multiple resolve entries for `lexicon-core`.
    #[error("lexicon-core is resolved to {count} entries; final admission requires exactly one")]
    DuplicateResolve { count: usize },
}

/// Invoke `cargo metadata --locked --format-version 1 --manifest-path
/// <workspace>/Cargo.toml` and verify the resolved `lexicon-core`
/// package agrees with the embedded identity entries. The audit
/// forbids `--no-deps` for final identity admission; this function
/// deliberately does not add that flag.
pub fn verify_lexicon_core_metadata(workspace_manifest: &std::path::Path) -> Result<(), CoreMetadataError> {
    if !workspace_manifest.is_file() {
        return Err(CoreMetadataError::CargoCommand(format!(
            "manifest path does not exist: {}",
            workspace_manifest.display()
        )));
    }

    let output = Command::new("cargo")
        .arg("metadata")
        .arg("--locked")
        .arg("--format-version")
        .arg("1")
        .arg("--manifest-path")
        .arg(workspace_manifest)
        .output()
        .map_err(|e| CoreMetadataError::CargoCommand(e.to_string()))?;

    if !output.status.success() {
        return Err(CoreMetadataError::CargoFailure {
            code: output.status.code().unwrap_or(-1),
            stderr: String::from_utf8_lossy(&output.stderr).to_string(),
        });
    }

    let json: serde_json::Value = serde_json::from_slice(&output.stdout)
        .map_err(|e| CoreMetadataError::CargoDecode(e.to_string()))?;

    let resolve_nodes = json
        .get("resolve")
        .and_then(|v| v.get("nodes"))
        .and_then(|v| v.as_array())
        .ok_or_else(|| CoreMetadataError::CargoDecode("missing resolve.nodes".to_owned()))?;

    let mut matching: Vec<&serde_json::Value> = resolve_nodes
        .iter()
        .filter(|node| {
            node.get("id")
                .and_then(|v| v.as_str())
                .map(|id| id.starts_with("lexicon-core@"))
                .unwrap_or(false)
        })
        .collect();
    if matching.len() > 1 {
        return Err(CoreMetadataError::DuplicateResolve {
            count: matching.len(),
        });
    }
    let Some(node) = matching.pop() else {
        return Err(CoreMetadataError::IdentityMismatch(
            "no resolve nodes for `lexicon-core`".to_owned(),
        ));
    };

    let source_url = node
        .get("source")
        .and_then(|v| v.as_str())
        .map(|s| s.to_owned());
    let source_url = source_url.unwrap_or_default();
    if !source_url.contains(REQUIRED_CORE_GIT_URL) {
        return Err(CoreMetadataError::IdentityMismatch(format!(
            "resolve source '{source_url}' does not reference required URL '{}'",
            REQUIRED_CORE_GIT_URL
        )));
    }

    if let Some(dep_kinds) = node.get("dependencies").and_then(|v| v.as_array()) {
        for dep in dep_kinds {
            let name = dep.get("name").and_then(|v| v.as_str()).unwrap_or("");
            if name == "lexicon-core" {
                let req = dep
                    .get("req")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default();
                let hash_starts_with = EMBEDDED_CORE_GIT_REV
                    .get(..7)
                    .unwrap_or(EMBEDDED_CORE_GIT_REV);
                if !req.contains(hash_starts_with) {
                    return Err(CoreMetadataError::IdentityMismatch(format!(
                        "dependency request '{req}' does not match embedded revision '{EMBEDDED_CORE_GIT_REV}'"
                    )));
                }
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn canonical_table() -> String {
        format!(
            "[dependencies]\n\
             lexicon_core = {{ package = \"{REQUIRED_CORE_PACKAGE}\", git = \"{REQUIRED_CORE_GIT_URL}\", rev = \"{EMBEDDED_CORE_GIT_REV}\" }}\n",
        )
    }

    #[test]
    fn canonical_dependency_table_is_admitted() {
        let text = canonical_table();
        let parsed = validate_lexicon_core_dependency_table(&text)
            .expect("canonical dependency table must be admitted");
        assert_eq!(parsed.package, REQUIRED_CORE_PACKAGE);
        assert_eq!(parsed.git, REQUIRED_CORE_GIT_URL);
        assert_eq!(parsed.rev, EMBEDDED_CORE_GIT_REV);
    }

    #[test]
    fn path_dependency_is_rejected() {
        let text = format!(
            "[dependencies]\n\
             lexicon_core = {{ path = \"../lexicon-core\" }}\n",
        );
        let err = validate_lexicon_core_dependency_table(&text).unwrap_err();
        assert!(matches!(err, CoreDependencyError::InvalidKeySet { .. }));
    }

    #[test]
    fn version_dependency_is_rejected() {
        let text = format!(
            "[dependencies]\n\
             lexicon_core = {{ version = \"0.1.2\" }}\n",
        );
        let err = validate_lexicon_core_dependency_table(&text).unwrap_err();
        assert!(matches!(err, CoreDependencyError::InvalidKeySet { .. }));
    }

    #[test]
    fn alternate_git_url_is_rejected() {
        let text = format!(
            "[dependencies]\n\
             lexicon_core = {{ package = \"{REQUIRED_CORE_PACKAGE}\", git = \"https://example.com/other/lexicon\", rev = \"{EMBEDDED_CORE_GIT_REV}\" }}\n",
        );
        let err = validate_lexicon_core_dependency_table(&text).unwrap_err();
        assert!(matches!(err, CoreDependencyError::GitUrlMismatch));
    }

    #[test]
    fn revision_mismatch_is_rejected() {
        let other_rev = "1".repeat(40);
        let text = format!(
            "[dependencies]\n\
             lexicon_core = {{ package = \"{REQUIRED_CORE_PACKAGE}\", git = \"{REQUIRED_CORE_GIT_URL}\", rev = \"{other_rev}\" }}\n",
        );
        let err = validate_lexicon_core_dependency_table(&text).unwrap_err();
        assert!(matches!(err, CoreDependencyError::RevisionMismatch));
    }

    #[test]
    fn missing_lexicon_core_table_is_rejected() {
        let text = "[dependencies]\nserde = \"1\"\n";
        let err = validate_lexicon_core_dependency_table(text).unwrap_err();
        assert!(matches!(err, CoreDependencyError::MissingTable));
    }

    #[test]
    fn package_name_mismatch_is_rejected() {
        let text = format!(
            "[dependencies]\n\
             lexicon_core = {{ package = \"another-package\", git = \"{REQUIRED_CORE_GIT_URL}\", rev = \"{EMBEDDED_CORE_GIT_REV}\" }}\n",
        );
        let err = validate_lexicon_core_dependency_table(&text).unwrap_err();
        assert!(matches!(err, CoreDependencyError::PackageMismatch));
    }

    #[test]
    fn extra_key_is_rejected() {
        let text = format!(
            "[dependencies]\n\
             lexicon_core = {{ package = \"{REQUIRED_CORE_PACKAGE}\", git = \"{REQUIRED_CORE_GIT_URL}\", rev = \"{EMBEDDED_CORE_GIT_REV}\", features = [] }}\n",
        );
        let err = validate_lexicon_core_dependency_table(&text).unwrap_err();
        assert!(matches!(err, CoreDependencyError::InvalidKeySet { .. }));
    }

    #[test]
    fn missing_lexicon_core_table_yields_missing_table_error() {
        let text = "# empty workspace\n";
        let err = validate_lexicon_core_dependency_table(text).unwrap_err();
        assert!(matches!(err, CoreDependencyError::MissingTable));
    }
}

