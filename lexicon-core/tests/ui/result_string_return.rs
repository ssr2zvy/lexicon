use std::ffi::OsString;

use lexicon_core::http::{HttpAcquisitionContext, HttpSourceContractV1};

fn acquire(_context: &mut HttpAcquisitionContext, _args: &[OsString]) -> Result<(), String> {
    Ok(())
}

fn main() {
    let _ = HttpSourceContractV1::new(acquire);
}
