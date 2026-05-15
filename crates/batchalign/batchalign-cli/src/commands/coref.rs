//! Command-owned metadata for `coref`.

use crate::ReleasedCommand;
use crate::commands::spec::declare_batched_text_command;
use crate::worker::InferTask;

declare_batched_text_command!(
    CorefCommand,
    COREF_DEFINITION,
    ReleasedCommand::Coref,
    InferTask::Coref,
);
