//! Typed work-unit shapes for the recipe-runner spike.

use std::path::PathBuf;

use crate::api::DisplayPath;

/// One discovered source input before command-specific planning.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DiscoveredInput {
    /// Relative display path tracked by the job system.
    pub display_path: DisplayPath,
    /// Concrete source path read by the runner.
    pub source_path: PathBuf,
    /// Optional `--before` companion for incremental processing.
    pub before_path: Option<PathBuf>,
}

impl DiscoveredInput {
    /// Construct a discovered input without a `--before` companion.
    #[cfg(test)]
    pub(crate) fn new(
        display_path: impl Into<DisplayPath>,
        source_path: impl Into<PathBuf>,
    ) -> Self {
        Self {
            display_path: display_path.into(),
            source_path: source_path.into(),
            before_path: None,
        }
    }
}

/// Text-only commands that transform one CHAT input.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TextWorkUnit {
    /// Source CHAT input.
    pub source: DiscoveredInput,
}

/// Audio-first commands such as transcribe and align.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AudioWorkUnit {
    /// Source audio input.
    pub audio: DiscoveredInput,
}

/// Paired main/reference transcript work unit for compare.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CompareWorkUnit {
    /// Main transcript to annotate.
    pub main: DiscoveredInput,
    /// Gold/reference transcript.
    pub gold: DiscoveredInput,
}

/// Audio input plus derived gold CHAT companion for benchmark.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct BenchmarkWorkUnit {
    /// Source audio input.
    pub audio: DiscoveredInput,
    /// Gold CHAT transcript expected for the benchmark.
    pub gold_chat: DiscoveredInput,
}

/// Media-analysis work unit for openSMILE or AVQI.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct MediaAnalysisWorkUnit {
    /// Source media input.
    pub source: DiscoveredInput,
}

/// Command-family-specific planned work units.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum PlannedWorkUnit {
    /// One text transform unit.
    Text(TextWorkUnit),
    /// One audio transform unit.
    Audio(AudioWorkUnit),
    /// One compare pair.
    Compare(CompareWorkUnit),
    /// One benchmark pair.
    Benchmark(BenchmarkWorkUnit),
    /// One media-analysis unit.
    MediaAnalysis(MediaAnalysisWorkUnit),
}
