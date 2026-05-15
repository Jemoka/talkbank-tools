//! Rust-side adapters for live worker-protocol V2 ASR results.
//!
//! The live V2 ASR worker path now returns typed `ExecuteResponseV2` payloads,
//! but the transcription pipeline still expects the established Rust
//! `AsrResponse` domain. This module keeps that normalization in Rust.

use crate::api::LanguageCode3;
use crate::transcribe::{AsrResponse, AsrToken};
use crate::types::worker_v2::{
    AsrElementKindV2, ExecuteOutcomeV2, ExecuteResponseV2, TaskResultV2,
};
use talkbank_transform::asr_postprocess::{
    AsrElement, AsrElementKind, AsrMonologue, AsrOutput, AsrRawText, AsrTimestampSecs, SpeakerIndex,
};
use tracing::warn;

/// Parse one live V2 ASR execute response into the established Rust ASR
/// domain.
///
/// `fallback_lang` is consulted only when the worker's response carries
/// an empty language string. Pass `Some(code)` for `Resolved(code)` jobs;
/// pass `None` for `Auto` jobs — in which case an empty worker response
/// becomes a typed error rather than a silent eng substitution.
pub fn parse_asr_response_v2(
    response: &ExecuteResponseV2,
    fallback_lang: Option<&LanguageCode3>,
) -> Result<AsrResponse, String> {
    match &response.outcome {
        ExecuteOutcomeV2::Success => {}
        ExecuteOutcomeV2::Error { code, message } => {
            return Err(format!(
                "worker protocol V2 ASR request failed with {code:?}: {message}"
            ));
        }
    }

    let Some(result) = &response.result else {
        return Err("worker protocol V2 ASR response was missing a result payload".into());
    };

    match result {
        TaskResultV2::WhisperChunkResult(result) => Ok(AsrResponse {
            lang: resolve_worker_lang(&result.lang, fallback_lang)?,
            tokens: result
                .chunks
                .iter()
                .filter_map(|chunk| {
                    let text = chunk.text.trim();
                    if text.is_empty() {
                        return None;
                    }

                    Some(AsrToken {
                        text: text.to_string(),
                        start_s: Some(chunk.start_s),
                        end_s: Some(chunk.end_s),
                        speaker: None,
                        confidence: None,
                    })
                })
                .collect(),
            source_monologues: None,
        }),
        TaskResultV2::MonologueAsrResult(result) => Ok(AsrResponse {
            lang: resolve_worker_lang(&result.lang, fallback_lang)?,
            tokens: result
                .monologues
                .iter()
                .flat_map(|monologue| {
                    monologue.elements.iter().filter_map(|element| {
                        if element.kind != AsrElementKindV2::Text {
                            return None;
                        }

                        let text = element.value.trim();
                        if text.is_empty() {
                            return None;
                        }

                        Some(AsrToken {
                            text: text.to_string(),
                            start_s: element.start_s,
                            end_s: element.end_s,
                            speaker: Some(monologue.speaker.clone()),
                            confidence: element.confidence,
                        })
                    })
                })
                .collect(),
            source_monologues: Some(
                AsrOutput {
                    monologues: result
                        .monologues
                        .iter()
                        .map(|monologue| AsrMonologue {
                            speaker: SpeakerIndex(
                                monologue.speaker.parse::<usize>().unwrap_or_else(|_| {
                                    warn!(
                                        speaker = %monologue.speaker,
                                        "unparseable V2 monologue speaker label, defaulting to speaker 0"
                                    );
                                    0
                                }),
                            ),
                            elements: monologue
                                .elements
                                .iter()
                                .filter_map(|element| {
                                    let text = element.value.trim();
                                    if text.is_empty() {
                                        return None;
                                    }
                                    Some(AsrElement {
                                        value: AsrRawText::new(text),
                                        ts: AsrTimestampSecs(
                                            element.start_s.map(|ts| ts.0).unwrap_or_else(|| {
                                                warn!(
                                                    token = text,
                                                    "V2 monologue element missing start timestamp, defaulting to 0.0s"
                                                );
                                                0.0
                                            }),
                                        ),
                                        end_ts: AsrTimestampSecs(
                                            element.end_s.map(|ts| ts.0).unwrap_or_else(|| {
                                                warn!(
                                                    token = text,
                                                    "V2 monologue element missing end timestamp, defaulting to 0.0s"
                                                );
                                                0.0
                                            }),
                                        ),
                                        kind: match element.kind {
                                            AsrElementKindV2::Text => AsrElementKind::Text,
                                            AsrElementKindV2::Punctuation => {
                                                AsrElementKind::Punctuation
                                            }
                                        },
                                    })
                                })
                                .collect(),
                        })
                        .collect(),
                }
                .monologues,
            ),
        }),
        TaskResultV2::WhisperTokenTimingResult(_) => {
            Err("worker protocol V2 ASR response returned forced-alignment token data".into())
        }
        TaskResultV2::IndexedWordTimingResult(_) => {
            Err("worker protocol V2 ASR response returned indexed timing data".into())
        }
        TaskResultV2::MorphosyntaxResult(_) => {
            Err("worker protocol V2 ASR response returned morphosyntax data".into())
        }
        TaskResultV2::UtsegResult(_) => {
            Err("worker protocol V2 ASR response returned utterance-segmentation data".into())
        }
        TaskResultV2::TranslationResult(_) => {
            Err("worker protocol V2 ASR response returned translation data".into())
        }
        TaskResultV2::CorefResult(_) => {
            Err("worker protocol V2 ASR response returned coreference data".into())
        }
        TaskResultV2::SpeakerResult(_) => {
            Err("worker protocol V2 ASR response returned speaker diarization data".into())
        }
        TaskResultV2::OpensmileResult(_) => {
            Err("worker protocol V2 ASR response returned openSMILE feature data".into())
        }
        TaskResultV2::AvqiResult(_) => {
            Err("worker protocol V2 ASR response returned AVQI feature data".into())
        }
    }
}

