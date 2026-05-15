//! Rust-owned Rev.AI ASR inference for server-mode transcription.
//!
//! This path exists so `transcribe` and `benchmark` do not need to route the
//! Rev.AI provider through the Python worker at all. The only engines that
//! should stay in Python are the ones that genuinely require Python-hosted
//! model libraries.

use std::path::Path;

use crate::revai::{RevAiClient, SubmitOptions, Transcript, TranscriptResult};
use talkbank_transform::asr_postprocess::{
    AsrElement, AsrElementKind, AsrMonologue, AsrOutput, AsrRawText, AsrTimestampSecs, SpeakerIndex,
};
use tracing::{info, warn};

use crate::api::{DurationSeconds, LanguageCode3, LanguageSpec, NumSpeakers};
use crate::error::ServerError;
use crate::transcribe::{AsrResponse, AsrToken};

use super::{RevAiLanguageHint, load_revai_api_key};

/// Run Rev.AI ASR directly from Rust and map the transcript into the shared
/// `AsrResponse` domain used by the transcribe pipeline.
///
/// When `lang` is `LanguageSpec::Auto`, passes `"auto"` to Rev.AI so it
/// auto-detects the spoken language, and reads the detected language from
/// the completed job to populate `AsrResponse.lang`.
pub(crate) async fn infer_revai_asr(
    audio_path: &Path,
    lang: &LanguageSpec,
    num_speakers: NumSpeakers,
    rev_job_id: Option<&str>,
) -> Result<AsrResponse, ServerError> {
    let api_key =
        load_revai_api_key().map_err(|error| ServerError::Validation(error.to_string()))?;
    let audio_path = audio_path.to_path_buf();
    let lang = lang.clone();
    let rev_job_id = rev_job_id.map(str::to_string);

    tokio::task::spawn_blocking(move || {
        // When auto-detecting, run Rev.AI Language Identification first.
        // This is a separate API (~5-30s) that identifies the spoken language
        // from audio features — far more accurate than text-based trigram
        // detection, especially for code-switched bilingual audio.
        let effective_lang = if matches!(lang, LanguageSpec::Auto) {
            let client = RevAiClient::new(api_key.as_str());
            match client.identify_language_blocking(&audio_path, 30) {
                Ok(langid_result) => {
                    let detected = &langid_result.top_language;
                    info!(
                        detected_language = %detected,
                        confidence = langid_result.language_confidences.first()
                            .map(|c| c.confidence).unwrap_or(0.0),
                        "Rev.AI Language ID detected language"
                    );
                    match revai_code_to_iso639_3(detected) {
                        Some(code) => LanguageSpec::Resolved(code),
                        None => {
                            tracing::warn!(
                                detected = %detected,
                                "Rev.AI Language ID returned unmapped code; using auto fallback"
                            );
                            lang.clone()
                        }
                    }
                }
                Err(error) => {
                    tracing::warn!(
                        %error,
                        "Rev.AI Language ID failed; falling back to transcription-level auto"
                    );
                    lang.clone()
                }
            }
        } else {
            lang.clone()
        };

        let result = fetch_revai_transcript(
            &api_key,
            &audio_path,
            &effective_lang,
            num_speakers,
            rev_job_id.as_deref(),
        )
        .map_err(|error| ServerError::Validation(error.to_string()))?;

        // Resolve the language. No silent fallback to English — if Language
        // ID didn't return anything usable and the user didn't supply
        // `--lang`, the file's `@Languages:` would be a lie. Surface the
        // failure instead so the operator re-runs with `--lang <iso3>`.
        let resolved_lang: LanguageCode3 = match &effective_lang {
            LanguageSpec::Resolved(code) => code.clone(),
            // Auto: user asked Rev.AI to detect. PerFile: transcribe path
            // shouldn't see this — submission validation rejects it. Either
            // way the only honest source here is Rev.AI's `detected_language`.
            LanguageSpec::Auto | LanguageSpec::PerFile => result
                .detected_language
                .as_deref()
                .filter(|d| !d.is_empty() && *d != "auto")
                .and_then(revai_code_to_iso639_3)
                .ok_or_else(|| {
                    ServerError::Validation(
                        "Rev.AI did not return a usable detected language for `--lang auto`. \
                         Re-run with an explicit `--lang <iso3>` so the @Languages header is \
                         honest."
                            .into(),
                    )
                })?,
        };

        Ok(transcript_to_asr_response(
            &result.transcript,
            &resolved_lang,
        ))
    })
    .await
    .map_err(|error| ServerError::Validation(format!("Rev.AI task join error: {error}")))?
}

