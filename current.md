# Current status

## Verified contract update
The HTTP acquisition contract has been corrected to the context-based form:

```rust
pub struct HttpAcquisitionContext;

pub trait HttpAcquisition {
    fn acquire(
        &self,
        context: &mut HttpAcquisitionContext,
    ) -> Result<(), String>;
}
```

The shared runtime helper now constructs the context and calls:

```rust
let mut context = HttpAcquisitionContext;
acquisition.acquire(&mut context)
```

The generated HTTP source template in [lexicon-framework/src/main.rs](lexicon-framework/src/main.rs) was updated to implement `acquire` instead of the old `run` method.

## Release/tag status
The portable generated dependency is pinned to the current public tag, not a local checkout path. The generated manifests now target the released tag `v0.1.2`.

## Validation status
Fresh validation was run against a newly initialized external project in `/tmp`:

- `lexicon init my-data-project` succeeded
- `lexicon source new example-source` succeeded
- both generated source crates passed `cargo check`
- generated manifests contained the git-tagged dependency with `tag = "v0.1.2"`
- no `/workspaces/lexicon` path remained in generated manifests

## Current progress
The contract fix and external-project validation are complete. The remaining next behavioral step is to give `HttpAcquisitionContext` its first real operation: making and recording an HTTP request in a concrete runtime flow.
