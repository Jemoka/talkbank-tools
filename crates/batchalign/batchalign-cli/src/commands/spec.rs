//! Command-owned catalog types and derived runtime policy.

use crate::ReleasedCommand;
use crate::command_family::WorkflowFamily;
use crate::worker::InferTask;

/// How one released command is surfaced relative to the worker infer-task layer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CommandCapabilityKind {
    /// Command is advertised directly from one infer task.
    DirectInfer,
    /// Command is synthesized by Rust from lower-level infer capability.
    ServerComposed,
}

/// How one released command maps an input filename to its primary output filename.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CommandOutputPathKind {
    /// Keep the incoming relative path and extension unchanged.
    PreserveInputName,
    /// Replace the input extension with a fixed output extension.
    ReplaceExtension(&'static str),
}

/// Which server-side runtime path currently owns one released command.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RunnerDispatchKind {
    /// Text-only commands pooled through the batched infer path.
    BatchedTextInfer,
    /// Forced alignment with per-file audio/media resolution.
    ForcedAlignment,
    /// Transcribe audio through the Rust-owned ASR orchestration path.
    TranscribeAudioInfer,
    /// Benchmark audio through the composite benchmark orchestrator.
    BenchmarkAudioInfer,
    /// Media-analysis V2 path for commands like openSMILE and AVQI.
    MediaAnalysisV2,
}

/// High-level scheduling shape the command expects from the shared kernel.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SchedulingPolicy {
    /// One audio/media file at a time, with bounded per-job parallelism.
    PerFileAudio,
    /// Many text files pooled into one or more shared infer batches.
    CrossFileBatch,
    /// One primary file plus one paired reference artifact.
    ReferenceProjection,
    /// The command is built by composing other command-owned flows.
    Composite,
    /// Per-file media analysis over non-CHAT inputs.
    PerFileMediaAnalysis,
}

/// How the command expects model state to be shared.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ModelSharingPolicy {
    /// Reuse warm workers and shared model state whenever possible.
    SharedWarmWorkers,
    /// Let composed child commands own model sharing.
    DelegatedToSubcommands,
}

/// Whether the command benefits from cross-file or internal batching.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BatchingPolicy {
    /// No profitable batching beyond ordinary per-file execution.
    None,
    /// Pool many files together into shared worker requests.
    CrossFileBatch,
    /// Keep the top-level unit per file, but allow internal stage batching.
    InternalStageBatching,
    /// One main file plus one paired reference artifact.
    PairedInputs,
}

/// How much per-command parallelism the shared kernel should expose.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ParallelismPolicy {
    /// Bound file-level concurrency and let the kernel auto-tune worker counts.
    BoundedFileWorkers,
    /// Keep one command-level dispatch at a time per job.
    SingleDispatchPerJob,
    /// Let composed child commands own their own parallelism.
    DelegatedToSubcommands,
}

/// How one command should behave on constrained-memory hosts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ConstrainedHostPolicy {
    /// Allow the host to clamp execution to one worker and rely on lazy startup
    /// rather than speculative resident state.
    SequentialFallback,
    /// Let composed child commands own constrained-host behavior.
    DelegatedToSubcommands,
}

/// Whether the command should participate in optional background warmup.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum WarmupPolicy {
    /// The command should stay lazy/on-demand by default.
    LazyOnDemand,
    /// The host may warm this command in the background when capacity allows.
    BackgroundEligible,
    /// Let composed child commands own warmup behavior.
    DelegatedToSubcommands,
}

/// Dominant resource lane for the command's hot path.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ResourceLane {
    /// GPU-backed workloads where device memory is the main bottleneck.
    GpuHeavy,
    /// CPU-bound workloads that still reuse warm model workers.
    CpuBound,
    /// Mostly IO / media feature extraction.
    IoBound,
    /// Mixed pipelines touching both CPU and GPU stages.
    Mixed,
}