/// Resolve a worker-provided language against the control-plane fallback.
///
/// When the worker's response carries an empty language string, fall
/// back to `fallback_lang` if the caller supplied one. If the caller
/// supplied `None` (an `Auto` job with no resolved language yet) and
/// the worker also returned nothing, surface a typed error so the
/// caller writes nothing to the output's `@Languages:` rather than
/// silently stamping English.
fn resolve_worker_lang(
    worker_lang: &LanguageCode3,
    fallback_lang: Option<&LanguageCode3>,
) -> Result<LanguageCode3, String> {
    if worker_lang.trim().is_empty() {
        fallback_lang.cloned().ok_or_else(|| {
            "ASR worker returned an empty language and the job has no resolved \
             fallback (`--lang auto` with no Rev.AI/Whisper-detected language). \
             Re-run with an explicit `--lang <iso3>` instead of `--lang auto`."
                .to_string()
        })
    } else {
        Ok(worker_lang.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::{DurationSeconds, LanguageCode3};
    use crate::types::worker_v2::{
        AsrElementKindV2, AsrElementV2, AsrMonologueV2, ExecuteOutcomeV2, ExecuteResponseV2,
        MonologueAsrResultV2, TaskResultV2, WhisperChunkResultV2, WhisperChunkSpanV2,
        WorkerRequestIdV2,
    };

    #[test]
    fn parses_whisper_chunk_result_into_established_asr_domain() {
        let response = ExecuteResponseV2 {
            request_id: WorkerRequestIdV2::from("req-asr-v2-1"),
            outcome: ExecuteOutcomeV2::Success,
            result: Some(TaskResultV2::WhisperChunkResult(WhisperChunkResultV2 {
                lang: LanguageCode3::eng(),
                text: "hello world".into(),
                chunks: vec![
                    WhisperChunkSpanV2 {
                        text: "hello".into(),
                        start_s: DurationSeconds(0.0),
                        end_s: DurationSeconds(0.5),
                    },
                    WhisperChunkSpanV2 {
                        text: "world".into(),
                        start_s: DurationSeconds(0.5),
                        end_s: DurationSeconds(1.0),
                    },
                ],
            })),
            elapsed_s: DurationSeconds(0.01),
        };

        let parsed = parse_asr_response_v2(&response, Some(&LanguageCode3::eng()))
            .expect("V2 ASR response should parse");

        assert_eq!(parsed.lang, "eng");
        assert_eq!(parsed.tokens.len(), 2);
        assert_eq!(parsed.tokens[0].text, "hello");
        assert_eq!(parsed.tokens[1].end_s, Some(DurationSeconds(1.0)));
    }

    #[test]
    fn parses_monologue_result_into_established_asr_domain() {
        let response = ExecuteResponseV2 {
            request_id: WorkerRequestIdV2::from("req-asr-v2-provider"),
            outcome: ExecuteOutcomeV2::Success,
            result: Some(TaskResultV2::MonologueAsrResult(MonologueAsrResultV2 {
                lang: LanguageCode3::yue(),
                monologues: vec![AsrMonologueV2 {
                    speaker: "1".into(),
                    elements: vec![
                        AsrElementV2 {
                            value: "nei5".into(),
                            start_s: Some(DurationSeconds(0.1)),
                            end_s: Some(DurationSeconds(0.4)),
                            kind: AsrElementKindV2::Text,
                            confidence: Some(0.9),
                        },
                        AsrElementV2 {
                            value: ",".into(),
                            start_s: None,
                            end_s: None,
                            kind: AsrElementKindV2::Punctuation,
                            confidence: None,
                        },
                        AsrElementV2 {
                            value: "hou2".into(),
                            start_s: Some(DurationSeconds(0.5)),
                            end_s: Some(DurationSeconds(0.8)),
                            kind: AsrElementKindV2::Text,
                            confidence: None,
                        },
                    ],
                }],
            })),
            elapsed_s: DurationSeconds(0.01),
        };

        let parsed = parse_asr_response_v2(&response, Some(&LanguageCode3::eng()))
            .expect("V2 monologue response should parse");

        assert_eq!(parsed.lang, "yue");
        assert_eq!(parsed.tokens.len(), 2);
        assert_eq!(parsed.tokens[0].speaker.as_deref(), Some("1"));
        assert_eq!(parsed.tokens[0].confidence, Some(0.9));
        assert_eq!(parsed.tokens[1].text, "hou2");
    }
}
