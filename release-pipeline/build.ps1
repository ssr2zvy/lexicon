$ReleasePipelineDirLocation = $PSScriptRoot

function log {
    param([string]$Message)
    "[BUILD-LEXICON] $Message"
}
function log_requirements {
    param([string]$Message)
    log "[requirements] $Message"
}
function log_interface {
    param([string]$Message)
    log "[interface] $Message"
}

function verify_requirements {
    log_requirements "Verifying requirements (Zig, Rust, cargo-zigbuild)"

    function verify_cargo {
        if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) {
            log_requirements 'Required Rust CLI command "cargo" could not be found'
            return $false
        }
        return $true
    }

    function verify_zig {
        if (-not (Get-Command zig -ErrorAction SilentlyContinue)) {
            log_requirements 'Required Zig CLI command "zig" could not be found'
            return $false
        }
        return $true
    }

    function install_cargozigbuild {
        cargo install cargo-zigbuild | Out-Host
        if ($LASTEXITCODE -ne 0) {
            log_requirements 'Failed to install "cargo-zigbuild"'
            return $false
        }
        return $true
    }

    function verify_cargozigbuild {
        param([string]$CargoVerifiedCheck)
        if ($CargoVerifiedCheck -ne "cargo") {
            log_requirements 'Required Rust module "cargo-zigbuild" cannot be installed due to Rust not be installed.'
            return $false
        }
        cargo-zigbuild --version *> $null
        if ($LASTEXITCODE -ne 0) {
            log_requirements 'Required Rust module "cargo-zigbuild" is not installed'
            while ($true) {
                $prompt = log_requirements 'Do you want to install it now? (y/n)'
                $response = Read-Host -Prompt $prompt
                switch -Regex ($response) {
                    '^(y|yes)$' {
                        if (-not (install_cargozigbuild)) {
                            return $false
                        }
                        return $true
                    }
                    '^(n|no)$' {
                        return $false
                    }
                    default {
                        log_requirements "Invalid response. Answer (y/n)"
                    }
                }
            }
        }
        return $true
    }

    $requirementsMet = @()
    if (verify_cargo) { $requirementsMet += "cargo" }
    if (verify_zig) { $requirementsMet += "zig" }
    if (verify_cargozigbuild -CargoVerifiedCheck $requirementsMet[0]) { $requirementsMet += "cargo-zigbuild" }

    if ($requirementsMet.Count -lt 3) {
        log_requirements "Aborting build process due to unmet requirements"
        return $false
    }
    else {
        log_requirements "Requirements met"
        return $true
    }
}

function interface {
    param([string[]]$Arguments)
    log_interface "Executing Rust interface"

    function entry {
        param([string[]]$Arguments)
        cargo run --manifest-path (Join-Path $ReleasePipelineDirLocation "build/Cargo.toml") -- @Arguments | Out-Host
        return ($LASTEXITCODE -eq 0)
    }

    if (entry -Arguments $Arguments) {
        log_interface "Rust interface successful"
        return $true
    }
    else {
        log_interface "Rust interface failed"
        return $false
    }
}

log "Starting build process"
if (-not (verify_requirements)) { exit 1 }
if (-not (interface -Arguments $args)) { exit 1 }
log "Build process completed"
