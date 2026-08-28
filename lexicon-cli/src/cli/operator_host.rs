use std::ffi::OsString;

use clap::Parser;

/// Reserved internal entrypoint used only by re-execution for background
/// execution (`lexicon data ... --bg`).
///
/// This is an internal protocol (see `contract.md` section 3, "Background
/// execution"), not a public command. It is hidden from ordinary `--help`
/// output and must not be invoked directly.
#[derive(Parser, Debug, Clone)]
#[command(
    name = "__operator-host",
    hide = true,
    about = "Reserved internal entrypoint. Do not invoke directly."
)]
pub struct OperatorHostCommand {
    /// The encoded `OperatorHostInvocationV1` reference.
    #[arg(value_name = "INVOCATION_REFERENCE")]
    pub reference: String,

    #[arg(
        trailing_var_arg = true,
        allow_hyphen_values = true,
        help = "Arguments forwarded after `--` to the selected source implementation."
    )]
    pub passthrough: Vec<OsString>,
}
