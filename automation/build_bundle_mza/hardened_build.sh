#!/usr/bin/env bash
# Hardened offline build invocation (current.md §12 SUPPLY-01).
#
# Conceptually equivalent to:
#
#   podman run --rm --read-only --network none \
#     --userns keep-id --cap-drop all --security-opt no-new-privileges \
#     --mount type=bind,src="$SOURCE_SNAPSHOT",dst=/src,ro=true \
#     --mount type=bind,src="$VENDOR_SNAPSHOT",dst=/vendor,ro=true \
#     --mount type=volume,dst=/target \
#     --mount type=tmpfs,dst=/tmp \
#     --workdir /src \
#     "$PINNED_BUILDER_IMAGE" \
#     cargo build --workspace --release --locked --offline --target-dir /target
#
# The script is intentionally inert under the current milestone:
# MZA's installer API hasn't shipped yet (current.md §3), so the
# Podman image/runtime used here is a placeholder. Concrete values
# change once the accepted MZA and toolchain fingerprints are pinned.
#
# Operate the script only after:
#   1. the MZA submodule is initialized at the accepted release commit;
#   2. the vendored source archive is resolved and content-addressed;
#   3. `automation/build_bundle_mza/build_release.sh` has been audited
#      against the freeze SHA.

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
SOURCE_SNAPSHOT="${SOURCE_SNAPSHOT:-$ROOT_DIR}"
VENDOR_SNAPSHOT="${VENDOR_SNAPSHOT:-$ROOT_DIR/vendor}"
PINNED_BUILDER_IMAGE="${PINNED_BUILDER_IMAGE:-lexicon-release-builder:<sha>}"

if [ "$PINNED_BUILDER_IMAGE" = "lexicon-release-builder:<sha>" ]; then
  echo "::error::hardened_build.sh: PINNED_BUILDER_IMAGE must be a real image digest" >&2
  exit 1
fi
if [ ! -d "$SOURCE_SNAPSHOT" ]; then
  echo "::error::hardened_build.sh: SOURCE_SNAPSHOT must point to the prepared source snapshot" >&2
  exit 1
fi
if [ ! -d "$VENDOR_SNAPSHOT" ]; then
  echo "::error::hardened_build.sh: VENDOR_SNAPSHOT must point to the content-addressed vendor archive" >&2
  exit 1
fi

podman run --rm --read-only --network none \
  --userns keep-id --cap-drop all --security-opt no-new-privileges \
  --mount type=bind,src="$SOURCE_SNAPSHOT",dst=/src,ro=true \
  --mount type=bind,src="$VENDOR_SNAPSHOT",dst=/vendor,ro=true \
  --mount type=volume,dst=/target \
  --mount type=tmpfs,dst=/tmp \
  --workdir /src \
  -e "LEXICON_VENDOR_DIR=/vendor" \
  "$PINNED_BUILDER_IMAGE" \
  cargo build --workspace --release --locked --offline --target-dir /target