/// How the CLI should ship inputs to the server for this command.
///
/// Makes the content/paths-mode superset structural: a command either
/// uploads file bodies over HTTP, sends paths for CHAT-only inputs, or
/// sends paths plus requires shared-filesystem audio access. The illegal
/// combination "needs local audio but cannot use paths mode" is
/// unrepresentable.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CommandIoProfile {
    /// CLI uploads full file bodies over HTTP; server never reads client paths.
    ContentOnly,
    /// CLI sends filesystem paths for text inputs; server reads CHAT directly.
    PathsModeText,
    /// CLI sends filesystem paths and the command also needs client-local
    /// audio on the shared filesystem (only valid for a local daemon).
    PathsModeAudio,
}

impl CommandIoProfile {
    /// Whether the server-side runner needs shared-filesystem audio access.
    pub const fn uses_local_audio(self) -> bool {
        matches!(self, Self::PathsModeAudio)
    }

    /// Whether the CLI may send paths instead of inlined content to a local daemon.
    pub const fn supports_paths_mode(self) -> bool {
        matches!(self, Self::PathsModeText | Self::PathsModeAudio)
    }
}

/// Typed descriptor for one released command.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct CommandWorkflowDescriptor {
    /// Stable command name exposed to users.
    pub command: ReleasedCommand,
    /// Workflow family that owns the command semantics.
    pub family: WorkflowFamily,
    /// Primary infer task required by the worker layer.
    pub infer_task: InferTask,
    /// How the command is surfaced relative to the worker layer.
    pub capability_kind: CommandCapabilityKind,
    /// How the CLI ships inputs and whether paths mode is eligible.
    pub io_profile: CommandIoProfile,
    /// How this command derives its primary output path.
    pub output_path_kind: CommandOutputPathKind,
    /// Which server-side runtime path currently owns this command.
    pub runner_dispatch_kind: RunnerDispatchKind,
}

/// Higher-level execution shape authored by commands.
///
/// This deliberately collapses the repeated low-level scheduling/profile knobs
/// into a smaller semantic vocabulary. Runtime-facing code derives its lower-
/// level policy directly from this shape instead of from a second authored
/// profile object.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CommandExecutionShape {
    /// Cross-file text pooling with one top-level dispatch per job.
    BatchedText,
    /// Main transcript plus paired reference projection.
    ReferenceProjection,
    /// Audio-first sequential processing with bounded file-level concurrency.
    AudioSequential,
    /// Per-file media analysis over non-CHAT/audio feature inputs.
    MediaAnalysis,
    /// Composite command that delegates runtime policy to child flows.
    Composite,
}

impl CommandExecutionShape {
    /// High-level workflow family implied by this authored execution shape.
    pub(crate) const fn workflow_family(self) -> WorkflowFamily {
        match self {
            Self::BatchedText => WorkflowFamily::CrossFileBatchTransform,
            Self::ReferenceProjection => WorkflowFamily::ReferenceProjection,
            Self::AudioSequential | Self::MediaAnalysis => WorkflowFamily::PerFileTransform,
            Self::Composite => WorkflowFamily::Composite,
        }
    }

    /// High-level scheduling shape implied by this authored execution shape.
    pub(crate) const fn scheduling_policy(self) -> SchedulingPolicy {
        match self {
            Self::BatchedText => SchedulingPolicy::CrossFileBatch,
            Self::ReferenceProjection => SchedulingPolicy::ReferenceProjection,
            Self::AudioSequential => SchedulingPolicy::PerFileAudio,
            Self::MediaAnalysis => SchedulingPolicy::PerFileMediaAnalysis,
            Self::Composite => SchedulingPolicy::Composite,
        }
    }

    /// Model-sharing policy implied by this authored execution shape.
    pub(crate) const fn model_sharing_policy(self) -> ModelSharingPolicy {
        match self {
            Self::Composite => ModelSharingPolicy::DelegatedToSubcommands,
            Self::BatchedText
            | Self::ReferenceProjection
            | Self::AudioSequential
            | Self::MediaAnalysis => ModelSharingPolicy::SharedWarmWorkers,
        }
    }

