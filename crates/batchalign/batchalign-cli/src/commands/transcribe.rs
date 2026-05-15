//! Command-owned metadata for `transcribe` and `transcribe_s`.

use crate::ReleasedCommand;
use crate::commands::spec::declare_transcription_command;

declare_transcription_command!(
    TranscribeCommand,
    TRANSCRIBE_DEFINITION,
    ReleasedCommand::Transcribe,
);
declare_transcription_command!(
    TranscribeSCommand,
    TRANSCRIBE_S_DEFINITION,
    ReleasedCommand::TranscribeS,
);
