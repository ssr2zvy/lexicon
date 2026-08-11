#!/usr/bin/env bash
# process-data.sh — the single outward-facing CLI for the process-data phase.
# Delegates to run.py (the phase's one entry point), which validates the
# source, chdirs to its root and drives the engine. Universal options
# (--bg, --abandon-recent-failed) and source-specific options are all
# passed through; see common/process-data/process_data.py for the contract.
#
# Usage: common/process-data/process-data.sh <source> [options...]
set -euo pipefail
exec python3 "$(dirname "$0")/run.py" "$@"
