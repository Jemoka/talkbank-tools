//! Server-side benchmark orchestrator.
//!
//! Benchmarking is conceptually "transcribe, then compare against a gold CHAT
//! companion". Neither half requires Python-side document orchestration:
//! - transcription already has a Rust-owned pipeline around raw ASR inference
//! - comparison is already Rust-owned morphosyntax + DP alignment
//!
//! This module composes those two existing Rust pipelines so the `benchmark`
//! command no longer depends on a fictitious Python worker benchmark path.

use std::path::Path;

use crate::chat_ops::morphosyntax_ops::MwtDict;

use crate::api::{ChatText, LanguageCode3};
use crate::error::ServerError;
use crate::pipeline::PipelineServices;
use crate::runner::util::ProgressSender;
use crate::transcribe::TranscribeOptions;

pub(crate) use crate::compare::MainAnnotatedCompareOutputs as BenchmarkOutputs;

/// Borrowed request bundle for one benchmark execution.
pub(crate) struct BenchmarkRequest<'a> {
    /// Audio file to transcribe before comparison.
    pub audio_path: &'a Path,
    /// Gold-standard CHAT transcript to compare against.
    pub gold_text: ChatText<'a>,
    /// Primary language used for comparison and downstream NLP shaping.
    pub lang: &'a LanguageCode3,
    /// Shared worker/cache services used by the transcribe and compare phases.
    pub services: PipelineServices<'a>,
    /// Typed transcription options for the Rust-owned transcribe pipeline.
    pub transcribe_options: &'a TranscribeOptions,
    /// Multi-word-token dictionary shared with the compare pipeline.
    pub mwt: &'a MwtDict,
    /// Optional progress sink for the transcribe sub-pipeline.
    pub progress: Option<&'a ProgressSender>,
}

/// Run the benchmark pipeline for one audio file and one gold CHAT transcript.
///
/// Returns a [`BenchmarkOutputs`] containing the benchmark CHAT and CSV metrics.
pub(crate) async fn process_benchmark(
    request: BenchmarkRequest<'_>,
) -> Result<BenchmarkOutputs, ServerError> {
    let transcribed_chat = crate::transcribe::process_transcribe(
        request.audio_path,
        request.services,
        request.transcribe_options,
        request.progress.cloned(),
        None,
    )
    .await?;

    crate::compare::process_compare_main_annotated(
        &transcribed_chat,
        request.gold_text.as_ref(),
        request.lang,
        request.services,
        request.mwt,
    )
    .await
}

/// Derive the companion gold CHAT path for one audio file.
///
/// Convention:
/// - `sample.wav` -> `sample.cha`
/// - `/dir/sample.mp3` -> `/dir/sample.cha`
#[cfg(test)]
pub(crate) fn gold_chat_path_for_audio(audio_path: &str) -> String {
    let path = Path::new(audio_path);
    let stem = path.file_stem().unwrap_or_default().to_string_lossy();
    let parent = path.parent().unwrap_or_else(|| Path::new(""));
    parent
        .join(format!("{stem}.cha"))
        .to_string_lossy()
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::gold_chat_path_for_audio;

    #[test]
    fn derives_gold_chat_path_from_audio() {
        assert_eq!(gold_chat_path_for_audio("sample.wav"), "sample.cha");
        assert_eq!(
            gold_chat_path_for_audio("/data/interview.mp3"),
            "/data/interview.cha"
        );
    }
}
