//! Command-owned metadata for `translate`.

use crate::ReleasedCommand;
use crate::commands::spec::declare_batched_text_command;
use crate::worker::InferTask;

declare_batched_text_command!(
    TranslateCommand,
    TRANSLATE_DEFINITION,
    ReleasedCommand::Translate,
    InferTask::Translate,
);
