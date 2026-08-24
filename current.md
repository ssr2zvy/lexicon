# Implementation report

## Completed work

This pass completed the two focused corrections requested for the source-build path:

1. Randomized runtime staging files now use a tempfile-managed unique path inside the final runtime directory, instead of a PID-only staging name.
2. Unsupported-protocol validation now uses the neutral message for both source create and source build operations:

   [lexicon] ERROR: unsupported protocol 'browser'; only 'http' is currently supported

## What changed

- In [lexicon-framework/src/main.rs](lexicon-framework/src/main.rs), the runtime staging file allocation now creates a randomized path with a prefix based on the executable name and writes the compiled binary into that runtime-local staging file before publication.
- The staging behavior remains same-filesystem and preserves the transactional backup/restore flow already in place.
- The shared protocol validation for unsupported values is now operation-neutral and no longer claims a source-creation error on build failures.
- The related checks in [lexicon-cli/src/cli/source.rs](lexicon-cli/src/cli/source.rs) and [lexicon-cli/src/cli/mod.rs](lexicon-cli/src/cli/mod.rs) were kept aligned with the neutral wording and the runtime behavior.

## Verification performed

I verified the behavior with the required regression checks:

- cargo test -p lexicon-cli -p lexicon-framework -- --nocapture
  - lexicon-cli: 24 passed, 0 failed
  - lexicon-framework: 24 passed, 0 failed
  - lexicon-framework-core: 1 passed, 0 failed
  - doc tests: passed
- bash automation/build_bundle_install/build_bundle_install.sh
  - completed successfully
- Real temp-project validation passed for the supported flow:
  - source create example-source --protocol http
  - source build example-source --protocol http
  - both runtime executables existed and were executable
- Rejected protocol validation also passed:
  - source build example-source --protocol browser exited with status 1
  - exactly one [lexicon] ERROR line was emitted
  - the output contained the neutral message and did not contain source creation
  - runtime executable hashes remained unchanged after the failed command

## Status

The requested source-build staging fix and neutral protocol wording fix are complete and verified.
- Additional protocols.
- Adding protocols to existing sources.
- MZA or bundling changes.

## Required final report

Replace `current.md` completely with one clean report using these sections:

```text
# Final source-build correction report

## Verdict

## Files changed

## Randomized staging implementation

## Staging collision tests

## Staging cleanup and rollback tests

## Protocol error correction

## Unsupported-protocol command evidence

## Supported end-to-end build

## Test results

## Remaining gaps
```

Under `## Verdict`, use exactly one of:

```text
VERIFIED COMPLETE
```

or:

```text
NOT VERIFIED COMPLETE
```

The report must include:

- The exact staging-allocation function changed.
- The exact randomized tempfile API used.
- Exact staging-related test names.
- Proof that two staging paths differ.
- Proof that an old PID-style file remains untouched.
- Proof that failed and successful operations leave no new staging files.
- The exact corrected unsupported-protocol output.
- The exact unsupported-protocol exit code.
- Proof that existing runtime hashes remain unchanged after rejection.
- Package-specific test totals.
- Any remaining failure or blocker.

Do not append this task after the report.

Do not declare completion unless both focused corrections and every listed regression check pass.