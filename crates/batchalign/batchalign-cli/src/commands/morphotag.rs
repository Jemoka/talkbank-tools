//! Command-owned metadata for `morphotag`.

use crate::ReleasedCommand;
use crate::commands::spec::declare_batched_text_command;
use crate::worker::InferTask;

declare_batched_text_command!(
    MorphotagCommand,
    MORPHOTAG_DEFINITION,
    ReleasedCommand::Morphotag,
    InferTask::Morphosyntax,
);