    /// Batching policy implied by this authored execution shape.
    pub(crate) const fn batching_policy(self) -> BatchingPolicy {
        match self {
            Self::BatchedText => BatchingPolicy::CrossFileBatch,
            Self::ReferenceProjection => BatchingPolicy::PairedInputs,
            Self::AudioSequential => BatchingPolicy::InternalStageBatching,
            Self::MediaAnalysis | Self::Composite => BatchingPolicy::None,
        }
    }

    /// Parallelism policy implied by this authored execution shape.
    pub(crate) const fn parallelism_policy(self) -> ParallelismPolicy {
        match self {
            Self::AudioSequential | Self::MediaAnalysis => ParallelismPolicy::BoundedFileWorkers,
            Self::BatchedText | Self::ReferenceProjection => {
                ParallelismPolicy::SingleDispatchPerJob
            }
            Self::Composite => ParallelismPolicy::DelegatedToSubcommands,
        }
    }

    /// Dominant resource lane implied by this authored execution shape.
    pub(crate) const fn resource_lane(self) -> ResourceLane {
        match self {
            Self::BatchedText => ResourceLane::CpuBound,
            Self::ReferenceProjection | Self::Composite => ResourceLane::Mixed,
            Self::AudioSequential => ResourceLane::GpuHeavy,
            Self::MediaAnalysis => ResourceLane::IoBound,
        }
    }

    /// Constrained-host behavior implied by this authored execution shape.
    pub(crate) const fn constrained_host_policy(self) -> ConstrainedHostPolicy {
        match self {
            Self::Composite => ConstrainedHostPolicy::DelegatedToSubcommands,
            Self::BatchedText
            | Self::ReferenceProjection
            | Self::AudioSequential
            | Self::MediaAnalysis => ConstrainedHostPolicy::SequentialFallback,
        }
    }

    /// Warmup behavior implied by this authored execution shape.
    pub(crate) const fn warmup_policy(self) -> WarmupPolicy {
        match self {
            Self::MediaAnalysis => WarmupPolicy::LazyOnDemand,
            Self::Composite => WarmupPolicy::DelegatedToSubcommands,
            Self::BatchedText | Self::ReferenceProjection | Self::AudioSequential => {
                WarmupPolicy::BackgroundEligible
            }
        }
    }

    /// Whether host-memory admission should remain enabled for this shape.
    pub(crate) const fn uses_host_memory_gate(self) -> bool {
        true
    }
}

/// Canonical authored command definition.
///
/// Prefer the family authoring traits/macros below for ordinary command work.
/// These constructor helpers are the lower-level substrate those generated
/// declarations build on. Command modules should only hand-write a full
/// [`CommandWorkflowDescriptor`] when they are introducing a genuinely new
/// execution family or an unusual routing shape that the existing helpers do
/// not model yet.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct CommandDefinition {
    /// Stable released-command descriptor.
    pub descriptor: CommandWorkflowDescriptor,
    /// Higher-level execution shape authored by the command.
    pub execution_shape: CommandExecutionShape,
}

impl CommandDefinition {
    const fn new(
        command: ReleasedCommand,
        infer_task: InferTask,
        capability_kind: CommandCapabilityKind,
        io_profile: CommandIoProfile,
        output_path_kind: CommandOutputPathKind,
        runner_dispatch_kind: RunnerDispatchKind,
        execution_shape: CommandExecutionShape,
    ) -> Self {
        Self {
            descriptor: CommandWorkflowDescriptor {
                command,
                family: execution_shape.workflow_family(),
                infer_task,
                capability_kind,
                io_profile,
                output_path_kind,
                runner_dispatch_kind,
            },
            execution_shape,
        }
    }

    /// Author-facing constructor for a text-first direct infer command.
    pub(crate) const fn batched_text(command: ReleasedCommand, infer_task: InferTask) -> Self {
        Self::new(
            command,
            infer_task,
            CommandCapabilityKind::DirectInfer,
            CommandIoProfile::PathsModeText,
            CommandOutputPathKind::PreserveInputName,
            RunnerDispatchKind::BatchedTextInfer,
            CommandExecutionShape::BatchedText,
        )
    }

