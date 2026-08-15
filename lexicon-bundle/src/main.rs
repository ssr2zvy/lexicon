include!(concat!(env!("OUT_DIR"), "/mza_bundle_inputs.rs"));

fn main() {
    println!("test main lexicon");
    for input in MZA_BUNDLE_INPUTS {
        println!("embedded bundle input: {} ({} bytes)", input.label, input.archive.len());
    }
}
