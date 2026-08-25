# Implementation report

Implemented the Core-owned runtime-information probe handler and exported it as `lexicon_core::http::runner`.

## What changed
- Added `lexicon-core/src/protocols/http/runner.rs` with the reserved probe constant, outcome enum, typed error enum, and `try_write_runtime_information_probe` function.
- Ensured the probe only activates when the first argument is exactly `--lexicon-runtime-information-v1`, returns `UnexpectedArguments` for extra values, and ignores later-position matches.
- Constructed the runtime document from `RuntimeIdentity`, `HttpSourceContractV1`, and the available capability set without invoking acquire/resume or capability validation.
- Wrote the JSON document plus a single trailing newline and flushed the supplied writer.
- Exported the module via `lexicon-core/src/protocols/http/mod.rs` so the public path is `lexicon_core::http::runner`.
- Added targeted tests for success cases, no-op cases, output content/newline requirements, capability preservation, resume-registration preservation, write/flush error handling, and non-UTF-8 Unix arguments.

## Validation
- `cargo test -p lexicon-core --quiet`
- `cargo test --workspace --quiet`

Both passed.

## Optional MZA bundle/install check
- Attempted: `bash automation/build_bundle_install/build_bundle_install.sh`
- Result: failed because the external MZA dependency is not present in this environment (`/home/runner/work/lexicon/lexicon/automation/build_bundle_install/../build_bundle_mza/mza/make-artifact.sh: No such file or directory`).
- No MZA or installer code was modified.


Do not implement:

* runner::run;
* a generated runner;
* runner main.rs;
* source workspace migration;
* process exit-code mapping;
* acquisition execution;
* resume execution;
* invocation envelopes;
* parent-side subprocess probing;
* probe timeouts;
* build-time probe validation;
* runtime admission;
* runtime.json;
* executable hashing;
* publication changes;
* HTTP transport;
* transaction recording;
* sessions;
* supervision;
* __operator-host;
* processing runtime probing.

Completion report

After completion, replace current.md with a report containing:

* files created and changed;
* the reserved probe argument;
* the exact probe API;
* outcome and error representations;
* argument-recognition behavior;
* output and newline behavior;
* write and flush failure results;
* required and available capability results;
* proof that incompatible metadata can still be reported;
* proof that acquisition and resume handlers were not invoked;
* non-UTF-8 argument behavior;
* Core and workspace test results;
* bundle/install result or the known external-MZA blocker.

Then stop. Do not generate or execute a managed runner.