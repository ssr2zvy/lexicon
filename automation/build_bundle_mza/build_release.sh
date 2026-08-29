#!/usr/bin/env bash
# RELEASE-01 official-release entrypoint (current.md §11).
# - Lockfiles are reviewed inputs and must not be regenerated here.
# - The release CI may not prompt, run `cargo install`, or mutate the
#   Rust toolchain.
# - The Cargo.lock at the repo root IS the lockfile Cargo actually uses
#   because the repository is one Cargo workspace.
# - The pinned MZA revision is verified before any build invocation.
# - The `<accepted-mza-sha>` placeholder MUST be replaced with the
#   accepted MZA release commit before this script is allowed to run
#   in a final release pipeline.

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
MZA_DIR="$ROOT_DIR/automation/build_bundle_mza/mza"
CONFIG="$ROOT_DIR/automation/build_bundle_mza/mza_artifacts.toml"
ACCEPTED_MZA_SHA="<accepted-mza-sha>"

# Idempotent guards. Anything that fails here aborts the release.
test -f "$ROOT_DIR/Cargo.lock"
test -f "$CONFIG"
test -d "$MZA_DIR"

# Verify the MZA gitlink points at the accepted commit and the worktree
# is clean (no local dirt the release would silently inherit).
if [ "$ACCEPTED_MZA_SHA" = "<accepted-mza-sha>" ]; then
  echo "::error::automation/build_bundle_mza/build_release.sh: replace <accepted-mza-sha>" >&2
  exit 1
fi
test "$(git -C "$MZA_DIR" rev-parse HEAD)" = "$ACCEPTED_MZA_SHA"
git -C "$MZA_DIR" diff --exit-code
git -C "$ROOT_DIR" diff --exit-code -- Cargo.lock

# Locked, noninteractive MZA build. Pre-provision Rust / Zig / cargo-zigbuild
# in the release image; never prompt and never reinstall during the run.
cargo run --release --locked \
  --manifest-path "$MZA_DIR/Cargo.toml" \
  -- --config "$CONFIG"

# Final guard: even after the build, the workspace lockfile must be identical
# to the reviewed input. Any drift here is immediate evidence of a release
# that is no longer conformant.
git -C "$ROOT_DIR" diff --exit-code -- Cargo.lock
