//! Command-routing policy helpers for the job runner.
//!
//! Keep these small and declarative. `runner/mod.rs` should read as job
//! lifecycle orchestration, not as a mixed bag of routing tables and filename
//! conventions.

use crate::api::ReleasedCommand;
use crate::commands::{
    RunnerDispatchKind, command_runner_dispatch_kind, command_workflow_descriptor,
};
#[cfg(test)]
use crate::recipe_runner::runtime::result_display_path_for_command;
use crate::worker::InferTask;

/// Return the primary infer task backing one released command.
pub(crate) fn infer_task_for_command(command: ReleasedCommand) -> Option<InferTask> {
    command_workflow_descriptor(command).map(|descriptor| descriptor.infer_task)
}

/// Return `true` when the released command must use a Rust-owned CHAT-backed
/// infer dispatch path instead of a pure content relay.
///
/// This is narrower than "uses infer somewhere in the runtime". Audio-first
/// commands like `opensmile` and `avqi` also dispatch to workers, but they do
/// not require `all_chat=true` and therefore must not go through the generic
/// CHAT-infer admission gate in `routing.rs`.
pub(crate) fn command_requires_chat_infer(command: ReleasedCommand) -> bool {
    matches!(
        command_runner_dispatch_kind(command),
        Some(RunnerDispatchKind::BatchedTextInfer | RunnerDispatchKind::ForcedAlignment)
    )
}

/// Derive the result filename for one released command.
#[cfg(test)]
pub(crate) fn result_filename_for_command(command: ReleasedCommand, filename: &str) -> String {
    result_display_path_for_command(command, filename).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn media_analysis_commands_do_not_require_chat_infer_gate() {
        assert!(!command_requires_chat_infer(ReleasedCommand::Opensmile));
        assert!(!command_requires_chat_infer(ReleasedCommand::Avqi));
    }

    #[test]
    fn chat_backed_infer_commands_still_require_chat_infer_gate() {
        assert!(command_requires_chat_infer(ReleasedCommand::Morphotag));
        assert!(command_requires_chat_infer(ReleasedCommand::Utseg));
        assert!(command_requires_chat_infer(ReleasedCommand::Translate));
        assert!(command_requires_chat_infer(ReleasedCommand::Coref));
        assert!(command_requires_chat_infer(ReleasedCommand::Compare));
        assert!(command_requires_chat_infer(ReleasedCommand::Align));
    }
}
