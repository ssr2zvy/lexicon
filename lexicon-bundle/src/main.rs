// MZA Protocol 1 release construction adapter (specs.md §41).
// Consumes generated Rust provided by MZA via MZA_BUNDLE_INPUTS.
include!(env!("MZA_BUNDLE_INPUTS"));

fn main() {
    println!("[[LEXICON-BUNDLE]] Lexicon release package (MZA Protocol 1)");
}