    /// Author-facing constructor for a reference-projection command.
    pub(crate) const fn reference_projection(
        command: ReleasedCommand,
        infer_task: InferTask,
    ) -> Self {
        Self::new(
            command,
            infer_task,
            CommandCapabilityKind::DirectInfer,
            CommandIoProfile::PathsModeText,
            CommandOutputPathKind::PreserveInputName,
            RunnerDispatchKind::BatchedTextInfer,
            CommandExecutionShape::ReferenceProjection,
        )
    }

    /// Author-facing constructor for the current forced-alignment family.
    pub(crate) const fn forced_alignment(command: ReleasedCommand) -> Self {
        Self::new(
            command,
            InferTask::Fa,
            CommandCapabilityKind::DirectInfer,
            CommandIoProfile::PathsModeAudio,
            CommandOutputPathKind::PreserveInputName,
            RunnerDispatchKind::ForcedAlignment,
            CommandExecutionShape::AudioSequential,
        )
    }

    /// Author-facing constructor for the current ASR transcription family.
    pub(crate) const fn transcription(command: ReleasedCommand) -> Self {
        Self::new(
            command,
            InferTask::Asr,
            CommandCapabilityKind::ServerComposed,
            CommandIoProfile::PathsModeAudio,
            CommandOutputPathKind::ReplaceExtension("cha"),
            RunnerDispatchKind::TranscribeAudioInfer,
            CommandExecutionShape::AudioSequential,
        )
    }

    /// Author-facing constructor for the current benchmark orchestration family.
    pub(crate) const fn benchmark(command: ReleasedCommand) -> Self {
        Self::new(
            command,
            InferTask::Asr,
            CommandCapabilityKind::ServerComposed,
            CommandIoProfile::PathsModeAudio,
            CommandOutputPathKind::PreserveInputName,
            RunnerDispatchKind::BenchmarkAudioInfer,
            CommandExecutionShape::Composite,
        )
    }

    /// Author-facing constructor for the current media-analysis family.
    pub(crate) const fn media_analysis(
        command: ReleasedCommand,
        infer_task: InferTask,
        io_profile: CommandIoProfile,
    ) -> Self {
        Self::new(
            command,
            infer_task,
            CommandCapabilityKind::DirectInfer,
            io_profile,
            CommandOutputPathKind::PreserveInputName,
            RunnerDispatchKind::MediaAnalysisV2,
            CommandExecutionShape::MediaAnalysis,
        )
    }

    /// High-level scheduling shape derived from the authored execution shape.
    pub(crate) const fn scheduling_policy(self) -> SchedulingPolicy {
        self.execution_shape.scheduling_policy()
    }

    /// Model-sharing policy derived from the authored execution shape.
    pub(crate) const fn model_sharing_policy(self) -> ModelSharingPolicy {
        self.execution_shape.model_sharing_policy()
    }

    /// Batching policy derived from the authored execution shape.
    pub(crate) const fn batching_policy(self) -> BatchingPolicy {
        self.execution_shape.batching_policy()
    }

    /// Parallelism policy derived from the authored execution shape.
    pub(crate) const fn parallelism_policy(self) -> ParallelismPolicy {
        self.execution_shape.parallelism_policy()
    }

    /// Dominant resource lane derived from the authored execution shape.
    pub(crate) const fn resource_lane(self) -> ResourceLane {
        self.execution_shape.resource_lane()
    }

    /// Constrained-host behavior derived from the authored execution shape.
    pub(crate) const fn constrained_host_policy(self) -> ConstrainedHostPolicy {
        self.execution_shape.constrained_host_policy()
    }

    /// Warmup behavior derived from the authored execution shape.
    pub(crate) const fn warmup_policy(self) -> WarmupPolicy {
        self.execution_shape.warmup_policy()
    }

