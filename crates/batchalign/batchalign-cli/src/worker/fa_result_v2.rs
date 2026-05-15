//! Rust-side adapters for staged worker-protocol V2 forced-alignment results.
//!
//! The staged Python V2 FA executor now returns typed `ExecuteResponseV2`
//! payloads, but the production FA pipeline still expects the existing
//! `batchalign-chat-ops` alignment domain. This module is the bridge:
//!
//! - accept a typed V2 worker response
//! - normalize it into the established FA raw-response shape
//! - reuse the existing deterministic Rust alignment and timing parser
//!
//! That keeps model-output interpretation on the Rust side while the transport
//! migration is still staged.

use crate::chat_ops::fa::{FaTimingMode, FaWord, WordTiming, parse_fa_response};
use crate::chat_ops::nlp::{FaIndexedTiming, FaRawResponse, FaRawToken};

use crate::api::DurationMs;
use crate::types::worker_v2::{ExecuteOutcomeV2, ExecuteResponseV2, TaskResultV2};

/// Parse one staged V2 FA execute response into the established FA timing
/// domain.
pub fn parse_forced_alignment_result_v2(
    response: &ExecuteResponseV2,
    original_words: &[FaWord],
    audio_start_ms: DurationMs,
    timing_mode: FaTimingMode,
) -> Result<Vec<Option<WordTiming>>, String> {
    match &response.outcome {
        ExecuteOutcomeV2::Success => {}
        ExecuteOutcomeV2::Error { code, message } => {
            return Err(format!(
                "worker protocol V2 forced-alignment request failed with {code:?}: {message}"
            ));
        }
    }

    let Some(result) = &response.result else {
        return Err(
            "worker protocol V2 forced-alignment response was missing a result payload".into(),
        );
    };

    let raw_response = match result {
        TaskResultV2::WhisperTokenTimingResult(result) => FaRawResponse::TokenLevel {
            tokens: result
                .tokens
                .iter()
                .map(|token| FaRawToken {
                    text: token.text.clone(),
                    time_s: token.time_s.0,
                })
                .collect(),
        },
        TaskResultV2::IndexedWordTimingResult(result) => FaRawResponse::IndexedWordLevel {
            indexed_timings: result
                .indexed_timings
                .iter()
                .map(|timing| {
                    timing.as_ref().map(|timing| FaIndexedTiming {
                        start_ms: timing.start_ms.0,
                        end_ms: timing.end_ms.0,
                        confidence: timing.confidence,
                    })
                })
                .collect(),
        },
        TaskResultV2::MorphosyntaxResult(_) => {
            return Err(
                "worker protocol V2 forced-alignment response returned morphosyntax data".into(),
            );
        }
        TaskResultV2::UtsegResult(_) => {
            return Err(
                "worker protocol V2 forced-alignment response returned utterance-segmentation data"
                    .into(),
            );
        }
        TaskResultV2::WhisperChunkResult(_) => {
            return Err(
                "worker protocol V2 forced-alignment response returned ASR chunk data".into(),
            );
        }
        TaskResultV2::TranslationResult(_) => {
            return Err(
                "worker protocol V2 forced-alignment response returned translation data".into(),
            );
        }
        TaskResultV2::CorefResult(_) => {
            return Err(
                "worker protocol V2 forced-alignment response returned coreference data".into(),
            );
        }
        TaskResultV2::MonologueAsrResult(_) => {
            return Err(
                "worker protocol V2 forced-alignment response returned monologue ASR data".into(),
            );
        }
        TaskResultV2::SpeakerResult(_) => {
            return Err(
                "worker protocol V2 forced-alignment response returned speaker diarization data"
                    .into(),
            );
        }
        TaskResultV2::OpensmileResult(_) => {
            return Err(
                "worker protocol V2 forced-alignment response returned openSMILE feature data"
                    .into(),
            );
        }
        TaskResultV2::AvqiResult(_) => {
            return Err(
                "worker protocol V2 forced-alignment response returned AVQI feature data".into(),
            );
        }
    };

    let raw_json = serde_json::to_string(&raw_response).map_err(|error| {
        format!("failed to serialize staged FA V2 response for parsing: {error}")
    })?;
    // Wave 5 consolidation: parse_fa_response returns a typed
    // FaAlignmentError. The surrounding V2-adapter layer still returns
    // `Result<_, String>`; convert at this boundary so the adapter
    // contract stays stable. A future follow-up can propagate the
    // typed error further up.
    parse_fa_response(&raw_json, original_words, audio_start_ms.0, timing_mode)
        .map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chat_ops::fa::{FaTimingMode, FaWord};
    use crate::chat_ops::{UtteranceIdx, WordIdx};

    use crate::api::{DurationMs, DurationSeconds};
    use crate::types::worker_v2::{
        ExecuteOutcomeV2, ExecuteResponseV2, IndexedWordTimingResultV2, IndexedWordTimingV2,
        TaskResultV2, WhisperTokenTimingResultV2, WhisperTokenTimingV2, WorkerRequestIdV2,
    };

    /// Build `FaWord` values for small result-adapter tests.
    fn make_words(texts: &[&str]) -> Vec<FaWord> {
        texts
            .iter()
            .enumerate()
            .map(|(index, text)| FaWord {
                utterance_index: UtteranceIdx(0),
                utterance_word_index: WordIdx(index),
                text: (*text).into(),
            })
            .collect()
    }

    #[test]
    fn parses_whisper_token_result_into_established_alignment_domain() {
        let response = ExecuteResponseV2 {
            request_id: WorkerRequestIdV2::from("req-fa-v2-1"),
            outcome: ExecuteOutcomeV2::Success,
            result: Some(TaskResultV2::WhisperTokenTimingResult(
                WhisperTokenTimingResultV2 {
                    tokens: vec![
                        WhisperTokenTimingV2 {
                            text: "hello".into(),
                            time_s: DurationSeconds(0.10),
                        },
                        WhisperTokenTimingV2 {
                            text: "world".into(),
                            time_s: DurationSeconds(0.35),
                        },
                    ],
                },
            )),
            elapsed_s: DurationSeconds(0.01),
        };

        let timings = parse_forced_alignment_result_v2(
            &response,
            &make_words(&["hello", "world"]),
            DurationMs(1_000),
            FaTimingMode::Continuous,
        )
        .expect("V2 whisper token result should parse");

        assert_eq!(timings.len(), 2);
        assert_eq!(timings[0].as_ref().expect("timing").start_ms, 1_100);
        assert_eq!(timings[1].as_ref().expect("timing").start_ms, 1_350);
    }

    #[test]
    fn parses_indexed_timing_result_into_established_alignment_domain() {
        let response = ExecuteResponseV2 {
            request_id: WorkerRequestIdV2::from("req-fa-v2-2"),
            outcome: ExecuteOutcomeV2::Success,
            result: Some(TaskResultV2::IndexedWordTimingResult(
                IndexedWordTimingResultV2 {
                    indexed_timings: vec![
                        Some(IndexedWordTimingV2 {
                            start_ms: DurationMs(25),
                            end_ms: DurationMs(75),
                            confidence: Some(0.9),
                        }),
                        None,
                    ],
                },
            )),
            elapsed_s: DurationSeconds(0.01),
        };

        let timings = parse_forced_alignment_result_v2(
            &response,
            &make_words(&["hello", "world"]),
            DurationMs(500),
            FaTimingMode::WithPauses,
        )
        .expect("V2 indexed timing result should parse");

        assert_eq!(timings[0].as_ref().expect("timing").start_ms, 525);
        assert!(timings[1].is_none());
    }

    #[test]
    fn rejects_non_fa_result_payloads() {
        let response = ExecuteResponseV2 {
            request_id: WorkerRequestIdV2::from("req-fa-v2-3"),
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

        let error = parse_forced_alignment_result_v2(
            &response,
            &make_words(&["hello"]),
            DurationMs(0),
            FaTimingMode::Continuous,
        )
        .expect_err("translation result should be rejected");

        assert!(error.contains("translation data"));
    }
}
