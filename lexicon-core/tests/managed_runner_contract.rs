// DOC-02 boundary contract test for the managed runner (trybuild driver).
//
// Verifies the actual compile-fail boundary the contract promises:
//
// - A managed source library must export `pub const SOURCE` so the
//   generated runner can `use <impl-crate>::SOURCE` at its main entrypoint.
// - When `pub` (or the descriptor itself) is absent, the runner's compile
//   fails at exactly that import site.
//
// The fixture `tests/ui/missing_exported_source_descriptor.rs` simulates the
// boundary in a single trybuild file: it declares a const inside a module
// without `pub`, then attempts to import it from outside. The compile error
// at this site is the same boundary failure the production runner-template
// experiences against a real source library whose `SOURCE` is not exported.

#[test]
fn missing_exported_source_descriptor_in_runner_boundary_fails_compile() {
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/ui/missing_exported_source_descriptor.rs");
}
