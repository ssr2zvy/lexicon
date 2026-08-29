//! NAME-01 typed identity rules.

pub mod managed_name;

pub use managed_name::{
    ManagedName, ManagedNameError, MAX_MANAGED_NAME_BYTES, RESERVED_NAMES,
};
