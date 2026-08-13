#!/usr/bin/env bash

RELEASEPIPELINE_DIR_LOCATION="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

log() {
    echo "[BUILD-LEXICON] $*"
}
log_requirements() {
    log "[requirements] $*"
}
log_interface() {
    log "[interface] $*"
}

verify_requirements() {
    log_requirements "Verifying requirements (Zig, Rust, cargo-zigbuild)"
    verify_cargo() {
        if ! command -v cargo >/dev/null 2>&1 ; then
            log_requirements "Required Rust CLI command \"cargo\" could not be found"
            return 1
        fi
    }
    verify_zig() {
        if ! command -v zig >/dev/null 2>&1 ; then
            log_requirements "Required Zig CLI command \"zig\" could not be found"
            return 1
        fi
    }
    install_cargozigbuild() {
        cargo install cargo-zigbuild || {
        log_requirements "Failed to install \"cargo-zigbuild\""
        return 1
        }
    }
    verify_cargozigbuild() {
        cargo_verified_check=$1
        if [ "$cargo_verified_check" != "cargo" ]; then
            log_requirements "Required Rust module \"cargo-zigbuild\" cannot be installed due to Rust not be installed."
            return 1
        fi
        if ! cargo-zigbuild --version >/dev/null 2>&1 ; then
            log_requirements "Required Rust module \"cargo-zigbuild\" is not installed"
            while true; do
                read -r -p "$(log_requirements "Do you want to install it now? (y/n) ")" response
                case "$response" in
                    [yY][eE][sS]|[yY]) 
                        install_cargozigbuild || return 1
                        break
                        ;;
                    [nN][oO]|[nN]) 
                        return 1
                        ;;
                    *) 
                        log_requirements "Invalid response. Answer (y/n)"
                        ;;
                esac
            done
        fi
    }   
    declare -a requirements_met
    verify_cargo && requirements_met[0]="cargo"
    verify_zig && requirements_met+=("zig")
    verify_cargozigbuild "${requirements_met[0]}" && requirements_met+=("cargo-zigbuild")
    if [ "${#requirements_met[@]}" -lt 3 ]; then
        log_requirements "Aborting build process due to unmet requirements"
        return 1
    else
        log_requirements "Requirements met"
    fi
}   

interface() {
    log_interface "Executing Rust interface"
    entry() {
        cargo run \
            --manifest-path "$RELEASEPIPELINE_DIR_LOCATION/build/Cargo.toml" \
            -- "$@"
    }
    if entry "$@"; then
        log_interface "Rust interface successful"
    else
        log_interface "Rust interface failed"
        exit 1
    fi
}

log "Starting build process"
verify_requirements || exit 1
interface "$@" || exit 1
log "Build process completed"