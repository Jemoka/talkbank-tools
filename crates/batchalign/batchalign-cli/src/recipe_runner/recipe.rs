//! Typed recipe metadata for the recipe-runner spike.

#[cfg(test)]
use std::collections::BTreeSet;
use std::fmt;

#[cfg(test)]
use crate::api::ReleasedCommand;
#[cfg(test)]
use crate::error::ServerError;
use crate::runner::util::FileStage;

/// How a command recipe owns execution.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ExecutionMode {
    /// One work unit progresses through an ordered recipe.
    SequentialPerUnit,
    /// One or more stages explicitly batch work units together.
    BatchedStage,
    /// A main transcript is projected against a typed reference companion.
    ReferenceProjection,
    /// The recipe delegates to other recipes as sub-workflows.
    Composite,
}

/// Stable identifiers for recipe-runner stages.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) enum RecipeStageId {
    /// Turn discovered files into typed work units.
    PlanWorkUnits,
    /// Read main CHAT inputs.
    ReadChatInputs,
    /// Read paired reference inputs.
    ReadReferenceInputs,
    /// Resolve and normalize media inputs.
    ResolveAudio,
    /// Automatic speech recognition.
    AsrInfer,
    /// Optional or required speaker diarization.
    SpeakerDiarization,
    /// Rust-owned ASR post-processing.
    AsrPostprocess,
    /// Build CHAT from utterance state.
    BuildChat,
    /// Optional utterance segmentation.
    UtteranceSegmentation,
    /// Morphosyntax enrichment.
    Morphosyntax,
    /// Forced alignment.
    ForcedAlignment,
    /// Cross-unit worker batching.
    BatchInfer,
    /// Align the main transcript against a reference transcript.
    CompareAlign,
    /// Derive compare metrics and sidecar data.
    CompareMetrics,
    /// Dispatch one media-analysis request.
    MediaAnalysis,
    /// Reuse the transcribe recipe inside a composite command.
    RunTranscribeRecipe,
    /// Reuse the compare recipe inside a composite command.
    RunCompareRecipe,
    /// Final CHAT serialization before persistence.
    SerializeChat,
    /// Turn recipe outputs into persistent artifacts.
    MaterializeOutputs,
}

impl RecipeStageId {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::PlanWorkUnits => "plan_work_units",
            Self::ReadChatInputs => "read_chat_inputs",
            Self::ReadReferenceInputs => "read_reference_inputs",
            Self::ResolveAudio => "resolve_audio",
            Self::AsrInfer => "asr_infer",
            Self::SpeakerDiarization => "speaker_diarization",
            Self::AsrPostprocess => "asr_postprocess",
            Self::BuildChat => "build_chat",
            Self::UtteranceSegmentation => "utterance_segmentation",
            Self::Morphosyntax => "morphosyntax",
            Self::ForcedAlignment => "forced_alignment",
            Self::BatchInfer => "batch_infer",
            Self::CompareAlign => "compare_align",
            Self::CompareMetrics => "compare_metrics",
            Self::MediaAnalysis => "media_analysis",
            Self::RunTranscribeRecipe => "run_transcribe_recipe",
            Self::RunCompareRecipe => "run_compare_recipe",
            Self::SerializeChat => "serialize_chat",
            Self::MaterializeOutputs => "materialize_outputs",
        }
    }
}

impl fmt::Display for RecipeStageId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Whether a stage is always present or controlled by command options.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RecipeStagePresence {
    /// The stage must always run.
    Required,
    /// The stage is part of the recipe but option-gated at runtime.
    Optional,
}

/// Whether a stage runs per work unit, across work units, or by delegating.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum StageExecutionKind {
    /// Each work unit runs the stage independently.
    PerWorkUnit,
    /// The stage pools multiple work units into one dispatch.
    BatchedAcrossWorkUnits,
    /// The stage delegates to another recipe.
    CompositeSubrecipe,
}

