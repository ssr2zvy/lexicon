pub mod processing;
pub mod protocols;
pub mod runtime;
pub mod session;

pub use protocols::http;
pub use protocols::http::HttpAcquisitionContext;
pub use runtime::{
    CORE_CONTRACT_VERSION, MANAGED_RUNNER_TEMPLATE_VERSION, MissingHttpCapabilities,
    OwnedRuntimeIdentity, RUNTIME_INVOCATION_PROTOCOL_VERSION, RUNTIME_PROTOCOL_VERSION,
    RuntimeCompatibilityError, RuntimeIdentifierError, RuntimeIdentity,
    RuntimeInformationDecodingError, RuntimeInformationEncodingError, RuntimeOperation,
    RuntimeProtocol,
};
pub use session::{
    RUNTIME_CONTEXT_ENVIRONMENT_VARIABLE, RuntimeContextPaths, SessionDataPaths, SessionIdentity,
};

pub use rusqlite;

// COREID-03: the previous public `HttpAcquisition` trait and
// `run_http_source(LEXICON_SOURCE_DIRECTORY)` helper have been removed. The
// only supported acquisition path is the session-aware runner driven by
// `LEXICON_RUNTIME_CONTEXT_V1`. Tests exercising the legacy surface are
// moved into a `#[cfg(test)]` module so production callers cannot
// accidentally re-import it.
