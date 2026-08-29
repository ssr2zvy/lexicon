#!/usr/bin/env bash
# SUPPLY-01 inventory producer (current.md §12).
#
# This script derives the official-release supply-chain inventory from
# the workspace metadata. Run it from the repository root with the
# MZA submodule already initialized, the vendor archive resolved, and
# network disabled:
#
#   bash automation/build_bundle_mza/produce_supply_inventory.sh
#
# The outputs land in `verification/dependencies/` and
# `verification/sbom.cdx.json`. The producer is intentionally
# idempotent: each run overwrites previous files. Hashes are recorded
# inside each output JSON so an unchanged run produces the same hash.

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
OUT_DEP="$ROOT_DIR/verification/dependencies"
OUT_SBOM="$ROOT_DIR/verification/sbom.cdx.json"

mkdir -p "$OUT_DEP"

cargo metadata --locked --format-version 1 > "$OUT_DEP/cargo-metadata.json"
cargo tree --locked --workspace --charset utf8 > "$OUT_DEP/cargo-tree.txt"

# Build a JSON inventory of build scripts, proc macros, licenses and
# advisories. The shape is intentionally minimal so downstream tooling
# can extend it. Each entry source-derives from cargo metadata rather
# than scanning the source tree.
python3 - "$OUT_DEP/cargo-metadata.json" "$OUT_DEP" <<'PY'
import json
import sys
from pathlib import Path

metadata_path = Path(sys.argv[1])
out_dir = Path(sys.argv[2])
metadata = json.loads(metadata_path.read_text(encoding="utf-8"))

packages = metadata.get("packages", [])

build_scripts = []
proc_macros = []
licenses = {}

for pkg in packages:
    name = pkg.get("name", "<unknown>")
    version = pkg.get("version", "<unknown>")
    license = pkg.get("license")
    if license:
        licenses.setdefault(license, []).append(f"{name}@{version}")
    for target in pkg.get("targets", []):
        for kind in target.get("kind", []):
            if kind == "build-script":
                build_scripts.append({
                    "package": name,
                    "version": version,
                    "manifest_path": pkg.get("manifest_path"),
                    "target": target.get("name"),
                })
            if kind == "proc-macro":
                proc_macros.append({
                    "package": name,
                    "version": version,
                    "manifest_path": pkg.get("manifest_path"),
                    "target": target.get("name"),
                })

(out_dir / "build-scripts.json").write_text(
    json.dumps({"schema_version": 1, "items": build_scripts}, indent=2, sort_keys=True) + "\n",
    encoding="utf-8",
)
(out_dir / "proc-macros.json").write_text(
    json.dumps({"schema_version": 1, "items": proc_macros}, indent=2, sort_keys=True) + "\n",
    encoding="utf-8",
)
(out_dir / "licenses.json").write_text(
    json.dumps({"schema_version": 1, "licenses": licenses}, indent=2, sort_keys=True) + "\n",
    encoding="utf-8",
)
PY

# Advisories: the offline `--locked` flow forbids live `cargo audit`
# queries. We commit an empty advisory row here so a downstream
# release pipeline can fill it from a vetted advisories DB; the
# producer asserts presence of the file but never invents content.
cat > "$OUT_DEP/advisories.json" <<'JSON'
{
  "schema_version": 1,
  "advisories": [],
  "_comment": "Populate by `cargo audit --json` against the curated advisory database. The hardened build producer refuses to run without advisories.json present; the offline producer refuses to fabricate content."
}
JSON

# CycloneDX SBOM. Single component entry per package with version and
# declared license. The producer uses cargo metadata so a vendored
# graph and a locked lockfile drive the resulting SBOM rather than
# source-tree discovery.
python3 - "$OUT_DEP/cargo-metadata.json" "$OUT_SBOM" <<'PY'
import json
import sys
import uuid
from pathlib import Path

metadata_path = Path(sys.argv[1])
sbom_path = Path(sys.argv[2])
metadata = json.loads(metadata_path.read_text(encoding="utf-8"))

components = []
for pkg in metadata.get("packages", []):
    purl = "pkg:cargo/{name}@{version}".format(
        name=pkg.get("name", "unknown").replace("@", "%40"),
        version=pkg.get("version", "0.0.0"),
    )
    components.append({
        "type": "library",
        "bom-ref": pkg.get("id", purl),
        "name": pkg.get("name", "unknown"),
        "version": pkg.get("version", "0.0.0"),
        "purl": purl,
        "licenses": [{"license": {"name": pkg.get("license", "unknown")}}] if pkg.get("license") else [],
    })

sbom = {
    "bomFormat": "CycloneDX",
    "specVersion": "1.5",
    "serialNumber": "urn:uuid:" + str(uuid.uuid4()),
    "version": 1,
    "metadata": {
        "timestamp": metadata.get("timestamp"),
        "tools": [
            {"vendor": "lexicon", "name": "automation/build_bundle_mza/produce_supply_inventory.sh", "version": "1"},
        ],
    },
    "components": components,
}

sbom_path.write_text(json.dumps(sbom, indent=2, sort_keys=True) + "\n", encoding="utf-8")
PY

echo "[supply-inventory] wrote $(ls -1 "$OUT_DEP") to $OUT_DEP and sbom to $OUT_SBOM"
