use std::ffi::OsString;

use lexicon_core::http::{AcquisitionResult, HttpAcquisitionContext, HttpSourceContractV1};

fn acquire(_args: &[OsString], _context: &mut HttpAcquisitionContext) -> AcquisitionResult<()> {
    Ok(())
}

fn main() {
    let _ = HttpSourceContractV1::new(acquire);
}
