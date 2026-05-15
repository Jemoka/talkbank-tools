//! Rust helpers for Hong Kong ASR provider projection.
//!
//! **See also:** [INTERFACE_MAP.md](../INTERFACE_MAP.md) section "9. HK/Cantonese ASR Bridges" for:
//! - Python callers: `python/batchalign/inference/languages/cantonese/`, `python/batchalign/worker/_asr_v2.py`
//! - Design: Projects provider-specific output into common MonologueAsrResultV2 shapes.
//!
//! The HK/Tencent/FunAudio Python adapters must still talk to Python-only SDKs
//! and model objects, but the shared result shaping should live in Rust. This
//! module owns:
//!
//! - FunASR text cleanup and token/timestamp projection
//! - Tencent result-detail projection into monologues and timed words
//! - Aliyun sentence-result projection and fallback tokenization
//! - Cantonese normalization at the provider boundary
//! - Cantonese character tokenization for per-character timestamp alignment
//!
//! Python code now forwards raw provider output into these helpers instead of
//! reimplementing the projection loops itself.

use crate::error::BatchalignBoundaryError;
use crate::py_json_bridge::py_to_json_value;
use pyo3::prelude::*;
use pyo3::types::PyList;
use serde::{Deserialize, Serialize};
use talkbank_transform::asr_postprocess::cantonese as cantonese_ops;

/// Speaker-attributed ASR projection shared by the HK provider bridges.
#[derive(Debug, Clone, Serialize, PartialEq)]
struct HkAsrProjection {
    /// Speaker monologues ready for the shared ASR worker contract.
    monologues: Vec<HkAsrMonologue>,
    /// Flat timed words for UTR-style timing injection paths.
    timed_words: Vec<HkTimedWord>,
}

/// One speaker monologue in the HK ASR projection.
#[derive(Debug, Clone, Serialize, PartialEq)]
struct HkAsrMonologue {
    /// Speaker index reported by the provider or synthesized by the adapter.
    speaker: usize,
    /// Ordered ASR elements in this speaker span.
    elements: Vec<HkAsrElement>,
}

/// One token entry in a projected HK ASR monologue.
#[derive(Debug, Clone, Serialize, PartialEq)]
struct HkAsrElement {
    /// Token kind for the shared worker contract.
    #[serde(rename = "type")]
    element_type: &'static str,
    /// Start time in seconds when known.
    ts: Option<f64>,
    /// End time in seconds when known.
    end_ts: Option<f64>,
    /// Surface token value after provider-boundary normalization.
    value: String,
}

/// One timed word emitted by a provider projection.
#[derive(Debug, Clone, Serialize, PartialEq)]
struct HkTimedWord {
    /// Surface token value.
    word: String,
    /// Start time in milliseconds.
    start_ms: i64,
    /// End time in milliseconds.
    end_ms: i64,
}

/// Minimal FunASR segment payload extracted from the Python model output.
#[derive(Debug, Clone, Deserialize)]
struct FunaudioSegmentInput {
    /// Raw segment text returned by FunASR.
    #[serde(default)]
    text: String,
    /// Raw timestamp entries aligned to the model tokens when available.
    #[serde(default)]
    timestamp: Vec<serde_json::Value>,
}

/// One word record from Tencent `ResultDetail`.
#[derive(Debug, Clone, PartialEq)]
struct TencentWordInput {
    /// Surface word returned by Tencent.
    word: String,
    /// Start offset relative to the segment in milliseconds.
    offset_start_ms: i64,
    /// End offset relative to the segment in milliseconds.
    offset_end_ms: i64,
}

/// One segment record from Tencent `ResultDetail`.
#[derive(Debug, Clone, PartialEq)]
struct TencentSegmentInput {
    /// Absolute segment start in milliseconds.
    start_ms: i64,
    /// Speaker id assigned by Tencent.
    speaker_id: usize,
    /// Word entries inside the segment.
    words: Vec<TencentWordInput>,
}

/// One word record from an Aliyun websocket sentence result.
#[derive(Debug, Clone, Deserialize, PartialEq)]
struct AliyunWordInput {
    /// Surface token returned by Aliyun when per-word timing is available.
    #[serde(default)]
    text: String,
    /// Start time in milliseconds from the websocket payload.
    #[serde(default, rename = "startTime")]
    start_time_ms: i64,
    /// End time in milliseconds from the websocket payload.
    #[serde(default, rename = "endTime")]
    end_time_ms: i64,
}