    /// Whether host-memory admission should remain enabled.
    pub(crate) const fn uses_host_memory_gate(self) -> bool {
        self.execution_shape.uses_host_memory_gate()
    }
}

/// Type-level authoring seam for ordinary batched-text commands.
pub(crate) trait BatchedTextCommand {
    /// Stable released command name.
    const COMMAND: ReleasedCommand;
    /// Worker infer task for this text family command.
    const INFER_TASK: InferTask;
    /// Generated canonical command definition.
    const DEFINITION: CommandDefinition =
        CommandDefinition::batched_text(Self::COMMAND, Self::INFER_TASK);
}

/// Type-level authoring seam for reference-projection commands.
pub(crate) trait ReferenceProjectionCommand {
    /// Stable released command name.
    const COMMAND: ReleasedCommand;
    /// Worker infer task for the projection pass.
    const INFER_TASK: InferTask;
    /// Generated canonical command definition.
    const DEFINITION: CommandDefinition =
        CommandDefinition::reference_projection(Self::COMMAND, Self::INFER_TASK);
}

/// Type-level authoring seam for forced-alignment commands.
pub(crate) trait ForcedAlignmentCommand {
    /// Stable released command name.
    const COMMAND: ReleasedCommand;
    /// Generated canonical command definition.
    const DEFINITION: CommandDefinition = CommandDefinition::forced_alignment(Self::COMMAND);
}

/// Type-level authoring seam for transcription commands.
pub(crate) trait TranscriptionCommand {
    /// Stable released command name.
    const COMMAND: ReleasedCommand;
    /// Generated canonical command definition.
    const DEFINITION: CommandDefinition = CommandDefinition::transcription(Self::COMMAND);
}

/// Type-level authoring seam for benchmark commands.
pub(crate) trait BenchmarkCommand {
    /// Stable released command name.
    const COMMAND: ReleasedCommand;
    /// Generated canonical command definition.
    const DEFINITION: CommandDefinition = CommandDefinition::benchmark(Self::COMMAND);
}

/// Type-level authoring seam for media-analysis commands.
pub(crate) trait MediaAnalysisCommand {
    /// Stable released command name.
    const COMMAND: ReleasedCommand;
    /// Worker infer task for this media-analysis command.
    const INFER_TASK: InferTask;
    /// How the CLI ships inputs for this media-analysis command.
    const IO_PROFILE: CommandIoProfile;
    /// Generated canonical command definition.
    const DEFINITION: CommandDefinition =
        CommandDefinition::media_analysis(Self::COMMAND, Self::INFER_TASK, Self::IO_PROFILE);
}

macro_rules! declare_batched_text_command {
    ($marker:ident, $definition:ident, $command:expr, $infer_task:expr $(,)?) => {
        pub(crate) struct $marker;
        impl $crate::commands::spec::BatchedTextCommand for $marker {
            const COMMAND: $crate::ReleasedCommand = $command;
            const INFER_TASK: $crate::worker::InferTask = $infer_task;
        }
        pub(crate) const $definition: $crate::commands::spec::CommandDefinition =
            <$marker as $crate::commands::spec::BatchedTextCommand>::DEFINITION;
    };
}
pub(crate) use declare_batched_text_command;

macro_rules! declare_reference_projection_command {
    ($marker:ident, $definition:ident, $command:expr, $infer_task:expr $(,)?) => {
        pub(crate) struct $marker;
        impl $crate::commands::spec::ReferenceProjectionCommand for $marker {
            const COMMAND: $crate::ReleasedCommand = $command;
            const INFER_TASK: $crate::worker::InferTask = $infer_task;
        }
        pub(crate) const $definition: $crate::commands::spec::CommandDefinition =
            <$marker as $crate::commands::spec::ReferenceProjectionCommand>::DEFINITION;
    };
}
pub(crate) use declare_reference_projection_command;

