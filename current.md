# Implementation report

Implemented the HTTP capability compatibility model and validation flow.

What changed
- Extended `RuntimeInformationV1` to track both `required_capabilities` and `available_capabilities` independently.
- Updated `from_http_source(...)` to require the runtime’s actual available capability set as an input parameter.
- Added `available_capabilities()` accessors and a pure `validate_capabilities()` check that returns `MissingHttpCapabilities` without invoking acquisition or resume handlers.
- Added the capability-set operations `is_subset_of(...)` and `missing_from(...)` on `HttpCapabilitySet` as allocation-free, const-friendly helpers.
- Added `MissingHttpCapabilities` with a typed `missing()` accessor.
- Extended the JSON runtime document to include `runtime.available_capabilities` and kept the schema version at 1.
- Updated deserialization to reject missing runtime data, unknown fields, unknown capability values, and duplicate entries while still accepting structurally valid but incompatible documents for later validation.
- Kept runtime capability availability separate from source requirements; no production code claims `ClientCertificateV1` is available unless explicitly set.

Validation
- Ran: `cargo test --workspace --quiet`
- Result: all workspace tests passed.