/// One sentence result emitted by the Aliyun websocket transport.
#[derive(Debug, Clone, Deserialize, PartialEq)]
struct AliyunSentenceInput {
    /// Per-word timing entries when Aliyun emits them.
    #[serde(default)]
    words: Vec<AliyunWordInput>,
    /// Sentence text fallback used when no per-word entries are present.
    #[serde(default)]
    sentence_text: String,
}

/// Clean one FunASR text segment by removing markup and punctuation that
/// should not become lexical tokens.
fn clean_funaudio_segment_text_inner(text: &str) -> String {
    let mut cleaned = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch == '<' && chars.peek() == Some(&'|') {
            chars.next();
            let mut found_end = false;
            while let Some(next) = chars.next() {
                if next == '|' && chars.peek() == Some(&'>') {
                    chars.next();
                    found_end = true;
                    break;
                }
            }
            if !found_end {
                cleaned.push('<');
                cleaned.push('|');
            }
            continue;
        }

        match ch {
            '「' | '」' => {}
            '。' | '，' | '！' | '？' => cleaned.push(' '),
            _ => cleaned.push(ch),
        }
    }

    cleaned.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Parse one FunASR timestamp entry into integer milliseconds.
fn parse_timestamp_pair_inner(value: &serde_json::Value) -> (Option<i64>, Option<i64>) {
    let Some(items) = value.as_array() else {
        return (None, None);
    };
    if items.len() < 2 {
        return (None, None);
    }

    let Some(start) = items[0].as_f64() else {
        return (None, None);
    };
    let Some(end) = items[1].as_f64() else {
        return (None, None);
    };

    (Some(start.round() as i64), Some(end.round() as i64))
}

/// Normalize one provider token for the requested language.
fn normalize_provider_token(token: &str, lang: &str) -> String {
    if lang == "yue" {
        cantonese_ops::normalize_cantonese(token)
    } else {
        token.to_string()
    }
}

/// Tokenize one cleaned FunASR segment according to the provider language.
fn tokenize_funaudio_segment(cleaned: &str, lang: &str) -> Vec<String> {
    if lang == "yue" {
        cantonese_ops::cantonese_char_tokens(cleaned)
    } else {
        cleaned
            .split_whitespace()
            .filter(|token| !token.is_empty())
            .map(ToOwned::to_owned)
            .collect()
    }
}

/// Tokenize one sentence-only provider fallback according to the provider
/// language. Cantonese keeps the Rust-owned normalization/tokenization path.
fn tokenize_sentence_fallback(text: &str, lang: &str) -> Vec<String> {
    if lang == "yue" {
        cantonese_ops::cantonese_char_tokens(text)
    } else {
        text.split_whitespace()
            .filter(|token| !token.is_empty())
            .map(ToOwned::to_owned)
            .collect()
    }
}

/// Project FunASR segments into the shared HK ASR projection.
fn project_funaudio_segments(segments: Vec<FunaudioSegmentInput>, lang: &str) -> HkAsrProjection {
    let mut monologues = Vec::new();
    let mut timed_words = Vec::new();

    for segment in segments {
        let cleaned = clean_funaudio_segment_text_inner(&segment.text);
        let tokens = tokenize_funaudio_segment(&cleaned, lang);
        let mut elements = Vec::new();

        for (index, token) in tokens.into_iter().enumerate() {
            let (start_ms, end_ms) = segment
                .timestamp
                .get(index)
                .map(parse_timestamp_pair_inner)
                .unwrap_or((None, None));

            elements.push(HkAsrElement {
                element_type: "text",
                ts: start_ms.map(|value| value as f64 / 1000.0),
                end_ts: end_ms.map(|value| value as f64 / 1000.0),
                value: token.clone(),
            });

            if let (Some(start_ms), Some(end_ms)) = (start_ms, end_ms)
                && end_ms > start_ms
            {
                timed_words.push(HkTimedWord {
                    word: token,
                    start_ms,
                    end_ms,
                });
            }
        }

        if !elements.is_empty() {
            monologues.push(HkAsrMonologue {
                speaker: 0,
                elements,
            });
        }
    }

    timed_words.sort_by_key(|item| item.start_ms);
    HkAsrProjection {
        monologues,
        timed_words,
    }
}

