//! Command-owned metadata for `opensmile`.

use crate::ReleasedCommand;
use crate::commands::spec::{CommandIoProfile, declare_media_analysis_command};
use crate::worker::InferTask;

declare_media_analysis_command!(
    OpensmileCommand,
    OPENSMILE_DEFINITION,
    ReleasedCommand::Opensmile,
    InferTask::Opensmile,
    CommandIoProfile::ContentOnly,
);
