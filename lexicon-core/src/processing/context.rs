#[derive(Debug, Clone, Copy, Default)]
pub struct ProcessingContext {
    _private: (),
}

impl ProcessingContext {
    #[cfg(test)]
    pub(crate) fn new_for_tests() -> Self {
        Self { _private: () }
    }
}
