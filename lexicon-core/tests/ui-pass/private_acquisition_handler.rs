//! DOC-02 compile-pass fixture: a private acquisition handler placed in a
//! public `SOURCE` descriptor must compile successfully.
//!
//! The descriptor is the source's compiled declaration of its mandatory
//! acquisition function and required capabilities. Consumers (the managed
//! runner) only see the public descriptor, but Rust permits the public
//! constant to point to a private function inside the same library.

use std::ffi::OsString;

use lexicon_core::HttpAcquisitionContext;
use lexicon_core::http::{AcquisitionResult, HttpSourceContractV1};

fn private_handler(
    _context: &mut HttpAcquisitionContext,
    _args: &[OsString],
) -> AcquisitionResult<()> {
    Ok(())
}

pub const SOURCE: HttpSourceContractV1 = HttpSourceContractV1::new(private_handler);

fn main() {
    // Statically resolves to the private handler through the public descriptor.
    let contract_ptr = SOURCE.acquire() as *const ();
    let handler_ptr = private_handler as *const ();
    assert_eq!(contract_ptr, handler_ptr);
}
