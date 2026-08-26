# Implementation report

Implemented disk-based processing runtime bundle admission in `lexicon-framework`.

Summary
- Added `AdmittedProcessingRuntimeBundle` and `admit_processing_runtime_bundle(...)` in `lexicon-framework/src/build/runtime_bundle_admission.rs`.
- Exported the public processing bundle API from `lexicon-framework/src/build/mod.rs`.
- Reused the shared manifest boundary checks and bundle validation flow already used by HTTP runtime admission, while leaving compatibility validation to the core processing runtime contract.
- Verified the implementation with focused regression tests for valid and malformed processing bundles.

Behavior covered
- rejects missing or non-directory bundle roots,
- rejects symlinked bundle paths and manifest/executable paths,
- validates `runtime.json` length and exact final-newline/UTF-8 boundary rules,
- decodes the processing runtime manifest and checks identity compatibility,
- resolves the manifest-declared executable as a direct child,
- rejects unexpected directory entries and mismatched executable size/SHA-256 hashes.

Validation
- Ran: `cargo test --workspace --quiet`
- Result: all workspace tests passed.

Implement Display and Error.

Do not return plain String, print diagnostics, or exit.

No process execution

Admission must not call:

probe_processing_runtime_information(...)

It validates:

* recorded processing runtime information;
* recorded artifact integrity;
* a fresh hash of the executable currently on disk.

Required tests

Add tests proving:

1. A successfully staged processing bundle is admitted.
2. All admitted paths and metadata are preserved.
3. Processing identity is accessible.
4. Missing and non-directory bundle paths are rejected.
5. Bundle-directory symlinks are rejected.
6. Missing, symlinked, or non-regular manifests are rejected.
7. Empty and oversized manifests are rejected.
8. Every invalid manifest boundary is rejected.
9. Invalid processing manifest JSON is rejected.
10. Unknown manifest schema versions are rejected.
11. Acquisition runtime information is rejected.
12. Processing identity mismatch is rejected.
13. Descriptor-version mismatch is rejected.
14. Missing declared executable is rejected.
15. Symlinked or non-regular executables are rejected.
16. Extra files and directories are rejected.
17. Modified executable size is rejected.
18. Same-size executable substitution is rejected by SHA-256.
19. Mismatch errors preserve expected and actual values.
20. Admission does not execute or modify the runtime.
21. Dropping the admitted value does not delete the bundle.
22. Acquisition bundle admission remains unchanged.
23. Acquisition and processing share private mechanics where practical.
24. Existing processing staging tests remain unchanged.
25. All workspace tests pass repeatedly.

Use StagedProcessingRuntimeBundle as the valid fixture and keep its owner alive during admission tests.

Preserve existing behavior

Do not change:

* acquisition bundle admission API;
* acquisition staging;
* processing staging ownership;
* processing manifest schema;
* processing verification or probing;
* hashing;
* reversible publication;
* source scaffolding;
* source create;
* source build;
* Cargo invocation;
* legacy publication;
* CLI behavior;
* MZA;
* Protocol 1;
* lexicon-bundle;
* installer behavior;
* bundle inputs;
* installed paths.

lexicon-bundle remains a binary installer built through cargo-bundler-v0.1.0.

Validation

Run:

cargo test -p lexicon-framework --quiet

Run:

cargo test --workspace --quiet
cargo test --workspace --quiet

If the external MZA checkout is available, run:

bash automation/build_bundle_install/build_bundle_install.sh

If unavailable, report the known external blocker separately. Do not modify MZA or installer code.

Explicit exclusions

Do not implement:

* processing publication;
* paired publication;
* integration with source build;
* runtime execution;
* re-probing during admission;
* processing runner main.rs;
* processing logic or SQLite;
* raw-data discovery;
* sessions;
* source workspace migration;
* managed acquisition runners;
* runner::run;
* invocation envelopes;
* HTTP execution;
* raw recording;
* supervision;
* __operator-host.

Completion report

After completion, replace current.md with a report containing:

* files changed;
* processing bundle-admission API;
* admitted processing bundle representation;
* shared filesystem admission mechanics;
* exact directory and manifest boundary rules;
* processing compatibility delegation;
* executable containment and fresh hashing;
* typed errors;
* malformed bundle rejection results;
* confirmation that no runtime was executed or modified;
* acquisition regression results;
* framework and repeated workspace test results;
* bundle/install result or the known external-MZA blocker.

Then stop. Do not publish the processing bundle.