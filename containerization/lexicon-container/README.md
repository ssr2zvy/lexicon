# Lexicon Container

Note: run all commands from the repository root, the folder that contains
Cargo.toml and the containerization directory.

This container is **intentionally inert** under the current milestone. The
       audit document [`current.md`](../../current.md) §11 MZA-02 fixes MZA as
the sole installer authority. Until MZA publishes an accepted Protocol 1
installer API that owns install / upgrade / uninstall / command
registration / platform integration, this Containerfile cannot exercise a
real installer without inventing one, which `current.md` §3 explicitly
forbids.

The placeholder Containerfile documents the future flow: it will accept
`INSTALLER_PATH` pointing at the artifact produced by
[`automation/build_bundle_mza/build_release.sh`](../../automation/build_bundle_mza/build_release.sh)
and run it with the **exact upstream install argument and binary name** —
never reconstructing installation via shell copies. Source the schema in
[`workspace/specs/release-policy.md`](../../workspace/specs/release-policy.md).

Until then, real Lexicon work happens via the
[test-container README](../test-container/README.md).