/// Project Tencent result-detail records into the shared HK ASR projection.
fn project_tencent_segments(segments: Vec<TencentSegmentInput>, lang: &str) -> HkAsrProjection {
    let mut monologues = Vec::new();
    let mut timed_words = Vec::new();

    for segment in segments {
        let mut elements = Vec::new();

        for word in segment.words {
            let raw = word.word.trim();
            if raw.is_empty() {
                continue;
            }

            let normalized = normalize_provider_token(raw, lang);
            let abs_start = segment.start_ms + word.offset_start_ms;
            let abs_end = segment.start_ms + word.offset_end_ms;

            elements.push(HkAsrElement {
                element_type: "text",
                ts: Some(abs_start as f64 / 1000.0),
                end_ts: Some(abs_end as f64 / 1000.0),
                value: normalized.clone(),
            });

            if abs_end > abs_start {
                timed_words.push(HkTimedWord {
                    word: normalized,
                    start_ms: abs_start,
                    end_ms: abs_end,
                });
            }
        }

        if !elements.is_empty() {
            monologues.push(HkAsrMonologue {
                speaker: segment.speaker_id,
                elements,
            });
        }
    }

    timed_words.sort_by_key(|item| item.start_ms);
    HkAsrProjection {
        monologues,
        timed_words,
    }
}

/// Project Aliyun sentence results into the shared HK ASR projection.
fn project_aliyun_sentences(sentences: Vec<AliyunSentenceInput>, lang: &str) -> HkAsrProjection {
    let mut monologues = Vec::new();
    let mut timed_words = Vec::new();

    for sentence in sentences {
        let mut elements = Vec::new();

        if !sentence.words.is_empty() {
            for word in sentence.words {
                let raw = word.text.trim();
                if raw.is_empty() {
                    continue;
                }

                let normalized = normalize_provider_token(raw, lang);
                elements.push(HkAsrElement {
                    element_type: "text",
                    ts: Some(word.start_time_ms as f64 / 1000.0),
                    end_ts: Some(word.end_time_ms as f64 / 1000.0),
                    value: normalized.clone(),
                });

                if word.end_time_ms > word.start_time_ms {
                    timed_words.push(HkTimedWord {
                        word: normalized,
                        start_ms: word.start_time_ms,
                        end_ms: word.end_time_ms,
                    });
                }
            }
        } else {
            for token in tokenize_sentence_fallback(&sentence.sentence_text, lang) {
                elements.push(HkAsrElement {
                    element_type: "text",
                    ts: None,
                    end_ts: None,
                    value: token,
                });
            }
        }

        if !elements.is_empty() {
            monologues.push(HkAsrMonologue {
                speaker: 0,
                elements,
            });
        }
    }

    timed_words.sort_by_key(|item| item.start_ms);
    HkAsrProjection {
        monologues,
        timed_words,
    }
}

/// Convert a Python list of Tencent result-detail objects into owned Rust data.
fn extract_tencent_segments(
    result_detail: &Bound<'_, PyAny>,
) -> PyResult<Vec<TencentSegmentInput>> {
    let list = result_detail.cast::<PyList>()?;
    let mut segments = Vec::with_capacity(list.len());

    for segment in list.iter() {
        let start_ms = segment
            .getattr("StartMs")
            .ok()
            .and_then(|value| value.extract::<i64>().ok())
            .unwrap_or(0);
        let speaker_id = segment
            .getattr("SpeakerId")
            .ok()
            .and_then(|value| value.extract::<usize>().ok())
            .unwrap_or(0);

        let mut words = Vec::new();
        if let Ok(raw_words) = segment.getattr("Words")
            && let Ok(raw_words) = raw_words.cast::<PyList>()
        {
            words.reserve(raw_words.len());
            for word in raw_words.iter() {
                let text = word
                    .getattr("Word")
                    .ok()
                    .and_then(|value| value.extract::<String>().ok())
                    .unwrap_or_default();
                let offset_start_ms = word
                    .getattr("OffsetStartMs")
                    .ok()
                    .and_then(|value| value.extract::<i64>().ok())
                    .unwrap_or(0);
                let offset_end_ms = word
                    .getattr("OffsetEndMs")
                    .ok()
                    .and_then(|value| value.extract::<i64>().ok())
                    .unwrap_or(offset_start_ms);
                words.push(TencentWordInput {
                    word: text,
                    offset_start_ms,
                    offset_end_ms,
                });
            }
        }

        segments.push(TencentSegmentInput {
            start_ms,
            speaker_id,
            words,
        });
    }

    Ok(segments)
}

