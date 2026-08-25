Implementation report

Old Core location:
- lexicon-framework/core/
- package: lexicon-framework-core

New Core location:
- lexicon-core/
- package: lexicon-core

Work completed:
- created the top-level lexicon-core crate and copied the existing Core implementation into it without changing the runtime behavior or the historical HTTP acquisition API
- updated the workspace manifest to include lexicon-core and removed the legacy lexicon-framework/core workspace member
- added the direct framework dependency on lexicon-core via path = "../lexicon-core"
- kept lexicon-framework as a library-only crate and left the generated source template references to the legacy compatibility package unchanged, as required for this migration step

Validation:
- cargo test --workspace --quiet passed
- cargo metadata confirms the workspace now contains lexicon-cli -> lexicon-framework -> lexicon-core
- the required build_bundle_install script was attempted, but this repo snapshot is missing the referenced helper at automation/build_bundle_install/../build_bundle_mza/mza/make-artifact.sh, so the bundle/install workflow cannot complete in this environment

Notes:
- the top-level Core package name is lexicon-core and the Rust crate name is lexicon_core
- the legacy lexicon-framework/core directory was removed so there is no second checked-in Core copy
