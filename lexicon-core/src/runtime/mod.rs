mod identity;
pub mod information;
pub mod invocation;
pub mod invocation_transport;

/// The normative command-line argument for probing runtime information per specs.md §22.
pub const RUNTIME_INFORMATION_PROBE_ARGUMENT: &str = "--lexicon-runtime-info";

pub use identity::{
    OwnedRuntimeIdentity, RuntimeIdentifierError, RuntimeIdentity, RuntimeOperation,
    RuntimeProtocol,
};
pub use information::{
    MissingHttpCapabilities, RuntimeCompatibilityError, RuntimeInformationDecodingError,
    RuntimeInformationEncodingError, RuntimeInformationV1,
};
pub use invocation::{
    CORE_CONTRACT_VERSION, MANAGED_RUNNER_TEMPLATE_VERSION, ProjectInvocationIdentity,
    RUNTIME_INVOCATION_PROTOCOL_VERSION, RUNTIME_PROTOCOL_VERSION, RuntimeExecutionMode,
    RuntimeInvocationConstructionError, RuntimeInvocationDecodingError,
    RuntimeInvocationEncodingError, RuntimeInvocationEnvelopeV1, RuntimeInvocationIdentifierError,
    RuntimeInvocationValueError, RuntimeSupervisionMode, SessionInvocationIdentity,
};
pub use invocation_transport::{
    EncodedRuntimeInvocation, MAX_RUNTIME_INVOCATION_ENVELOPE_JSON_BYTES, ParsedRuntimeInvocation,
    RUNTIME_INVOCATION_ARGUMENT, RUNTIME_SOURCE_ARGUMENT_DELIMITER,
    RuntimeInvocationTransportDecodingError, RuntimeInvocationTransportEncodingError,
    encode_runtime_invocation, parse_runtime_invocation,
};
