//! SCAFFOLD-02 host-level durability helpers.
//!
//! The audit requires every staged file write inside the source-creation
//! pipeline to use a path-creation + write + fsync primitive, and to
//! directory-fsync the staging tree before publication.
//!
//! The scaffolding code lives in `lexicon-framework/src/build/` (target
//! directory writers) and `lexicon-framework/src/publication/`
//! (staged-runtime publication). Those modules adopt this helper via the
//! `crate::fs` re-export so any future caller inherits the durability
//! guarantee.
pub(crate) mod durable;

pub use durable::{
    DirectorySyncOutcome, DurableFileError, sync_directory, sync_subtree_bottom_up, write_new_file,
};
