use std::ffi::OsString;

use super::{ProcessingContext, ProcessingResult};

pub type ProcessDataFn =
    fn(context: &mut ProcessingContext, args: &[OsString]) -> ProcessingResult<()>;

#[derive(Clone, Copy)]
pub struct ProcessingSourceContractV1 {
    process: ProcessDataFn,
}

impl ProcessingSourceContractV1 {
    pub const CONTRACT_VERSION: u32 = 1;

    pub const fn new(process: ProcessDataFn) -> Self {
        Self { process }
    }

    pub const fn process_handler(&self) -> ProcessDataFn {
        self.process
    }
}

#[cfg(test)]
mod tests {
    use std::ffi::OsString;
    use std::sync::atomic::{AtomicUsize, Ordering};

    use super::{ProcessDataFn, ProcessingSourceContractV1};
    use crate::processing::{ProcessingContext, ProcessingResult};

    const SOURCE: ProcessingSourceContractV1 = ProcessingSourceContractV1::new(process);

    static CONSTRUCTION_CALL_COUNT: AtomicUsize = AtomicUsize::new(0);

    fn process(_context: &mut ProcessingContext, _args: &[OsString]) -> ProcessingResult<()> {
        Ok(())
    }

    fn expect_two_args(
        _context: &mut ProcessingContext,
        args: &[OsString],
    ) -> ProcessingResult<()> {
        assert_eq!(args, &[OsString::from("alpha"), OsString::from("beta")]);
        Ok(())
    }

    fn expect_three_args(
        _context: &mut ProcessingContext,
        args: &[OsString],
    ) -> ProcessingResult<()> {
        assert_eq!(
            args,
            &[
                OsString::from("alpha"),
                OsString::from("beta"),
                OsString::from("gamma"),
            ]
        );
        Ok(())
    }

    #[cfg(unix)]
    fn expect_non_utf8_args(
        _context: &mut ProcessingContext,
        args: &[OsString],
    ) -> ProcessingResult<()> {
        use std::os::unix::ffi::OsStringExt;

        assert_eq!(
            args,
            &[
                OsString::from_vec(vec![b'a', 0x80, b'c']),
                OsString::from_vec(vec![0xFF, 0xFE, 0xFD]),
            ]
        );
        Ok(())
    }

    fn private_handler(context: &mut ProcessingContext, args: &[OsString]) -> ProcessingResult<()> {
        let _ = context;
        let _ = args;
        Ok(())
    }

    fn counting_handler(
        context: &mut ProcessingContext,
        args: &[OsString],
    ) -> ProcessingResult<()> {
        let _ = context;
        let _ = args;
        CONSTRUCTION_CALL_COUNT.fetch_add(1, Ordering::Relaxed);
        Ok(())
    }

    #[test]
    fn contract_version_is_one() {
        assert_eq!(ProcessingSourceContractV1::CONTRACT_VERSION, 1);
    }

    #[test]
    fn valid_processing_function_constructs_descriptor() {
        let contract = ProcessingSourceContractV1::new(process);
        let process_ptr = contract.process_handler() as *const ();
        let original_ptr = process as *const ();
        assert_eq!(process_ptr, original_ptr);
    }

    #[test]
    fn descriptor_works_in_a_constant() {
        let mut context = ProcessingContext::new_for_tests();
        let args = [OsString::from("alpha"), OsString::from("beta")];
        let result = SOURCE.process_handler()(&mut context, &args);
        assert!(result.is_ok(), "result: {result:?}");
    }

    #[test]
    fn descriptor_is_copy() {
        let first = ProcessingSourceContractV1::new(process);
        let second = first;
        let _ = (first, second);
    }

    #[test]
    fn process_handler_returns_stored_function() {
        let contract = ProcessingSourceContractV1::new(process);
        let handler: ProcessDataFn = contract.process_handler();
        let handler_ptr = handler as *const ();
        let expected_ptr = process as *const ();
        assert_eq!(handler_ptr, expected_ptr);
    }

    #[test]
    fn retained_function_pointer_is_callable() {
        let mut context = ProcessingContext::new_for_tests();
        let args = [OsString::from("alpha"), OsString::from("beta")];
        let result =
            ProcessingSourceContractV1::new(expect_two_args).process_handler()(&mut context, &args);
        assert!(result.is_ok(), "result: {result:?}");
    }

    #[test]
    fn handler_receives_mutable_processing_context() {
        let mut context = ProcessingContext::new_for_tests();
        let args = [OsString::from("alpha"), OsString::from("beta")];
        let result =
            ProcessingSourceContractV1::new(expect_two_args).process_handler()(&mut context, &args);
        assert!(result.is_ok(), "result: {result:?}");
    }

    #[test]
    fn handler_receives_osstring_slice() {
        let mut context = ProcessingContext::new_for_tests();
        let args = [OsString::from("alpha"), OsString::from("beta")];
        let result =
            ProcessingSourceContractV1::new(expect_two_args).process_handler()(&mut context, &args);
        assert!(result.is_ok(), "result: {result:?}");
    }

    #[test]
    fn nonempty_native_arguments_reach_handler_unchanged() {
        let mut context = ProcessingContext::new_for_tests();
        let args = [
            OsString::from("alpha"),
            OsString::from("beta"),
            OsString::from("gamma"),
        ];
        let result = ProcessingSourceContractV1::new(expect_three_args).process_handler()(
            &mut context,
            &args,
        );
        assert!(result.is_ok(), "result: {result:?}");
    }

    #[cfg(unix)]
    #[test]
    fn non_utf8_native_arguments_are_preserved() {
        use std::os::unix::ffi::OsStringExt;

        let mut context = ProcessingContext::new_for_tests();
        let args = [
            OsString::from_vec(vec![b'a', 0x80, b'c']),
            OsString::from_vec(vec![0xFF, 0xFE, 0xFD]),
        ];
        let result = ProcessingSourceContractV1::new(expect_non_utf8_args).process_handler()(
            &mut context,
            &args,
        );
        assert!(result.is_ok(), "result: {result:?}");
    }

    #[test]
    fn constructing_descriptor_does_not_invoke_handler() {
        CONSTRUCTION_CALL_COUNT.store(0, Ordering::Relaxed);
        let _ = ProcessingSourceContractV1::new(counting_handler);
        assert_eq!(CONSTRUCTION_CALL_COUNT.load(Ordering::Relaxed), 0);
    }

    #[test]
    fn copying_descriptor_does_not_invoke_handler() {
        CONSTRUCTION_CALL_COUNT.store(0, Ordering::Relaxed);
        let contract = ProcessingSourceContractV1::new(counting_handler);
        let _ = contract;
        let _ = contract;
        assert_eq!(CONSTRUCTION_CALL_COUNT.load(Ordering::Relaxed), 0);
    }

    #[test]
    fn private_handler_works_behind_public_descriptor_constant() {
        const PRIVATE_SOURCE: ProcessingSourceContractV1 =
            ProcessingSourceContractV1::new(private_handler);
        let mut context = ProcessingContext::new_for_tests();
        let args = [OsString::from("alpha")];
        let result = PRIVATE_SOURCE.process_handler()(&mut context, &args);
        assert!(result.is_ok(), "result: {result:?}");
    }
}
