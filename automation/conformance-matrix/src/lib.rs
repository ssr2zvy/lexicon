//! Conformance-matrix loader and rule-driven checker.
//!
//! A `conformance.toml` file enumerates the requirement rows the contract and
//! specification demand. Each row carries:
//!
//! - an `id` (unique across the file),
//! - an `authority` pointer (such as `workspace/specs/specs.md#30`),
//! - the `implementation` files/symbols that satisfy the requirement,
//! - the exact `tests` that are named evidence, and
//! - the supported `platforms`.
//!
//! The checker rejects:
//!
//! - duplicate requirement IDs,
//! - duplicate platform entries inside a single requirement,
//! - empty `implementation` lists,
//! - empty or unresolved `tests`,
//! - `tests` names that do not appear in `cargo test --workspace -- --list`,
//! - `conformant` rows without `durable_evidence`.

use std::collections::{BTreeMap, BTreeSet};

use serde::Deserialize;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum MatrixError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("toml parse error: {0}")]
    Toml(#[from] toml::de::Error),
    #[error("duplicate requirement id: {0}")]
    DuplicateRequirement(String),
    #[error("requirement {0} has empty implementation list")]
    EmptyImplementation(String),
    #[error("requirement {0} declares duplicate platform {1}")]
    DuplicatePlatform(String, String),
    #[error("requirement {0} declares empty tests list")]
    EmptyTests(String),
    #[error("requirement {0} declares conformant status without durable_evidence")]
    ConformantWithoutEvidence(String),
    #[error("requirement {0} test {1} does not appear in `cargo test -- --list`")]
    UnknownTest(String, String),
}

#[derive(Debug, Deserialize, Clone)]
pub struct ConformanceFile {
    pub schema_version: u32,
    #[serde(default)]
    pub requirement: Vec<Requirement>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct Requirement {
    pub id: String,
    pub authority: String,
    pub implementation: Vec<String>,
    #[serde(default)]
    pub tests: Vec<String>,
    #[serde(default)]
    pub platforms: Vec<String>,
    pub status: String,
    #[serde(default)]
    pub durable_evidence: Option<String>,
}

impl ConformanceFile {
    pub fn load(path: &std::path::Path) -> Result<Self, MatrixError> {
        let text = std::fs::read_to_string(path)?;
        let file: ConformanceFile = toml::from_str(&text)?;
        Ok(file)
    }
}

/// Performs the structural checks the matrix audit demands against an
/// already-loaded manifest and a known set of test names (typically captured
/// from `cargo test --workspace -- --list`).
pub fn check(file: &ConformanceFile, known_tests: &BTreeSet<String>) -> Result<(), MatrixError> {
    if file.schema_version != 1 {
        return Err(MatrixError::Toml(toml::de::Error::custom(
            "conformance.toml schema_version must be 1",
        )));
    }

    let mut seen_ids: BTreeSet<String> = BTreeSet::new();
    for req in &file.requirement {
        if !seen_ids.insert(req.id.clone()) {
            return Err(MatrixError::DuplicateRequirement(req.id.clone()));
        }
        if req.implementation.is_empty() {
            return Err(MatrixError::EmptyImplementation(req.id.clone()));
        }

        let mut seen_platforms: BTreeSet<String> = BTreeSet::new();
        for platform in &req.platforms {
            if !seen_platforms.insert(platform.clone()) {
                return Err(MatrixError::DuplicatePlatform(
                    req.id.clone(),
                    platform.clone(),
                ));
            }
        }

        if req.tests.is_empty() {
            return Err(MatrixError::EmptyTests(req.id.clone()));
        }
        for test in &req.tests {
            if !known_tests.contains(test) {
                return Err(MatrixError::UnknownTest(req.id.clone(), test.clone()));
            }
        }

        if req.status == "conformant" && req.durable_evidence.as_deref().map_or(true, str::is_empty)
        {
            return Err(MatrixError::ConformantWithoutEvidence(req.id.clone()));
        }
    }
    Ok(())
}

/// Convenience helper that folds `(crate-target, test-name)` pairs into a
/// flattened set of fully-qualified test identifiers, the form expected by
/// `conformance.toml` `tests` entries.
pub fn flatten_test_index<'a, I>(entries: I) -> BTreeSet<String>
where
    I: IntoIterator<Item = &'a (String, String)>,
{
    let mut out = BTreeSet::new();
    for (target, name) in entries {
        out.insert(format!("{target}::{name}"));
        out.insert(name.clone());
    }
    out
}

/// Parse the textual output of `cargo test --workspace -- --list` into
/// `(target, test_name)` rows. Cargo emits one line per discovered test as
/// `<binary-target>: <test-name>`. Returns one entry per line.
pub fn parse_cargo_test_list(text: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        if let Some((target, rest)) = line.split_once(':') {
            let target = target.trim().to_owned();
            let rest = rest.trim();
            if let Some(name) = rest.split_whitespace().next() {
                out.push((target, name.to_owned()));
            }
        }
    }
    out
}

