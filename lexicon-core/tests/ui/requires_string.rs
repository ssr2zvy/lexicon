use std::ffi::OsString;

use lexicon_core::http::{AcquisitionResult, HttpAcquisitionContext, HttpSourceContractV1};

fn acquire(
    context: &mut HttpAcquisitionContext,
    args: &[OsString],
) -> AcquisitionResult<()> {
    let _ = (context, args);
    Ok(())
}

fn main() {
    let _ = HttpSourceContractV1::new(acquire)
        .requires("client-certificate-v1");
}
