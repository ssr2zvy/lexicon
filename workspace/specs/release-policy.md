# Lexicon official-release supply-chain policy

This policy governs how official Lexicon releases are constructed,
verified, and published. It is the canonical counterpart to the
implementation gate documented in `current.md` §SUPPLY-01.

## Scope

This policy controls official Lexicon build inputs, build-time
execution, artifact handling, verification, and provenance. It does
**not** claim that a container makes dependencies trustworthy, and it
does not change Contract V1's trusted-native source execution model.

## Immutable inputs

An official release identifies:

- The exact Lexicon and MZA commits.
- An exact stable Rust toolchain.
- The exact Zig and cargo-zigbuild versions and hashes.
- The reviewed `Cargo.lock`.
- A vendored Cargo source tree.
- Container or VM image digests.
- Release configuration.
- Target triples.

Release-time resolution or lockfile mutation is forbidden. Build
images are pre-resolved with the required toolchains at preparation
time; the release job never calls `cargo install` or `rustup` against
moving targets.

## Build-time code inventory

Before approval, inventory:

- Every package containing a `build.rs`.
- Every procedural macro reachable from compiled sources.
- Every native compiler or linker dependency.
- Every tool invoked by those `build.rs` scripts.

For each inventory entry record: package name, package version,
source checksum, purpose, review status, and reviewer. A changed
inventory blocks release approval.

## Dependency review

Review newly introduced and changed dependencies for:

- Source code shape and surface area.
- `build.rs` and procedural macro surface area.
- Unsafe code concentration.
- Licenses compatible with the project license.
- Open advisories.
- Maintainer and source changes since the previous inventory.

A clean advisory scan is **evidence**, not proof of benign behavior.

## Isolated source build

The build runs in a fresh rootless container or VM with:

- Network disabled (`--network none`).
- The contributor's credentials, SSH agents, Docker/Podman sockets,
  host home directories, and signing keys never mounted into the
  builder.
- Project source, vendor tree, and toolchains mounted read-only.
- Only target/output/temp directories writable.
- Cargo invocation: `cargo build --workspace --release --locked --offline`.
- For release-time signing, a separate minimal environment receives
  only the signed artifact and a release-specific signing key.

This isolation contains most build-time effects; it does not establish
that the produced binary is benign.

## Artifact quarantine

Produced binaries and build logs are treated as untrusted until:

- Structural inspection completes.
- Malware scanning completes.
- Native functional test suites pass.
- Reproducibility comparison (where the toolchain supports it) passes.
- Provenance verification completes.

Signing occurs only after these gates and in a separate minimal
environment.

## Evidence

Release evidence includes:

- The exact commit SHAs for Lexicon and MZA.
- The exact container or VM image digest.
- The exact toolchain versions and SHA digests.
- The exact command list with argv, exit codes, started-at, and
  finished-at timestamps.
- Stream hashes (stdout and stderr) for every command.
- The vendored source archive hash.
- The CycloneDX SBOM and per-package license map.
- The final artifact SHA-256 set.

A number in `current.md` (or its successor) without the attached
verification manifest and durable workflow link is not evidence.

## Native platform evidence

For each supported target platform, the CI workflow must record:

- The platform name and architecture.
- The exact CI runner image digest.
- The native test results.

Cross-compilation proves compilation; runtime behavior must be
demonstrated on the target's native runner.

## Exceptions

An exception names the invariant it waives, the reason, the owner,
the expiry, the compensating controls, and the approving reviewers.
It cannot be described as conformance with the waived property;
verifiers must surface the exception alongside the durable evidence.
