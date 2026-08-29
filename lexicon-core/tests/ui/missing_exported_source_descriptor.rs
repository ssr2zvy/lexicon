// Reproduces the boundary failure that occurs when a managed source library
// declares its descriptor but does not expose it as `pub`. A managed runner
// imports the source library's `pub const SOURCE`. Without `pub`, the
// compile error at this exact site is the boundary failure the contract
// promises.
//
// The actual production boundary lives in the generated
// `<source>/<protocol>/<operation>/lexicon-runner/src/main.rs`, which does
// `use <impl-crate>::SOURCE;`. The fixture exercises the same compile site.

mod source_library {
    // Note: in production this would be:
    //   pub const SOURCE: HttpSourceContractV1 = HttpSourceContractV1::new(handler);
    // `pub` is the boundary. Omitting it is the missing-exported-descriptor
    // case this fixture covers.
    const SOURCE: u32 = 42;
}

fn main() {
    // Runner-main equivalent: import the source library's descriptor.
    let _ = source_library::SOURCE;
}
