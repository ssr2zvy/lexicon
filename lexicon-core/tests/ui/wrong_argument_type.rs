use std::ffi::OsString;

use lexicon_core::http::{AcquisitionResult, HttpAcquisitionContext, HttpSourceContractV1};

fn acquire(_context: &mut HttpAcquisitionContext, _args: &[String]) -> AcquisitionResult<()> {
    Ok(())
}

fn main() {
    let _ = HttpSourceContractV1::new(acquire);
}
