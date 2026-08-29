#!/usr/bin/env bash
# Test-container entrypoint (current.md §13 / §16 / RELEASE-02).
#
# The release/bundle entrypoint now lives at
# `automation/build_bundle_mza/build_release.sh`. The previous
# `automation/build_bundle_install/` scripts (`build_bundle_install.sh`,
# `install.sh`, `get_build_variables.sh`, `local_build_variables.sh`,
# `update_lock_file.sh`) and `lexicon-install.toml` are removed per
# RELEASE-02; an interactive bundle install menu is no longer wired in.

set -euo pipefail

export PATH="/usr/local/cargo/bin:${PATH}"

LEXICON_ROOT="${LEXICON_ROOT:-/lexicon}"
REPO_DIR="${REPO_DIR:-$LEXICON_ROOT}"
RELEASE_SCRIPT="$REPO_DIR/automation/build_bundle_mza/build_release.sh"
MZA_CONFIG="$REPO_DIR/automation/build_bundle_mza/mza_artifacts.toml"

REQUIRED_PATHS=(
    "$REPO_DIR/Cargo.toml"
    "$REPO_DIR/lexicon-cli"
    "$REPO_DIR/lexicon-bundle"
    "$REPO_DIR/lexicon-framework"
    "$RELEASE_SCRIPT"
    "$MZA_CONFIG"
    "$REPO_DIR/automation/build_bundle_mza/mza"
    "$REPO_DIR/.gitmodules"
)

tc_echo() {
    echo "[[TEST_CONTAINER]] $1"
}

wait_for_work() {
    tc_echo "Container is idle and still running. Use 'docker exec -it <container> bash' to work inside it."
    exec tail -f /dev/null
}

verify_repo() {
    tc_echo "Verifying repository files are present in $REPO_DIR"
    for path in "${REQUIRED_PATHS[@]}"; do
        if [ ! -e "$path" ]; then
            tc_echo "Missing required file or directory: $path" >&2
            exit 1
        fi
    done
    tc_echo "All required files found."
}

run_build_pipeline() {
    verify_repo
    cd "$REPO_DIR"
    tc_echo "Running locked, non-interactive MZA release pipeline"
    bash "$RELEASE_SCRIPT" "$@"
    tc_echo "Release pipeline completed successfully. Keeping container alive for further work."
    wait_for_work
}

if [ "$#" -gt 0 ]; then
    case "$1" in
        build|release|bundle)
            shift
            run_build_pipeline "$@"
            ;;
        shell|bash)
            shift
            exec bash "$@"
            ;;
        exec)
            shift
            exec "$@"
            ;;
        *)
            exec "$@"
            ;;
    esac
fi

verify_repo
wait_for_work
