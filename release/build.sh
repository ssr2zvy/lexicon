#!/usr/bin/env bash

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

cargo build \
    --manifest-path "$ROOT/Cargo.toml" \
    --release

mkdir -p "$ROOT/artifacts"

cp "$ROOT/target/release/lexicon-cli" "$ROOT/artifacts/lexicon-cli"
cp "$ROOT/target/release/lexicon-framework" "$ROOT/artifacts/lexicon-framework"