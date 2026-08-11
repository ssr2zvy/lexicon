#!/usr/bin/env bash
# get-raw-data.sh — the single outward-facing CLI for the get-raw-data phase.
# Delegates to run.py (the phase's one entry point), which validates the
# source, chdirs to its root and drives the engine. Universal options
# (--bg, --abandon-recent-failed) and source-specific options are all
# passed through; see common/get-raw-data/get_raw_data.py for the contract.
#
# Usage: common/get-raw-data/get-raw-data.sh <source> [options...]
set -euo pipefail
exec python3 "$(dirname "$0")/run.py" "$@"
