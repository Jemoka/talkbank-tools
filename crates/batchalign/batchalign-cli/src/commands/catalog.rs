//! Static command-owned catalog.

use crate::ReleasedCommand;
use crate::command_model;

use super::spec::{CommandDefinition, CommandWorkflowDescriptor};

const RELEASED_COMMAND_ORDER: &[ReleasedCommand] = &[
    ReleasedCommand::Morphotag,
    ReleasedCommand::Utseg,
    ReleasedCommand::Translate,
    ReleasedCommand::Coref,
    ReleasedCommand::Align,
    ReleasedCommand::Transcribe,
    ReleasedCommand::TranscribeS,
    ReleasedCommand::Compare,
    ReleasedCommand::Benchmark,
    ReleasedCommand::Opensmile,
    ReleasedCommand::Avqi,
];

/// Return the canonical authored definition for one released command.
pub(crate) fn released_command_definition(command: ReleasedCommand) -> CommandDefinition {
    command_model::legacy_command_definition(command)
}

/// Return the canonical authored definitions for all released commands.
pub(crate) fn released_command_definitions() -> Vec<CommandDefinition> {
    RELEASED_COMMAND_ORDER
        .iter()
        .copied()
        .map(command_model::legacy_command_definition)
        .collect()
}

/// Return the compatibility workflow descriptor derived from one released command.
pub(crate) fn released_command_descriptor(command: ReleasedCommand) -> CommandWorkflowDescriptor {
    command_model::legacy_command_descriptor(command)
}

/// Return the compatibility workflow descriptor for one released command if present.
pub(crate) fn command_workflow_descriptor(
    command: ReleasedCommand,
) -> Option<CommandWorkflowDescriptor> {
    Some(released_command_descriptor(command))
}

#[cfg(test)]
mod tests {
    use super::{released_command_definition, released_command_definitions};
    use crate::ReleasedCommand;
    use crate::commands::spec::{
        BatchingPolicy, CommandExecutionShape, ConstrainedHostPolicy, SchedulingPolicy,
        WarmupPolicy,
    };

    #[test]
    fn command_definitions_have_unique_names() {
        let definitions = released_command_definitions();
        let mut names: Vec<&str> = definitions
            .iter()
            .map(|definition| definition.descriptor.command.as_ref())
            .collect();
        let original_len = names.len();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), original_len, "duplicate command definitions");
    }

    #[test]
    fn compare_keeps_paired_reference_profile() {
        let definition = released_command_definition(ReleasedCommand::Compare);
        assert_eq!(
            definition.scheduling_policy(),
            SchedulingPolicy::ReferenceProjection
        );
        assert_eq!(definition.batching_policy(), BatchingPolicy::PairedInputs);
    }

    #[test]
    fn morphotag_keeps_cross_file_batch_profile() {
        let definition = released_command_definition(ReleasedCommand::Morphotag);
        assert_eq!(
            definition.scheduling_policy(),
            SchedulingPolicy::CrossFileBatch
        );
        assert_eq!(definition.batching_policy(), BatchingPolicy::CrossFileBatch);
    }

    #[test]
    fn transcribe_profile_is_background_eligible_but_can_fallback() {
        let definition = released_command_definition(ReleasedCommand::Transcribe);
        assert_eq!(definition.warmup_policy(), WarmupPolicy::BackgroundEligible);
        assert_eq!(
            definition.constrained_host_policy(),
            ConstrainedHostPolicy::SequentialFallback
        );
    }

    #[test]
    fn benchmark_profile_delegates_constrained_host_behavior() {
        let definition = released_command_definition(ReleasedCommand::Benchmark);
        assert_eq!(
            definition.warmup_policy(),
            WarmupPolicy::DelegatedToSubcommands
        );
        assert_eq!(
            definition.constrained_host_policy(),
            ConstrainedHostPolicy::DelegatedToSubcommands
        );
    }

    #[test]
    fn command_definitions_expose_authored_execution_shapes() {
        assert_eq!(
            released_command_definition(ReleasedCommand::Morphotag).execution_shape,
            CommandExecutionShape::BatchedText
        );
        assert_eq!(
            released_command_definition(ReleasedCommand::Transcribe).execution_shape,
            CommandExecutionShape::AudioSequential
        );
        assert_eq!(
            released_command_definition(ReleasedCommand::Compare).execution_shape,
            CommandExecutionShape::ReferenceProjection
        );
    }
}
