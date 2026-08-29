//! DOC-02 compile-pass fixture: a private processing handler placed in a
//! public `SOURCE` descriptor (here `pub const PROCESSOR`) must compile
//! successfully.
//!
//! The descriptor is the source's compiled declaration of its mandatory
//! processing function. Consumers (the managed processing runner) only see
//! the public descriptor, but Rust permits the public constant to point to
//! a private function inside the same library.

use std::ffi::OsString;

use lexicon_core::processing::{ProcessingContext, ProcessingResult, ProcessingSourceContractV1};

fn private_process(
    _context: &mut ProcessingContext,
    _args: &[OsString],
) -> ProcessingResult<()> {
    Ok(())
}

fn private_resume_only(
    _context: &mut ProcessingContext,
    _args: &[OsString],
) -> ProcessingResult<()> {
    Ok(())
}

pub const PROCESSOR: ProcessingSourceContractV1 = ProcessingSourceContractV1::new(private_process);

fn main() {
    let contract_ptr = PROCESSOR.process_handler() as *const ();
    let handler_ptr = private_process as *const ();
    assert_eq!(contract_ptr, handler_ptr);
    // Also confirm a second private handler reference in a separate
    // descriptor constructs without runtime invocation.
    let _ = ProcessingSourceContractV1::new(private_resume_only);
}