/// Build a stable index keyed by requirement id; useful for tests that want
/// to assert a specific requirement is present (or absent).
pub fn index_requirements<'a>(file: &'a ConformanceFile) -> BTreeMap<&'a str, &'a Requirement> {
    file.requirement
        .iter()
        .map(|req| (req.id.as_str(), req))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_file() -> ConformanceFile {
        let text = r#"
            schema_version = 1

            [[requirement]]
            id = "contract-30-background-continuous-ownership"
            authority = "workspace/specs/specs.md#30"
            implementation = [
              "lexicon-core/src/session/handoff.rs",
              "lexicon-framework/src/data/background.rs",
            ]
            tests = [
              "background_handoff::real_operator_host_claims_reserved_handoff",
            ]
            platforms = ["linux-x86_64", "windows-x86_64"]
            status = "implemented and tested"
            durable_evidence = "ci/conformance.yml (linux + windows jobs)"

            [[requirement]]
            id = "specs-44-private-handler"
            authority = "workspace/specs/specs.md#44"
            implementation = ["lexicon-core/src/protocols/http/contract.rs"]
            tests = ["compile_pass_contracts"]
            platforms = ["linux-x86_64"]
            status = "conformant"
            durable_evidence = "ci/conformance.yml (linux)"
        "#;
        toml::from_str(text).expect("parse")
    }

    #[test]
    fn duplicate_ids_are_rejected() {
        let mut file = sample_file();
        let dup = file.requirement[0].clone();
        file.requirement.push(dup);
        let index = sample_index();
        let err = check(&file, &index).unwrap_err();
        matches!(err, MatrixError::DuplicateRequirement(_));
    }

    #[test]
    fn duplicate_platforms_are_rejected() {
        let mut file = sample_file();
        file.requirement[0]
            .platforms
            .push("linux-x86_64".to_owned());
        let index = sample_index();
        let err = check(&file, &index).unwrap_err();
        matches!(err, MatrixError::DuplicatePlatform(_, _));
    }

    #[test]
    fn empty_implementation_is_rejected() {
        let mut file = sample_file();
        file.requirement[0].implementation.clear();
        let index = sample_index();
        let err = check(&file, &index).unwrap_err();
        matches!(err, MatrixError::EmptyImplementation(_));
    }

    #[test]
    fn unknown_test_is_rejected() {
        let mut file = sample_file();
        file.requirement[0].tests[0] = "missing::test_name".to_owned();
        let index = sample_index();
        let err = check(&file, &index).unwrap_err();
        matches!(err, MatrixError::UnknownTest(_, _));
    }

    #[test]
    fn conformant_without_evidence_is_rejected() {
        let mut file = sample_file();
        file.requirement[1].durable_evidence = None;
        let index = sample_index();
        let err = check(&file, &index).unwrap_err();
        matches!(err, MatrixError::ConformantWithoutEvidence(_));
    }

    #[test]
    fn parse_cargo_test_list_extracts_target_and_name() {
        let text = "lexicon-core::protocols::http::runner::tests::one_get\n\
                    lexicon-cli::tests::background_handoff::real_operator_host_claims_reserved_handoff\n";
        let parsed = parse_cargo_test_list(text);
        assert_eq!(parsed.len(), 2);
        assert!(parsed.iter().any(|(t, n)| t == "lexicon-core" && n.starts_with("tests::")));
    }

    #[test]
    fn flatten_test_index_includes_short_and_long_names() {
        let entries = vec![(
            "background_handoff".to_owned(),
            "real_operator_host_claims_reserved_handoff".to_owned(),
        )];
        let index = flatten_test_index(entries.iter());
        assert!(index.contains("background_handoff::real_operator_host_claims_reserved_handoff"));
        assert!(index.contains("real_operator_host_claims_reserved_handoff"));
    }

    fn sample_index() -> BTreeSet<String> {
        let mut set = BTreeSet::new();
        set.insert("real_operator_host_claims_reserved_handoff".to_owned());
        set.insert("background_handoff::real_operator_host_claims_reserved_handoff".to_owned());
        set.insert("compile_pass_contracts".to_owned());
        set.insert("p::compile_pass_contracts".to_owned());
        set
    }
}
