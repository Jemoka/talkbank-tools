//! Canonical runner-owned stage labels for file lifecycles.
//!
//! This keeps the control-plane stage vocabulary typed all the way through the
//! runner and store layers before the API derives an operator-facing label.

use crate::api::{FileProgressStage, ReleasedCommand};

/// Canonical runner-owned stage labels for file lifecycles.
///
/// This keeps the control-plane stage vocabulary typed all the way through the
/// runner and store layers before the API derives an operator-facing label.
/// The enum intentionally covers both top-level runner stages and the
/// lower-level FA/transcribe pipeline progress labels that feed the same file
/// status channel.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum FileStage {
    /// Generic processing for worker-owned media commands.
    Processing,
    /// Initial file read/setup work.
    Reading,
    /// Media discovery or normalization.
    ResolvingAudio,
    /// Utterance-level timing recovery before alignment.
    RecoveringUtteranceTiming,
    /// Fallback timing recovery after an FA failure.
    RecoveringTimingFallback,
    /// Main forced-alignment stage.
    Aligning,
    /// Main transcription stage.
    Transcribing,
    /// Main benchmark stage.
    Benchmarking,
    /// Cache partition / cache lookup stage inside FA.
    CheckingCache,
    /// Apply aligned timings or inferred annotations back into the document.
    ApplyingResults,
    /// ASR output post-processing before CHAT construction.
    PostProcessing,
    /// Build a CHAT document from intermediate utterance state.
    BuildingChat,
    /// Utterance segmentation within the transcribe pipeline.
    SegmentingUtterances,
    /// Morphosyntax enrichment within the transcribe pipeline.
    AnalyzingMorphosyntax,
    /// Final pipeline serialization/finalization step.
    Finalizing,
    /// Final serialization/write stage.
    Writing,
    /// Batched morphosyntax analysis.
    Analyzing,
    /// Batched utterance segmentation.
    Segmenting,
    /// Batched translation.
    Translating,
    /// Batched coreference resolution.
    ResolvingCoreference,
    /// Batched transcript/reference comparison.
    Comparing,
}

impl FileStage {
    /// Resolve the initial batch-infer stage for a top-level command.
    pub(crate) fn for_batch_command(command: ReleasedCommand) -> Self {
        match command {
            ReleasedCommand::Morphotag => Self::Analyzing,
            ReleasedCommand::Utseg => Self::Segmenting,
            ReleasedCommand::Translate => Self::Translating,
            ReleasedCommand::Coref => Self::ResolvingCoreference,
            ReleasedCommand::Compare => Self::Comparing,
            _ => Self::Processing,
        }
    }

    /// Convert the runner-local stage vocabulary to the stable API enum.
    pub(crate) const fn api_stage(self) -> FileProgressStage {
        match self {
            Self::Processing => FileProgressStage::Processing,
            Self::Reading => FileProgressStage::Reading,
            Self::ResolvingAudio => FileProgressStage::ResolvingAudio,
            Self::RecoveringUtteranceTiming => FileProgressStage::RecoveringUtteranceTiming,
            Self::RecoveringTimingFallback => FileProgressStage::RecoveringTimingFallback,
            Self::Aligning => FileProgressStage::Aligning,
            Self::Transcribing => FileProgressStage::Transcribing,
            Self::Benchmarking => FileProgressStage::Benchmarking,
            Self::CheckingCache => FileProgressStage::CheckingCache,
            Self::ApplyingResults => FileProgressStage::ApplyingResults,
            Self::PostProcessing => FileProgressStage::PostProcessing,
            Self::BuildingChat => FileProgressStage::BuildingChat,
            Self::SegmentingUtterances => FileProgressStage::SegmentingUtterances,
            Self::AnalyzingMorphosyntax => FileProgressStage::AnalyzingMorphosyntax,
            Self::Finalizing => FileProgressStage::Finalizing,
            Self::Writing => FileProgressStage::Writing,
            Self::Analyzing => FileProgressStage::Analyzing,
            Self::Segmenting => FileProgressStage::Segmenting,
            Self::Translating => FileProgressStage::Translating,
            Self::ResolvingCoreference => FileProgressStage::ResolvingCoreference,
            Self::Comparing => FileProgressStage::Comparing,
        }
    }
}