/// Fetch a Rev.AI transcript either by polling an existing submitted job or by
/// submitting one local audio file and waiting for completion.
///
/// When `lang` is `LanguageSpec::Auto`, sends `language: "auto"` to Rev.AI.
/// In auto mode, `speakers_count` and `skip_postprocessing` are not sent
/// because we can't know the language characteristics ahead of time.
///
/// For concrete languages:
/// - `speakers_count` is sent for English and Spanish (Rev.AI performs its own
///   speaker diarization for these languages).
/// - `skip_postprocessing` is `Some(true)` for English and Spanish — Rev.AI's
///   post-processing applies Inverse Text Normalization (ITN) which converts
///   spoken form (what the speaker said) into written form
///   (`"eighty percent"` → `"80%"`, `"seventeen year old"` → `"17-year-old"`).
///   CHAT records spoken form, so we skip ITN wherever the flag is available.
///   For other languages the flag is a no-op per Rev.AI docs, so we omit it.
///
/// The typical production path is preflight-submitted jobs (see
/// `revai/preflight.rs`); callers pass `rev_job_id: Some(id)` and this
/// function just polls. The non-preflight branch below is for other flows
/// (UTR ASR, tests) and uses the same policy helpers from `preflight`.
pub(super) fn fetch_revai_transcript(
    api_key: &super::RevAiApiKey,
    audio_path: &Path,
    lang: &LanguageSpec,
    num_speakers: NumSpeakers,
    rev_job_id: Option<&str>,
) -> crate::revai::Result<TranscriptResult> {
    let client = RevAiClient::new(api_key.as_str());
    if let Some(job_id) = rev_job_id {
        return client.poll_and_download(job_id, 5, 30);
    }

    let lang_hint_str = match lang.as_resolved() {
        Some(code) => RevAiLanguageHint::from(code).as_str().to_string(),
        None => "auto".to_string(),
    };
    let is_auto = lang.is_auto();

    // In auto mode, we can't assume language-specific settings.
    let speakers_count = if is_auto {
        None
    } else {
        match lang_hint_str.as_str() {
            "en" | "es" => Some(num_speakers.0),
            _ => None,
        }
    };
    let skip_postprocessing = if is_auto {
        None
    } else {
        super::preflight::skip_postprocessing_hint(lang_hint_str.as_str())
    };

    let metadata = audio_path
        .file_stem()
        .map(|stem| format!("batchalign3_{}", stem.to_string_lossy()));
    let options = SubmitOptions {
        language: lang_hint_str,
        speakers_count,
        skip_postprocessing,
        metadata,
    };
    client.transcribe_blocking(audio_path, &options, 30)
}