/// Clean one FunASR segment text using the Rust-owned provider projection rule.
#[pyfunction]
pub(crate) fn clean_funaudio_segment_text(py: Python<'_>, text: &str) -> String {
    py.detach(move || clean_funaudio_segment_text_inner(text))
}

/// Project raw FunASR output into monologues and timed words.
///
/// Python passes the model output after only shallow parsing. Rust then owns
/// the segment cleanup, Cantonese tokenization, timestamp conversion, and
/// shared projection shape.
#[pyfunction]
pub(crate) fn funaudio_segments_to_asr(
    py: Python<'_>,
    segments: &Bound<'_, PyAny>,
    lang: &str,
) -> PyResult<String> {
    let value = py_to_json_value(segments)?;
    let lang = lang.to_string();

    py.detach(move || {
        let segments = match value {
            serde_json::Value::Array(_) => {
                serde_json::from_value::<Vec<FunaudioSegmentInput>>(value).map_err(|error| {
                    BatchalignBoundaryError::internal(error.to_string()).into_py_err()
                })?
            }
            serde_json::Value::Object(_) => {
                vec![
                    serde_json::from_value::<FunaudioSegmentInput>(value).map_err(|error| {
                        BatchalignBoundaryError::internal(error.to_string()).into_py_err()
                    })?,
                ]
            }
            _ => {
                return Err(pyo3::exceptions::PyTypeError::new_err(
                    "FunASR output must be a dict or list of dicts",
                ));
            }
        };

        serde_json::to_string(&project_funaudio_segments(segments, &lang))
            .map_err(|error| pyo3::exceptions::PyRuntimeError::new_err(error.to_string()))
    })
}

/// Project Tencent `ResultDetail` objects into monologues and timed words.
///
/// Python keeps only the transport/SDK responsibilities. Rust owns the timing
/// math, Cantonese normalization, and projection into the shared worker shape.
#[pyfunction]
pub(crate) fn tencent_result_detail_to_asr(
    py: Python<'_>,
    result_detail: &Bound<'_, PyAny>,
    lang: &str,
) -> PyResult<String> {
    let segments = extract_tencent_segments(result_detail)?;
    let lang = lang.to_string();

    py.detach(move || {
        serde_json::to_string(&project_tencent_segments(segments, &lang))
            .map_err(|error| pyo3::exceptions::PyRuntimeError::new_err(error.to_string()))
    })
}

/// Project Aliyun sentence results into monologues and timed words.
///
/// Python keeps only websocket transport, credential handling, and shallow
/// payload parsing. Rust owns the sentence fallback tokenization plus the
/// shared monologue/timed-word projection shape.
#[pyfunction]
pub(crate) fn aliyun_sentences_to_asr(
    py: Python<'_>,
    sentences: &Bound<'_, PyAny>,
    lang: &str,
) -> PyResult<String> {
    let value = py_to_json_value(sentences)?;
    let lang = lang.to_string();

    py.detach(move || {
        let sentences = match value {
            serde_json::Value::Array(_) => {
                serde_json::from_value::<Vec<AliyunSentenceInput>>(value).map_err(|error| {
                    BatchalignBoundaryError::internal(error.to_string()).into_py_err()
                })?
            }
            serde_json::Value::Object(_) => {
                vec![
                    serde_json::from_value::<AliyunSentenceInput>(value).map_err(|error| {
                        BatchalignBoundaryError::internal(error.to_string()).into_py_err()
                    })?,
                ]
            }
            _ => {
                return Err(pyo3::exceptions::PyTypeError::new_err(
                    "Aliyun sentences must be a dict or list of dicts",
                ));
            }
        };

        serde_json::to_string(&project_aliyun_sentences(sentences, &lang))
            .map_err(|error| pyo3::exceptions::PyRuntimeError::new_err(error.to_string()))
    })
}

// ---------------------------------------------------------------------------
// Cantonese text normalization
// ---------------------------------------------------------------------------