macro_rules! declare_forced_alignment_command {
    ($marker:ident, $definition:ident, $command:expr $(,)?) => {
        pub(crate) struct $marker;
        impl $crate::commands::spec::ForcedAlignmentCommand for $marker {
            const COMMAND: $crate::ReleasedCommand = $command;
        }
        pub(crate) const $definition: $crate::commands::spec::CommandDefinition =
            <$marker as $crate::commands::spec::ForcedAlignmentCommand>::DEFINITION;
    };
}
pub(crate) use declare_forced_alignment_command;

macro_rules! declare_transcription_command {
    ($marker:ident, $definition:ident, $command:expr $(,)?) => {
        pub(crate) struct $marker;
        impl $crate::commands::spec::TranscriptionCommand for $marker {
            const COMMAND: $crate::ReleasedCommand = $command;
        }
        pub(crate) const $definition: $crate::commands::spec::CommandDefinition =
            <$marker as $crate::commands::spec::TranscriptionCommand>::DEFINITION;
    };
}
pub(crate) use declare_transcription_command;

macro_rules! declare_benchmark_command {
    ($marker:ident, $definition:ident, $command:expr $(,)?) => {
        pub(crate) struct $marker;
        impl $crate::commands::spec::BenchmarkCommand for $marker {
            const COMMAND: $crate::ReleasedCommand = $command;
        }
        pub(crate) const $definition: $crate::commands::spec::CommandDefinition =
            <$marker as $crate::commands::spec::BenchmarkCommand>::DEFINITION;
    };
}
pub(crate) use declare_benchmark_command;

macro_rules! declare_media_analysis_command {
    ($marker:ident, $definition:ident, $command:expr, $infer_task:expr, $io_profile:expr $(,)?) => {
        pub(crate) struct $marker;
        impl $crate::commands::spec::MediaAnalysisCommand for $marker {
            const COMMAND: $crate::ReleasedCommand = $command;
            const INFER_TASK: $crate::worker::InferTask = $infer_task;
            const IO_PROFILE: $crate::commands::spec::CommandIoProfile = $io_profile;
        }
        pub(crate) const $definition: $crate::commands::spec::CommandDefinition =
            <$marker as $crate::commands::spec::MediaAnalysisCommand>::DEFINITION;
    };
}
pub(crate) use declare_media_analysis_command;

#[cfg(test)]
mod tests {
    use super::{
        BatchedTextCommand, BatchingPolicy, CommandCapabilityKind, CommandDefinition,
        CommandExecutionShape, CommandIoProfile, CommandOutputPathKind, ConstrainedHostPolicy,
        MediaAnalysisCommand, RunnerDispatchKind, SchedulingPolicy, TranscriptionCommand,
        WarmupPolicy,
    };
    use crate::ReleasedCommand;
    use crate::worker::InferTask;

    #[test]
    fn batched_text_shape_keeps_cross_file_batch_contract() {
        let shape = CommandExecutionShape::BatchedText;
        assert_eq!(shape.scheduling_policy(), SchedulingPolicy::CrossFileBatch);
        assert_eq!(shape.batching_policy(), BatchingPolicy::CrossFileBatch);
        assert_eq!(
            shape.constrained_host_policy(),
            ConstrainedHostPolicy::SequentialFallback
        );
        assert_eq!(shape.warmup_policy(), WarmupPolicy::BackgroundEligible);
        assert!(shape.uses_host_memory_gate());
    }

    #[test]
    fn media_analysis_shape_stays_lazy_on_demand() {
        let shape = CommandExecutionShape::MediaAnalysis;
        assert_eq!(
            shape.scheduling_policy(),
            SchedulingPolicy::PerFileMediaAnalysis
        );
        assert_eq!(shape.batching_policy(), BatchingPolicy::None);
        assert_eq!(shape.warmup_policy(), WarmupPolicy::LazyOnDemand);
        assert!(shape.uses_host_memory_gate());
    }