/// Map a Rev.AI ISO 639-1 language code back to an ISO 639-3 code.
///
/// This is the reverse of [`try_revai_language_hint`]. When Rev.AI auto-detects
/// a language, it returns the ISO 639-1 code (e.g. `"es"`). We need to convert
/// that back to ISO 639-3 (e.g. `"spa"`) for CHAT headers and downstream NLP.
///
/// Returns `None` for unrecognized codes rather than panicking — the caller
/// should fall through to whatlang trigram detection.
fn revai_code_to_iso639_3(revai_code: &str) -> Option<LanguageCode3> {
    let iso3 = match revai_code {
        "en" => "eng",
        "es" => "spa",
        "fr" => "fra",
        "de" => "deu",
        "it" => "ita",
        "pt" => "por",
        "nl" => "nld",
        "ja" => "jpn",
        "ko" => "kor",
        "ru" => "rus",
        "ar" => "ara",
        "tr" => "tur",
        "cmn" => "zho",
        "pl" => "pol",
        "cs" => "ces",
        "ro" => "ron",
        "hu" => "hun",
        "bg" => "bul",
        "hr" => "hrv",
        "sr" => "srp",
        "sk" => "slk",
        "sl" => "slv",
        "uk" => "ukr",
        "lt" => "lit",
        "lv" => "lav",
        "et" => "est",
        "fi" => "fin",
        "da" => "dan",
        "no" => "nor",
        "sv" => "swe",
        "is" => "isl",
        "el" => "ell",
        "ca" => "cat",
        "gl" => "glg",
        "eu" => "eus",
        "cy" => "cym",
        "sq" => "sqi",
        "be" => "bel",
        "bs" => "bos",
        "mk" => "mkd",
        "mt" => "mlt",
        "hi" => "hin",
        "ur" => "urd",
        "bn" => "ben",
        "ta" => "tam",
        "te" => "tel",
        "kn" => "kan",
        "ml" => "mal",
        "mr" => "mar",
        "pa" => "pan",
        "ne" => "nep",
        "si" => "sin",
        "th" => "tha",
        "vi" => "vie",
        "id" => "ind",
        "tl" => "tgl",
        "my" => "mya",
        "km" => "khm",
        "lo" => "lao",
        "su" => "sun",
        "ka" => "kat",
        "hy" => "hye",
        "az" => "aze",
        "kk" => "kaz",
        "uz" => "uzb",
        "tg" => "tgk",
        "fa" => "fas",
        "he" => "heb",
        "yi" => "yid",
        "af" => "afr",
        "sw" => "swa",
        "ht" => "hat",
        "gu" => "guj",
        "mg" => "mlg",
        other => {
            tracing::warn!(
                revai_code = other,
                "Unknown Rev.AI language code; falling back to trigram detection"
            );
            return None;
        }
    };
    LanguageCode3::try_new(iso3).ok()
}

fn transcript_to_asr_response(transcript: &Transcript, lang: &LanguageCode3) -> AsrResponse {
    let mut tokens = Vec::new();

    for monologue in &transcript.monologues {
        let speaker = monologue.speaker.to_string();
        for element in &monologue.elements {
            if element.element_type != "text" {
                continue;
            }

            let text = element.value.trim();
            if text.is_empty() {
                continue;
            }

            tokens.push(AsrToken {
                text: text.to_string(),
                start_s: element.ts.map(DurationSeconds),
                end_s: element.end_ts.map(DurationSeconds),
                speaker: Some(speaker.clone()),
                confidence: element.confidence,
            });
        }
    }

    AsrResponse {
        tokens,
        lang: lang.clone(),
        source_monologues: Some(transcript_to_asr_output(transcript).monologues),
    }
}