/// Normalize Cantonese text: simplified → HK traditional + domain replacements.
///
/// Uses embedded OpenCC rules for s2hk conversion, then applies a domain
/// replacement table for Cantonese-specific character corrections.
#[pyfunction]
pub(crate) fn normalize_cantonese(py: Python<'_>, text: &str) -> String {
    py.detach(|| cantonese_ops::normalize_cantonese(text))
}

/// Normalize Cantonese text and split into per-character tokens.
///
/// Strips CJK punctuation and whitespace after normalization.
/// Used by FunASR Cantonese to align per-character timestamps.
#[pyfunction]
pub(crate) fn cantonese_char_tokens(py: Python<'_>, text: &str) -> Vec<String> {
    py.detach(|| cantonese_ops::cantonese_char_tokens(text))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn funaudio_cleaning_matches_current_rules() {
        assert_eq!(clean_funaudio_segment_text_inner("<|zh|> hello"), "hello");
        assert_eq!(
            clean_funaudio_segment_text_inner("你好，世界！"),
            "你好 世界"
        );
        assert_eq!(clean_funaudio_segment_text_inner("「hello」"), "hello");
        assert_eq!(
            clean_funaudio_segment_text_inner("<|zh|> 「你好」，我係啊！"),
            "你好 我係啊"
        );
    }

    #[test]
    fn funaudio_projection_handles_cantonese_and_missing_timestamps() {
        let projection = project_funaudio_segments(
            vec![FunaudioSegmentInput {
                text: "<|zh|> 真系".to_string(),
                timestamp: vec![serde_json::json!([0, 100]), serde_json::json!([100, 200])],
            }],
            "yue",
        );

        assert_eq!(projection.monologues.len(), 1);
        let values: Vec<&str> = projection.monologues[0]
            .elements
            .iter()
            .map(|element| element.value.as_str())
            .collect();
        assert_eq!(values, vec!["真", "係"]);
        assert_eq!(projection.timed_words.len(), 2);
    }

    #[test]
    fn tencent_projection_normalizes_and_sorts_timed_words() {
        let projection = project_tencent_segments(
            vec![
                TencentSegmentInput {
                    start_ms: 1000,
                    speaker_id: 2,
                    words: vec![
                        TencentWordInput {
                            word: "系".to_string(),
                            offset_start_ms: 0,
                            offset_end_ms: 200,
                        },
                        TencentWordInput {
                            word: "你".to_string(),
                            offset_start_ms: 300,
                            offset_end_ms: 500,
                        },
                    ],
                },
                TencentSegmentInput {
                    start_ms: 500,
                    speaker_id: 1,
                    words: vec![TencentWordInput {
                        word: "呀".to_string(),
                        offset_start_ms: 0,
                        offset_end_ms: 100,
                    }],
                },
            ],
            "yue",
        );

        assert_eq!(projection.monologues.len(), 2);
        assert_eq!(projection.monologues[0].elements[0].value, "係");
        assert_eq!(projection.monologues[1].elements[0].value, "啊");
        assert_eq!(
            projection
                .timed_words
                .iter()
                .map(|item| (item.word.as_str(), item.start_ms, item.end_ms))
                .collect::<Vec<_>>(),
            vec![("啊", 500, 600), ("係", 1000, 1200), ("你", 1300, 1500)]
        );
    }

    #[test]
    fn aliyun_projection_handles_sentence_fallback_and_timed_words() {
        let projection = project_aliyun_sentences(
            vec![
                AliyunSentenceInput {
                    words: vec![AliyunWordInput {
                        text: "系".to_string(),
                        start_time_ms: 100,
                        end_time_ms: 250,
                    }],
                    sentence_text: "系".to_string(),
                },
                AliyunSentenceInput {
                    words: Vec::new(),
                    sentence_text: "真系呀，".to_string(),
                },
            ],
            "yue",
        );

        assert_eq!(projection.monologues.len(), 2);
        assert_eq!(projection.monologues[0].elements[0].value, "係");
        assert_eq!(
            projection.monologues[1]
                .elements
                .iter()
                .map(|element| element.value.as_str())
                .collect::<Vec<_>>(),
            vec!["真", "係", "啊"]
        );
        assert_eq!(
            projection
                .timed_words
                .iter()
                .map(|item| (item.word.as_str(), item.start_ms, item.end_ms))
                .collect::<Vec<_>>(),
            vec![("係", 100, 250)]
        );
    }
}
