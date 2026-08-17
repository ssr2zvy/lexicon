#!/usr/bin/env bash
set -e

ROOT_DIR="${ROOT_DIR:-$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)}"
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ARTIFACT_FILE="${ARTIFACT_FILE:-$SCRIPT_DIR/../build_and_bundle/mza_artifacts.toml}"
WORKSPACE_CARGO_FILE="${WORKSPACE_CARGO_FILE:-$ROOT_DIR/Cargo.toml}"
LOCAL_BUILD_VARIABLES_FILE="$SCRIPT_DIR/local_build_variables.sh"

# toml_value FILE TABLE_HEADER MATCH_KEY MATCH_VALUE FIELD
# Scans FILE for a [[TABLE_HEADER]] block whose MATCH_KEY equals MATCH_VALUE
# (or, if MATCH_KEY is empty, the first such block) and prints FIELD's value.
toml_value() {
    local file=$1 table=$2 match_key=$3 match_value=$4 field=$5
    awk -v table="[[$table]]" -v match_key="$match_key" -v match_value="$match_value" -v field="$field" '
        function trim(s) { gsub(/^[ \t]+|[ \t]+$/, "", s); return s }
        function unquote(s) { gsub(/^"|"$/, "", s); return s }
        /^\[\[.*\]\]$/ {
            if (in_block && (match_key == "" || matched)) { print values[field]; done = 1; exit }
            in_block = ($0 == table)
            matched = (match_key == "")
            delete values
            next
        }
        in_block && /=/ {
            line = trim($0)
            split(line, kv, "=")
            key = trim(kv[1])
            value = trim(substr(line, index(line, "=") + 1))
            value = unquote(value)
            values[key] = value
            if (match_key != "" && key == match_key && value == match_value) { matched = 1 }
        }
        END {
            if (!done && in_block && (match_key == "" || matched)) { print values[field] }
        }
    ' "$file"
}

BUNDLE_VERSION=$(awk '
    /^\[workspace\.package\]$/ { in_block = 1; next }
    /^\[/ { in_block = 0 }
    in_block && /^version[ \t]*=/ {
        line = $0
        gsub(/^[^=]*=[ \t]*"|"[ \t]*$/, "", line)
        print line
        exit
    }
' "$WORKSPACE_CARGO_FILE")

if [ -z "$BUNDLE_VERSION" ]; then
    echo "Could not determine workspace package version from Cargo.toml" >&2
    exit 1
fi

BUNDLE_PROTOCOL=$(toml_value "$ARTIFACT_FILE" "bundle" "label" "lexicon_bundle" "protocol")
BUNDLE_TYPE=$(toml_value "$ARTIFACT_FILE" "bundle" "label" "lexicon_bundle" "type")

if [ -z "$BUNDLE_PROTOCOL" ] && [ -z "$BUNDLE_TYPE" ]; then
    echo "Could not find a bundle entry labeled lexicon_bundle in mza_artifacts.toml" >&2
    exit 1
fi

TARGET_OS=$(toml_value "$ARTIFACT_FILE" "target" "" "" "os")
TARGET_ARCH=$(toml_value "$ARTIFACT_FILE" "target" "" "" "arch")
TARGET_ENV=$(toml_value "$ARTIFACT_FILE" "target" "" "" "environment")

if [ -z "$TARGET_OS" ] || [ -z "$TARGET_ARCH" ]; then
    echo "Could not determine target triple from mza_artifacts.toml" >&2
    exit 1
fi

if [ -n "$TARGET_ENV" ]; then
    BUNDLE_TARGET="$TARGET_ARCH-unknown-$TARGET_OS-$TARGET_ENV"
else
    BUNDLE_TARGET="$TARGET_ARCH-unknown-$TARGET_OS"
fi

cat > "$LOCAL_BUILD_VARIABLES_FILE" <<EOF
BUNDLE_VERSION=$BUNDLE_VERSION
BUNDLE_PROTOCOL=$BUNDLE_PROTOCOL
BUNDLE_TYPE=$BUNDLE_TYPE
BUNDLE_TARGET=$BUNDLE_TARGET
EOF

chmod +x "$LOCAL_BUILD_VARIABLES_FILE"
