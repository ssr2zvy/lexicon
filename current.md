# Implementation report

Implemented the runtime invocation argv transport and source-argument splitting required by the current request.

## What changed
- Added the canonical transport constants and size guard in `lexicon-core/src/runtime/invocation_transport.rs`:
  - `RUNTIME_INVOCATION_ARGUMENT = "--lexicon-invocation-v1"`
  - `RUNTIME_SOURCE_ARGUMENT_DELIMITER = "--"`
  - `MAX_RUNTIME_INVOCATION_ENVELOPE_JSON_BYTES = 16 * 1024`
- Added the encoded/parsed transport API for parent-side encoding and child-side parsing, while preserving all native source `OsString` values after the mandatory delimiter.
- Exported the new API through `lexicon_core::runtime` so the runtime module exposes the transport types and functions consistently.
- Rejected malformed invocation transport layouts and probe mode before any source-specific argument interpretation.

## Validation
- `cargo test --workspace --quiet` ✅
Typed encoding error

Define:

#[derive(Debug)]
pub enum RuntimeInvocationTransportEncodingError {
    Envelope(
        RuntimeInvocationEncodingError,
    ),
    EnvelopeTooLarge {
        maximum: usize,
        actual: usize,
    },
}

Equivalent naming is acceptable.

Typed decoding error

Define:

#[derive(Debug)]
pub enum RuntimeInvocationTransportDecodingError {
    MissingInvocationArgument,
    UnexpectedInvocationArgument {
        actual: OsString,
    },
    MissingEnvelope,
    EnvelopeNotUtf8,
    EnvelopeTooLarge {
        maximum: usize,
        actual: usize,
    },
    MissingDelimiter,
    UnexpectedDelimiter {
        actual: OsString,
    },
    Envelope(
        RuntimeInvocationDecodingError,
    ),
}

Equivalent typed organization is acceptable, but callers must distinguish:

* missing or wrong invocation argument;
* missing envelope;
* non-UTF-8 envelope;
* oversized envelope;
* missing or misplaced delimiter;
* envelope decoding failure.

Implement:

std::fmt::Display
std::error::Error

Use source() for nested envelope errors.

Do not return plain String, print arguments, or exit.

Sensitive error handling

Error formatting must not include:

* serialized envelope JSON;
* project identity;
* session identity;
* source argument values;
* raw non-UTF-8 bytes.

An error may identify the failed structural position without echoing its contents.

If the typed error retains an unexpected OsString, its Display implementation must not print that value.

No handler execution

Encoding and parsing must not:

* invoke acquisition;
* invoke resume;
* invoke processing;
* construct acquisition or processing contexts;
* create sessions;
* access the filesystem;
* launch a process.

Required tests

Add tests proving:

1. Acquisition/run encodes into the exact three-element internal prefix.
2. Acquisition/resume encodes correctly.
3. Processing/run encodes correctly.
4. No source arguments still produces the mandatory delimiter.
5. Ordinary source arguments are appended after the delimiter.
6. Parsing recovers the original acquisition envelope.
7. Parsing recovers the original processing envelope.
8. Parsing preserves source argument order.
9. Empty source argument values are preserved.
10. Duplicate source arguments are preserved.
11. A source argument equal to -- is preserved after the delimiter.
12. A source argument equal to the invocation flag is preserved after the delimiter.
13. A source argument equal to the probe flag is preserved after the delimiter.
14. Unicode source arguments round-trip.
15. Non-UTF-8 Unix source arguments round-trip byte-for-byte.
16. Empty input is rejected.
17. Wrong first argument is rejected.
18. Probe mode is rejected.
19. Missing envelope is rejected.
20. Non-UTF-8 envelope is rejected.
21. Oversized envelope is rejected during encoding.
22. Oversized envelope is rejected during parsing.
23. Missing delimiter is rejected.
24. Wrong delimiter position is rejected.
25. Extra internal values before the delimiter are rejected.
26. Invalid envelope JSON returns the nested typed error.
27. Processing/resume remains rejected through envelope decoding.
28. Error display does not reveal envelope JSON.
29. Error display does not reveal source arguments.
30. Encoding and parsing invoke no source handler.
31. Existing envelope JSON tests remain unchanged.
32. Existing runtime-information probe tests remain unchanged.
33. All workspace tests pass repeatedly.

Preserve existing behavior

Do not change:

* invocation-envelope JSON;
* in-memory envelope validation;
* runtime-information probing;
* source descriptors;
* runtime identities;
* hashing;
* verification;
* manifests;
* staging;
* bundle admission;
* paired publication;
* source scaffolding;
* source create;
* source build;
* Cargo invocation;
* CLI behavior;
* MZA;
* Protocol 1;
* lexicon-bundle;
* installer behavior;
* bundle inputs;
* installed paths.

lexicon-framework remains library-only.

lexicon-bundle remains a binary installer compiled through cargo-bundler-v0.1.0.

Validation

Run:

cargo test -p lexicon-core --quiet

Run the workspace suite twice:

cargo test --workspace --quiet
cargo test --workspace --quiet

If the external MZA checkout is available, run:

bash automation/build_bundle_install/build_bundle_install.sh

If unavailable, report the known external blocker separately. Do not modify MZA or installer code.

Explicit exclusions

Do not implement:

* subprocess launching;
* child runtime admission;
* envelope files;
* environment-variable transport;
* project-path transport;
* source-handler selection;
* resume-handler availability validation;
* managed runner generation;
* runner main.rs;
* runner::run;
* acquisition execution;
* processing execution;
* session creation or locking;
* HTTP transport;
* raw transaction recording;
* SQLite behavior;
* foreground supervision;
* background supervision;
* __operator-host;
* source build integration.

Completion report

After completion, replace current.md with a report containing:

* files created and changed;
* canonical reserved argument constants;
* envelope size limit;
* encoding and parsing APIs;
* exact argv layout;
* typed encoding and decoding errors;
* malformed transport rejection results;
* source-argument preservation results;
* non-UTF-8 Unix round-trip result;
* probe/invocation separation;
* confirmation that error messages do not reveal arguments or envelope contents;
* proof that no source handler was invoked;
* Core and repeated workspace test results;
* bundle/install result or the known external-MZA blocker.

Then stop. Do not launch or execute a managed runtime.