#!/usr/bin/env bash
set -euo pipefail

LEXICON_ROOT="${LEXICON_ROOT:-/lexicon}"
REPO_DIR="${REPO_DIR:-$LEXICON_ROOT}"

REQUIRED_PATHS=(
    "$REPO_DIR/Cargo.toml"
    "$REPO_DIR/lexicon-install.toml"
    "$REPO_DIR/lexicon-cli"
    "$REPO_DIR/lexicon-bundle"
    "$REPO_DIR/lexicon-framework"
    "$REPO_DIR/automation/build_bundle_install/build_bundle_install.sh"
    "$REPO_DIR/automation/build_bundle_install/get_build_variables.sh"
    "$REPO_DIR/automation/build_bundle_install/install.sh"
    "$REPO_DIR/automation/build_and_bundle/mza"
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
    tc_echo "Running build, bundle, install pipeline"
    bash "$REPO_DIR/automation/build_bundle_install/build_bundle_install.sh" "$@"
    tc_echo "Build pipeline completed successfully. Keeping container alive for further work."
    wait_for_work
}

if [ "$#" -gt 0 ]; then
    case "$1" in
        build|bundle)
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
