#!/usr/bin/env bash
set -e



BUILD_BUNDLE_INSTALL=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
if [ -z "$BUNDLE_VERSION" ]; then
    bash "$BUILD_BUNDLE_INSTALL/get_build_variables.sh"
fi
source "$BUILD_BUNDLE_INSTALL/local_build_variables.sh"
cd "$BUILD_BUNDLE_INSTALL/bundles/lexicon-bundle-$BUNDLE_VERSION"

i_echo() {
    message=$1
    echo "[[INSTALL]] $message"
}

chmod +x ./lexicon-bundle
i_echo "Triggering installation maintenance by executing lexicon-bundle"
./lexicon-bundle

i_echo "Verifying installation status by checking lexicon version"
lexicon -V

i_echo "Installed."