//! Command-owned metadata for `compare`.

use crate::ReleasedCommand;
use crate::commands::spec::declare_reference_projection_command;
use crate::worker::InferTask;

declare_reference_projection_command!(
    CompareCommand,
    COMPARE_DEFINITION,
    ReleasedCommand::Compare,
    InferTask::Morphosyntax,
);
