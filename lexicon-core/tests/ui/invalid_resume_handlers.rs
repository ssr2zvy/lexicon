use std::ffi::OsString;

use lexicon_core::http::{AcquisitionResult, HttpAcquisitionContext, HttpSourceContractV1};

fn acquire(
    _context: &mut HttpAcquisitionContext,
    _args: &[OsString],
) -> AcquisitionResult<()> {
    Ok(())
}

async fn async_resume(
    _context: &mut HttpAcquisitionContext,
    _args: &[OsString],
) -> AcquisitionResult<()> {
    Ok(())
}

fn context_by_value_resume(
    _context: HttpAcquisitionContext,
    _args: &[OsString],
) -> AcquisitionResult<()> {
    Ok(())
}

fn immutable_context_resume(
    _context: &HttpAcquisitionContext,
    _args: &[OsString],
) -> AcquisitionResult<()> {
    Ok(())
}

fn missing_osstring_slice_resume(
    _context: &mut HttpAcquisitionContext,
    _args: &[String],
) -> AcquisitionResult<()> {
    Ok(())
}

fn reversed_parameters_resume(
    _args: &[OsString],
    _context: &mut HttpAcquisitionContext,
) -> AcquisitionResult<()> {
    Ok(())
}

fn bool_return_resume(
    _context: &mut HttpAcquisitionContext,
    _args: &[OsString],
) -> bool {
    true
}

fn string_result_resume(
    _context: &mut HttpAcquisitionContext,
    _args: &[OsString],
) -> Result<(), String> {
    Ok(())
}

fn main() {
    let _ = HttpSourceContractV1::new(acquire).with_resume(async_resume);
    let _ = HttpSourceContractV1::new(acquire).with_resume(context_by_value_resume);
    let _ = HttpSourceContractV1::new(acquire).with_resume(immutable_context_resume);
    let _ = HttpSourceContractV1::new(acquire).with_resume(missing_osstring_slice_resume);
    let _ = HttpSourceContractV1::new(acquire).with_resume(reversed_parameters_resume);
    let _ = HttpSourceContractV1::new(acquire).with_resume(bool_return_resume);
    let _ = HttpSourceContractV1::new(acquire).with_resume(string_result_resume);
}
