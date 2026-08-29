//! Internal test support exposed only under `#[cfg(test)]`. The module is
//! not part of the public crate surface and is used by HTTP-03 durable
//! recording tests to construct deterministic shadow servers.

pub(crate) mod scripted_server;
pub(crate) mod durability_observer;

pub(crate) use scripted_server::{
    ReceivedRequest, ScriptedServerHandle, ScriptedStep,
};
pub(crate) use durability_observer::{
    BlockingDurabilityObserver, DurabilityBlockingHook, DurabilityEvent,
    DurabilityEventKind, DurabilityObserver, DurabilityPublisherHooks,
    NoopDurabilityObserver, RecordingDurabilityObserver,
};
