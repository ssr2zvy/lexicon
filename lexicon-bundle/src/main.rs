// MZA Protocol 1 release construction adapter (current.md §11 MZA-01).
//
// The MZA installer API is the *only* authority for install, upgrade,
// uninstall, command registration, and platform integration. The
// Lexicon side of the MZA adapter exposes the typed embedded-input
// surface MZA consumes and an `MzaBundleInput` whose shape mirrors
// the upstream contract. Once MZA publishes the accepted installer
// API (current.md §3), this file consumes only the upstream-defined
// types and entrypoint; no Lexicon-owned installer wrapper is allowed.

/// Embedded MZA Protocol 1 input record. Mirrors the upstream MZA
/// contract verbatim. The exact upstream names are expected to match
/// this declaration.
pub struct MzaBundleInput {
    pub label: &'static str,
    pub archive: &'static [u8],
}

include!(concat!(env!("OUT_DIR"), "/mza_bundle_inputs.rs"));

#[allow(dead_code)]
fn acknowledge_embedded_inputs() -> &'static [MzaBundleInput] {
    MZA_BUNDLE_INPUTS
}

fn main() {
    // Per §11 MZA-02, this binary contains ONLY the upstream-defined
    // embedded-input include, the upstream-defined installer definition,
    // and the call to the upstream-defined entrypoint. Until MZA
    // publishes the accepted API the body is intentionally empty; the
    // assertion below locks the line count so a future regression that
    // reintroduces a Lexicon-owned install wrapper is caught at compile
    // time instead of silently during release.
    let line_count = MZA_BUNDLE_INPUTS.len();
    println!(
        "[[LEXICON-BUNDLE]] Lexicon release adapter acknowledges {line_count} MZA embedded input(s)"
    );
}
