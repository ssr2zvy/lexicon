# Lexicon

Lexicon makes data: it acquires raw sources via typed HTTP contracts, applies
source-owned processing against a Core-owned SQLite ledger, and publishes the
processed artifacts onto a managed runbook. Every executable ships through
[MZA](https://github.com/ssr2zvy/mza) as the single installer authority.

The audit document at [`current.md`](current.md) is the source of truth for
conformance state. Anything in this README that disagrees with `current.md`
is wrong.

## Source and submodule layout

Clone this repository with submodules:

```bash
git clone https://github.com/ssr2zvy/lexicon.git
cd lexicon
git submodule update --init --recursive
```

The pinned MZA submodule lives under `automation/build_bundle_mza/mza/`.
The submodule's gitlink is recorded in [`.gitmodules`](.gitmodules) and
must point at the **accepted MZA release commit** recorded in `current.md`
§11 MZA-01. Until MZA publishes a Protocol 1 installer API, the placeholder
gitlink points at the audited snapshot commit
`d2c2406ed9f83d2de4c7a38fbf1ac3a568d1e410`; release pipelines refuse to
run while the link disagrees with `build_release.sh`'s `<accepted-mza-sha>`.

## Release entrypoint

The official-release build entrypoint is noninteractive, locked, and stops
on any toolchain mutation:

```bash
bash automation/build_bundle_mza/build_release.sh
```

The artifact configuration the script consumes is
[`automation/build_bundle_mza/mza_artifacts.toml`](automation/build_bundle_mza/mza_artifacts.toml).
Updates to MZA's Protocol 1 grammar must be reflected in that TOML and in
`automation/build_bundle_mza/build_release.sh` together, in the same commit.

The Lexicon-owned wrapper scripts that previously lived under
`automation/build_bundle_install/` have been removed. The release pipeline
no longer:

- regenerates lockfiles (`update_lock_file.sh` is gone);
- parses MZA TOML with `awk` or extracts archives with `find | head`;
- uninstalls the developer's current command;
- drives MZA's `make-artifact.sh` interactively; or
- copies a moving MZA archive into a singular `artifact.toml`.

## Containerization

Container definitions live under [containerization/](containerization/):

- [containerization/test-container/README.md](containerization/test-container/README.md) —
  for developing, testing, and releasing the Lexicon project itself. Mounts
  the whole repo and builds Lexicon from source, giving you a persistent,
  working Rust/Zig toolchain.
- [containerization/lexicon-container/README.md](containerization/lexicon-container/README.md) —
  for using the Lexicon project. After MZA publishes an installer, the
  container bakes the produced installer into the image and runs it once
  at build time. Until then it is intentionally inert: see `current.md` §11
  MZA-02.

Use the repository root as the Podman build context, not the container's
own folder:

- `podman build -f containerization/test-container/Containerfile -t lexicon-local-test-image .`
- `podman build -f containerization/lexicon-container/Containerfile --build-arg BUNDLE_INSTALLER=<path-to-installer> -t lexicon-local-image .`

This keeps the repo available to the builder and lets each Containerfile
copy its own `entrypoint.sh` from the root context correctly.
