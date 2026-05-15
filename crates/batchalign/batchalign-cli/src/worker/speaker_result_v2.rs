//! Rust-side adapters for live worker-protocol V2 speaker results.
//!
//! Speaker diarization is still a raw-model concern at the worker boundary.
//! This adapter keeps Rust in charge of interpreting the typed response shape
//! instead of letting downstream callers pattern-match on generic JSON.

use crate::types::worker_v2::{ExecuteOutcomeV2, ExecuteResponseV2, SpeakerResultV2, TaskResultV2};

/// Parse one V2 speaker execute response into the typed segment list.
pub fn parse_speaker_result_v2(response: &ExecuteResponseV2) -> Result<&SpeakerResultV2, String> {
    match &response.outcome {
        ExecuteOutcomeV2::Success => {}
        ExecuteOutcomeV2::Error { code, message } => {
            return Err(format!(
                "worker protocol V2 speaker request failed with {code:?}: {message}"
            ));
        }
    }

    let Some(result) = &response.result else {
        return Err("worker protocol V2 speaker response was missing a result payload".into());
    };

    match result {
        TaskResultV2::SpeakerResult(result) => Ok(result),
        TaskResultV2::MorphosyntaxResult(_) => {
            Err("worker protocol V2 speaker response returned morphosyntax data".into())
        }
        TaskResultV2::UtsegResult(_) => {
            Err("worker protocol V2 speaker response returned utterance-segmentation data".into())
        }
        TaskResultV2::WhisperChunkResult(_) => {
            Err("worker protocol V2 speaker response returned ASR chunk data".into())
        }
        TaskResultV2::MonologueAsrResult(_) => {
            Err("worker protocol V2 speaker response returned monologue ASR data".into())
        }
        TaskResultV2::WhisperTokenTimingResult(_) => {
            Err("worker protocol V2 speaker response returned forced-alignment token data".into())
        }
        TaskResultV2::IndexedWordTimingResult(_) => {
            Err("worker protocol V2 speaker response returned indexed timing data".into())
        }
        TaskResultV2::TranslationResult(_) => {
            Err("worker protocol V2 speaker response returned translation data".into())
        }
        TaskResultV2::CorefResult(_) => {
            Err("worker protocol V2 speaker response returned coreference data".into())
        }
        TaskResultV2::OpensmileResult(_) => {
            Err("worker protocol V2 speaker response returned openSMILE feature data".into())
        }
        TaskResultV2::AvqiResult(_) => {
            Err("worker protocol V2 speaker response returned AVQI feature data".into())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::{DurationMs, DurationSeconds};
    use crate::types::worker_v2::{
        ExecuteResponseV2, SpeakerResultV2, SpeakerSegmentV2, WorkerRequestIdV2,
    };

    #[test]
    fn parses_speaker_segments_from_typed_v2_result() {
        let response = ExecuteResponseV2 {
            request_id: WorkerRequestIdV2::from("req-speaker-v2-1"),
            outcome: ExecuteOutcomeV2::Success,
            result: Some(TaskResultV2::SpeakerResult(SpeakerResultV2 {
                segments: vec![SpeakerSegmentV2 {
                    start_ms: DurationMs(0),
                    end_ms: DurationMs(900),
                    speaker: "SPEAKER_1".into(),
                }],
            })),
            elapsed_s: DurationSeconds(0.01),
        };

        let parsed = parse_speaker_result_v2(&response).expect("speaker result should parse");
        assert_eq!(parsed.segments[0].speaker, "SPEAKER_1");
        assert_eq!(parsed.segments[0].end_ms, DurationMs(900));
    }

    #[test]
    fn rejects_non_speaker_v2_payloads() {
        let response = ExecuteResponseV2 {
            request_id: WorkerRequestIdV2::from("req-speaker-v2-2"),
            outcome: ExecuteOutcomeV2::Success,
            result: Some(TaskResultV2::TranslationResult(
                crate::types::worker_v2::TranslationResultV2 {
                    items: vec![crate::types::worker_v2::TranslationItemResultV2 {
                        raw_translation: Some("hola".into()),
                        error: None,
                    }],
                },
            )),
            elapsed_s: DurationSeconds(0.01),
        };

        let error =
            parse_speaker_result_v2(&response).expect_err("translation result should be rejected");
        assert!(error.contains("translation data"));
    }
}
