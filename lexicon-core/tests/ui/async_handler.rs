use std::ffi::OsString;

use lexicon_core::http::{AcquisitionResult, HttpAcquisitionContext, HttpSourceContractV1};

async fn acquire(
    _context: &mut HttpAcquisitionContext,
    _args: &[OsString],
) -> AcquisitionResult<()> {
    Ok(())
}

fn main() {
    let _ = HttpSourceContractV1::new(acquire);
}
