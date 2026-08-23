#!/usr/bin/env bash
set -e

BUILD_BUNDLE_INSTALL=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
ROOT_DIR="${ROOT_DIR:-$(cd "$BUILD_BUNDLE_INSTALL/../.." && pwd)}"

bash "$BUILD_BUNDLE_INSTALL/get_build_variables.sh"
source "$BUILD_BUNDLE_INSTALL/local_build_variables.sh"

BUNDLE_ARTIFACT_DIR="$ROOT_DIR/artifacts/lexicon_bundle/$BUNDLE_TYPE/$BUNDLE_PROTOCOL/$BUNDLE_VERSION/$BUNDLE_TARGET"
UPDATE_LOCK_FILE_SCRIPT="$BUILD_BUNDLE_INSTALL/update_lock_file.sh"
MAKE_ARTIFACT_SCRIPT="$BUILD_BUNDLE_INSTALL/../build_bundle_mza/mza/make-artifact.sh"
INSTALL_SCRIPT="$BUILD_BUNDLE_INSTALL/install.sh"

bbi_echo() {
    message=$1
    echo "[[BUILD_BUNDLE_INSTALL]] $message"
}

case "${1:-}" in
    skip|--skip|--skip-artifact)
        SKIP_ARTIFACT=1
        ;;
    *)
        SKIP_ARTIFACT=0
        ;;
esac

bbi_echo "Beginning build, bundle, install process"
bbi_echo "Version: $BUNDLE_VERSION"
bbi_echo "Protocol: $BUNDLE_PROTOCOL"
bbi_echo "Type: $BUNDLE_TYPE"
bbi_echo "Target: $BUNDLE_TARGET"

bbi_echo "Updating lock files"
bash "$UPDATE_LOCK_FILE_SCRIPT"

bbi_echo "Building and bundling lexicon"
if [ "$SKIP_ARTIFACT" -eq 1 ]; then
    bbi_echo "Skipping artifact creation."
else
    bash "$MAKE_ARTIFACT_SCRIPT"
fi

ARCHIVE_PATH="$(find "$BUNDLE_ARTIFACT_DIR" -maxdepth 1 -type f -name '*.tar.xz' | head -n 1)"
if [ -z "$ARCHIVE_PATH" ]; then
    bbi_echo "No bundle archive found in $BUNDLE_ARTIFACT_DIR" >&2
    exit 1
fi

rm -rf "$BUILD_BUNDLE_INSTALL/bundles/lexicon-bundle-$BUNDLE_VERSION"
mkdir -p "$BUILD_BUNDLE_INSTALL/bundles/"

bbi_echo "Extracting bundle archive from $(basename "$ARCHIVE_PATH")"
tar -xf "$ARCHIVE_PATH" -C "$BUILD_BUNDLE_INSTALL/bundles/"

cd "$BUILD_BUNDLE_INSTALL"

if command -v lexicon >/dev/null 2>&1; then
    bbi_echo "Lexicon is installed; uninstalling current version before reinstall"
    bash "$INSTALL_SCRIPT" --uninstall
else
    bbi_echo "Lexicon is not installed; skipping uninstall step"
fi

bbi_echo "Installing bundle"
bash "$INSTALL_SCRIPT" --install

bbi_echo "Build, bundle, install process completed successfully"
