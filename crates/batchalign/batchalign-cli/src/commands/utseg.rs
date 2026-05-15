//! Command-owned metadata for `utseg`.

use crate::ReleasedCommand;
use crate::commands::spec::declare_batched_text_command;
use crate::worker::InferTask;

declare_batched_text_command!(
    UtsegCommand,
    UTSEG_DEFINITION,
    ReleasedCommand::Utseg,
    InferTask::Utseg,
);
