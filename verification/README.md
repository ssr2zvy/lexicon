# Verification Manifest Format

This file specifies the durable evidence record produced by the Lexicon
conformance workflow (`.github/workflows/conformance.yml`). Every green
Lexicon build must emit a `verification-manifest.json` whose structure
matches this schema; the final conformance job treats the matrix step
as a hard prerequisite.

The schema complements the policy in [`workspace/specs/release-policy.md`](../workspace/specs/release-policy.md).
A number in `current.md` (or its successor) without the attached
verification manifest is **not** evidence.

## Schema (v1)

```json
{
  "schema_version": 1,
  "repository": "https://github.com/ssr2zvy/lexicon",
  "commit": "<40-character sha>",
  "dirty": false,
  "os": "linux-x86_64 | windows-x86_64",
  "architecture": "x86_64 | aarch64",
  "rustc": "rustc 1.98.0 (xxxx 2026-xx-xx)",
  "cargo": "cargo 1.98.0 (xxxx 2026-xx-xx)",
  "mza_commit": "<pinned MZA sha or empty if not yet signed>",
  "commands": [
    {
      "argv": ["cargo", "check", "--workspace", "--locked"],
      "exit_code": 0,
      "started_at": "RFC3339 timestamp",
      "finished_at": "RFC3339 timestamp",
      "stdout_sha256": "<sha256>",
      "stderr_sha256": "<sha256>"
    }
  ],
  "tests": {
    "passed": <int>,
    "failed": 0,
    "ignored": 0,
    "outer_workflow_skipped": 0
  },
  "artifacts": [
    "verification/verification-manifest.json",
    "verification/run.log"
  ]
}
```

### Field semantics

- `schema_version` is `1` until a future intentional bump. Bumps are
  incompatible and require a synchronized `current.md` update.
- `commit` is the exact 40-character commit SHA the workflow ran against.
  Branch protection requires this SHA to match `refs/heads/main` on a
  merge.
- `dirty` is the literal `false` only when the merge commit is the exact
  worktree; any uncommitted change forces `true` and stops the gate.
- `os` and `architecture` describe the CI runner. Cross-compiled
  artifacts require a follow-up native run before promotion.
- `rustc` and `cargo` capture the runner's actual toolchain version. The
  workflow pins a single toolchain via `dtolnay/rust-toolchain@<sha>`;
  the version strings here are the verbatim `rustc -V` / `cargo -V`
  output.
- `mza_commit` is the pin from MZA's accepted installer API; until MZA
  publishes that API the field is empty string and `current.md` §3
  keeps the master milestone open as the audit instructs.
- `commands[].argv` is the exact argument vector the workflow invoked;
  `passed_at` and `finished_at` are RFC3339 timestamps recorded by the
  workflow at the moment of the OS exec boundary.
- `commands[].stdout_sha256` and `commands[].stderr_sha256` are SHA-256
  hashes of the captured raw bytes (UTF-8 preserved); they let
  reviewers compare against the artifact's raw content.
- `tests.passed`/`failed`/`ignored` are the cargo-test rollups from
  the run. `outer_workflow_skipped` counts platform skips the outer
  workflow reports — non-zero is a hard failure (a Rust test must not
  print "skipped" and return success; see `current.md` §65).
- `artifacts` lists manifest-adjacent files the workflow uploaded
  (`verification/verification-manifest.json` itself, plus the raw run
  log on Linux at `verification/run.log`).

### Retention

- A green run uploads the manifest via `actions/upload-artifact@v4`
  and keeps it bounded to the workflow retention window (default 90
  days). Adjust workspace retention after promotion to release.
- Release promotion requires the latest green run's manifest URL to
  link alongside the SBOM and audit ledger.
- A red run does **not** upload a manifest; CI records a typed error
  in the workflow summary.

### Native Windows evidence

Per `current.md` §CI-01, cross-compilation is not promoted to release
status. The `windows-native` job runs `cargo check`, `cargo test`,
`background_handoff`, `foreground_cancellation`, and
`windows_runtime_replacement` on `windows-latest`. Native output is
emitted as a UTF-8 JSON document (the workflow writes through
`Out-File -Encoding utf8` / `ConvertTo-Json | Set-Content`); the matrix
check fails if any of those artifacts is missing or if its schema
validation rejects the run.
