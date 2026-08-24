ROOT_DIR="${ROOT_DIR:-$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)}"
LEXICON_CLI_DIR="${LEXICON_CLI:-$ROOT_DIR/lexicon-cli}"
LEXICON_BUNDLE_DIR="${LEXICON_BUNDLE:-$ROOT_DIR/lexicon-bundle}"

cargo generate-lockfile --manifest-path "$LEXICON_CLI_DIR/Cargo.toml"
cargo generate-lockfile --manifest-path "$LEXICON_BUNDLE_DIR/Cargo.toml"