    #[test]
    fn composite_shape_delegates_runtime_policy() {
        let shape = CommandExecutionShape::Composite;
        assert_eq!(shape.scheduling_policy(), SchedulingPolicy::Composite);
        assert_eq!(
            shape.constrained_host_policy(),
            ConstrainedHostPolicy::DelegatedToSubcommands
        );
        assert_eq!(shape.warmup_policy(), WarmupPolicy::DelegatedToSubcommands);
    }

    #[test]
    fn batched_text_constructor_keeps_author_surface_direct_first() {
        let definition =
            CommandDefinition::batched_text(ReleasedCommand::Morphotag, InferTask::Morphosyntax);
        assert_eq!(
            definition.descriptor.family,
            definition.execution_shape.workflow_family()
        );
        assert_eq!(
            definition.descriptor.capability_kind,
            CommandCapabilityKind::DirectInfer
        );
        assert_eq!(
            definition.descriptor.runner_dispatch_kind,
            RunnerDispatchKind::BatchedTextInfer
        );
        assert_eq!(
            definition.descriptor.output_path_kind,
            CommandOutputPathKind::PreserveInputName
        );
    }

    #[test]
    fn transcription_constructor_hides_server_composed_defaults() {
        let definition = CommandDefinition::transcription(ReleasedCommand::Transcribe);
        assert_eq!(
            definition.descriptor.family,
            definition.execution_shape.workflow_family()
        );
        assert_eq!(
            definition.descriptor.capability_kind,
            CommandCapabilityKind::ServerComposed
        );
        assert!(definition.descriptor.io_profile.uses_local_audio());
        assert_eq!(
            definition.descriptor.runner_dispatch_kind,
            RunnerDispatchKind::TranscribeAudioInfer
        );
        assert_eq!(
            definition.descriptor.output_path_kind,
            CommandOutputPathKind::ReplaceExtension("cha")
        );
    }

    struct GeneratedMorphotagCommand;
    impl BatchedTextCommand for GeneratedMorphotagCommand {
        const COMMAND: ReleasedCommand = ReleasedCommand::Morphotag;
        const INFER_TASK: InferTask = InferTask::Morphosyntax;
    }

    struct GeneratedAvqiCommand;
    impl MediaAnalysisCommand for GeneratedAvqiCommand {
        const COMMAND: ReleasedCommand = ReleasedCommand::Avqi;
        const INFER_TASK: InferTask = InferTask::Avqi;
        const IO_PROFILE: CommandIoProfile = CommandIoProfile::PathsModeAudio;
    }

    struct GeneratedTranscribeCommand;
    impl TranscriptionCommand for GeneratedTranscribeCommand {
        const COMMAND: ReleasedCommand = ReleasedCommand::Transcribe;
    }

    declare_batched_text_command!(
        GeneratedTranslateCommand,
        GENERATED_TRANSLATE_DEFINITION,
        ReleasedCommand::Translate,
        InferTask::Translate,
    );

    #[test]
    fn batched_text_trait_generates_same_definition_as_constructor() {
        assert_eq!(
            <GeneratedMorphotagCommand as BatchedTextCommand>::DEFINITION,
            CommandDefinition::batched_text(ReleasedCommand::Morphotag, InferTask::Morphosyntax)
        );
    }

    #[test]
    fn media_analysis_trait_generates_same_definition_as_constructor() {
        assert_eq!(
            <GeneratedAvqiCommand as MediaAnalysisCommand>::DEFINITION,
            CommandDefinition::media_analysis(
                ReleasedCommand::Avqi,
                InferTask::Avqi,
                CommandIoProfile::PathsModeAudio,
            )
        );
    }

    #[test]
    fn transcription_trait_generates_same_definition_as_constructor() {
        assert_eq!(
            <GeneratedTranscribeCommand as TranscriptionCommand>::DEFINITION,
            CommandDefinition::transcription(ReleasedCommand::Transcribe)
        );
    }

    #[test]
    fn declaration_macro_generates_batched_text_definition() {
        assert_eq!(
            GENERATED_TRANSLATE_DEFINITION,
            CommandDefinition::batched_text(ReleasedCommand::Translate, InferTask::Translate)
        );
    }
}