fn transcript_to_asr_output(transcript: &Transcript) -> AsrOutput {
    AsrOutput {
        monologues: transcript
            .monologues
            .iter()
            .map(|monologue| AsrMonologue {
                speaker: SpeakerIndex(monologue.speaker as usize),
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
                            ts: AsrTimestampSecs(element.ts.unwrap_or_else(|| {
                                warn!(
                                    token = text,
                                    "Rev.AI element missing start timestamp, defaulting to 0.0s"
                                );
                                0.0
                            })),
                            end_ts: AsrTimestampSecs(element.end_ts.unwrap_or_else(|| {
                                warn!(
                                    token = text,
                                    "Rev.AI element missing end timestamp, defaulting to 0.0s"
                                );
                                0.0
                            })),
                            kind: if element.element_type == "text" {
                                AsrElementKind::Text
                            } else {
                                AsrElementKind::Punctuation
                            },
                        })
                    })
                    .collect(),
            })
            .collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::LanguageCode3;
    use crate::revai::Transcript;

    #[test]
    fn transcript_projection_keeps_flat_text_tokens_for_legacy_paths() {
        let transcript: Transcript = serde_json::from_str(
            r#"{
            "monologues": [{
                "speaker": 3,
                "elements": [
                    {"type": "text", "value": "hello", "ts": 0.5, "end_ts": 0.9, "confidence": 0.75},
                    {"type": "punct", "value": ","},
                    {"type": "text", "value": "world", "ts": 1.0, "end_ts": 1.4}
                ]
            }]
        }"#,
        )
        .unwrap();

        let response = transcript_to_asr_response(&transcript, &LanguageCode3::eng());
        assert_eq!(response.lang, "eng");
        assert_eq!(response.tokens.len(), 2);
        assert_eq!(response.tokens[0].text, "hello");
        assert_eq!(response.tokens[0].speaker.as_deref(), Some("3"));
        assert_eq!(response.tokens[0].confidence, Some(0.75));
        assert_eq!(response.tokens[1].text, "world");
    }

    #[test]
    fn transcript_projection_preserves_punctuation_and_monologue_boundaries() {
        let transcript: Transcript = serde_json::from_str(
            r#"{
            "monologues": [
                {
                    "speaker": 3,
                    "elements": [
                        {"type": "text", "value": "hello", "ts": 0.5, "end_ts": 0.9, "confidence": 0.75},
                        {"type": "punct", "value": ","},
                        {"type": "text", "value": "world", "ts": 1.0, "end_ts": 1.4}
                    ]
                },
                {
                    "speaker": 3,
                    "elements": [
                        {"type": "text", "value": "again", "ts": 2.0, "end_ts": 2.4},
                        {"type": "punct", "value": "?"}
                    ]
                }
            ]
        }"#,
        )
        .unwrap();

        let response = transcript_to_asr_response(&transcript, &LanguageCode3::eng());
        let monologues = response
            .source_monologues
            .expect("Rev projection should preserve provider-shaped monologues");

        assert_eq!(monologues.len(), 2);
        assert_eq!(monologues[0].speaker, SpeakerIndex(3));
        assert_eq!(monologues[0].elements.len(), 3);
        assert_eq!(monologues[0].elements[1].value, ",");
        assert_eq!(monologues[0].elements[1].kind, AsrElementKind::Punctuation);
        assert_eq!(monologues[1].speaker, SpeakerIndex(3));
        assert_eq!(monologues[1].elements.len(), 2);
        assert_eq!(monologues[1].elements[1].value, "?");
        assert_eq!(monologues[1].elements[1].kind, AsrElementKind::Punctuation);
    }

    #[test]
    fn revai_code_roundtrip_major_languages() {
        assert_eq!(revai_code_to_iso639_3("es").as_deref(), Some("spa"));
        assert_eq!(revai_code_to_iso639_3("en").as_deref(), Some("eng"));
        assert_eq!(revai_code_to_iso639_3("fr").as_deref(), Some("fra"));
        assert_eq!(revai_code_to_iso639_3("cmn").as_deref(), Some("zho"));
        assert_eq!(revai_code_to_iso639_3("ja").as_deref(), Some("jpn"));
    }

    #[test]
    fn revai_code_rejects_auto_sentinel() {
        // "auto" is not a language — must return None so caller falls
        // through to whatlang detection.
        assert_eq!(revai_code_to_iso639_3("auto"), None);
    }

    #[test]
    fn revai_code_rejects_unknown() {
        assert_eq!(revai_code_to_iso639_3("xx"), None);
        assert_eq!(revai_code_to_iso639_3(""), None);
    }
}
