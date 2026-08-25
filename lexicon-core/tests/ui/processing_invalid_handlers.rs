use std::ffi::OsString;

use lexicon_core::processing::{ProcessingContext, ProcessingResult, ProcessingSourceContractV1};

fn no_parameters() -> ProcessingResult<()> {
    Ok(())
}

fn missing_args(_context: &mut ProcessingContext) -> ProcessingResult<()> {
    Ok(())
}

fn context_by_value(_context: ProcessingContext, _args: &[OsString]) -> ProcessingResult<()> {
    Ok(())
}

fn immutable_context(_context: &ProcessingContext, _args: &[OsString]) -> ProcessingResult<()> {
    Ok(())
}

fn wrong_context_type(_context: &mut String, _args: &[OsString]) -> ProcessingResult<()> {
    Ok(())
}

fn wrong_argument_type(_context: &mut ProcessingContext, _args: &[String]) -> ProcessingResult<()> {
    Ok(())
}

fn mutable_argument_slice(_context: &mut ProcessingContext, _args: &mut [OsString]) -> ProcessingResult<()> {
    Ok(())
}

fn reversed_parameters(_args: &[OsString], _context: &mut ProcessingContext) -> ProcessingResult<()> {
    Ok(())
}

fn bool_return(_context: &mut ProcessingContext, _args: &[OsString]) -> bool {
    true
}

fn string_result(_context: &mut ProcessingContext, _args: &[OsString]) -> Result<(), String> {
    Ok(())
}

async fn async_handler(_context: &mut ProcessingContext, _args: &[OsString]) -> ProcessingResult<()> {
    Ok(())
}

fn function_returning_processing_result_bool(
    _context: &mut ProcessingContext,
    _args: &[OsString],
) -> ProcessingResult<bool> {
    Ok(true)
}

fn main() {
    let _ = ProcessingSourceContractV1::new(no_parameters);
    let _ = ProcessingSourceContractV1::new(missing_args);
    let _ = ProcessingSourceContractV1::new(context_by_value);
    let _ = ProcessingSourceContractV1::new(immutable_context);
    let _ = ProcessingSourceContractV1::new(wrong_context_type);
    let _ = ProcessingSourceContractV1::new(wrong_argument_type);
    let _ = ProcessingSourceContractV1::new(mutable_argument_slice);
    let _ = ProcessingSourceContractV1::new(reversed_parameters);
    let _ = ProcessingSourceContractV1::new(bool_return);
    let _ = ProcessingSourceContractV1::new(string_result);
    let _ = ProcessingSourceContractV1::new(async_handler);
    let captured = String::from("captured");
    let _ = ProcessingSourceContractV1::new(move |context: &mut ProcessingContext, args: &[OsString]| {
        let _ = captured;
        let _ = context;
        let _ = args;
        Ok(())
    });
    let _ = ProcessingSourceContractV1::new(function_returning_processing_result_bool);
}