/// Static metadata for one recipe stage.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct RecipeStage {
    /// Stable stage id.
    pub id: RecipeStageId,
    /// Whether the stage is always present.
    pub presence: RecipeStagePresence,
    /// Execution shape for this stage.
    pub execution: StageExecutionKind,
    /// Progress label surfaced to operators.
    pub progress_stage: FileStage,
    /// Other stages that must complete first.
    pub depends_on: &'static [RecipeStageId],
}

impl RecipeStage {
    /// Construct one static recipe stage.
    pub(crate) const fn new(
        id: RecipeStageId,
        presence: RecipeStagePresence,
        execution: StageExecutionKind,
        progress_stage: FileStage,
        depends_on: &'static [RecipeStageId],
    ) -> Self {
        Self {
            id,
            presence,
            execution,
            progress_stage,
            depends_on,
        }
    }
}

/// Static recipe metadata for one released command.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Recipe {
    /// Top-level execution mode for the command.
    pub mode: ExecutionMode,
    /// Ordered recipe stages.
    pub stages: &'static [RecipeStage],
}

impl Recipe {
    /// Validate the recipe metadata for one released command.
    #[cfg(test)]
    pub(crate) fn validate(&self, command: ReleasedCommand) -> Result<(), ServerError> {
        let mut seen = BTreeSet::new();
        for stage in self.stages {
            if !seen.insert(stage.id) {
                return Err(ServerError::Validation(format!(
                    "recipe for {command} has duplicate stage {}",
                    stage.id
                )));
            }
        }

        for stage in self.stages {
            for dependency in stage.depends_on {
                if *dependency == stage.id {
                    return Err(ServerError::Validation(format!(
                        "recipe for {command} has self-dependency on stage {}",
                        stage.id
                    )));
                }
                if !seen.contains(dependency) {
                    return Err(ServerError::Validation(format!(
                        "recipe for {command} references missing dependency {dependency} from stage {}",
                        stage.id
                    )));
                }
            }
        }

        Ok(())
    }

    /// Return the ordered stage ids for inspection tests and docs.
    #[cfg(test)]
    pub(crate) fn ordered_stage_ids(&self) -> Vec<RecipeStageId> {
        self.stages.iter().map(|stage| stage.id).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const VALID_STAGES: &[RecipeStage] = &[
        RecipeStage::new(
            RecipeStageId::PlanWorkUnits,
            RecipeStagePresence::Required,
            StageExecutionKind::PerWorkUnit,
            FileStage::Reading,
            &[],
        ),
        RecipeStage::new(
            RecipeStageId::MaterializeOutputs,
            RecipeStagePresence::Required,
            StageExecutionKind::PerWorkUnit,
            FileStage::Writing,
            &[RecipeStageId::PlanWorkUnits],
        ),
    ];

    const DUPLICATE_STAGES: &[RecipeStage] = &[
        RecipeStage::new(
            RecipeStageId::PlanWorkUnits,
            RecipeStagePresence::Required,
            StageExecutionKind::PerWorkUnit,
            FileStage::Reading,
            &[],
        ),
        RecipeStage::new(
            RecipeStageId::PlanWorkUnits,
            RecipeStagePresence::Required,
            StageExecutionKind::PerWorkUnit,
            FileStage::Writing,
            &[],
        ),
    ];

    #[test]
    fn recipe_validation_accepts_unique_known_dependencies() {
        let recipe = Recipe {
            mode: ExecutionMode::SequentialPerUnit,
            stages: VALID_STAGES,
        };
        recipe
            .validate(ReleasedCommand::Transcribe)
            .expect("valid recipe");
    }

    #[test]
    fn recipe_validation_rejects_duplicate_stage_ids() {
        let recipe = Recipe {
            mode: ExecutionMode::SequentialPerUnit,
            stages: DUPLICATE_STAGES,
        };
        let error = recipe
            .validate(ReleasedCommand::Compare)
            .expect_err("duplicate id should fail");
        assert!(error.to_string().contains("duplicate stage"));
    }
}
