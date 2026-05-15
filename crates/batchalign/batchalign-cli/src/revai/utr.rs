//! Rust-owned Rev.AI timed-word projection for utterance timing recovery.
//!
//! UTR only needs monotonically ordered timed words. Keeping this projection in
//! Rust means the FA/UTR path can reuse the shared Rev.AI client without
//! widening the Python worker boundary with another provider-specific fallback.

use std::path::Path;

use crate::chat_ops::fa::utr::AsrTimingToken;
use crate::revai::extract_timed_words;

use crate::api::{LanguageCode3, LanguageSpec, NumSpeakers};
use crate::error::ServerError;

use super::{asr::fetch_revai_transcript, load_revai_api_key};

/// Download Rev.AI timed words for UTR through the Rust-owned control-plane
/// transport.
///
/// When `rev_job_id` is present, this function polls the already-submitted
/// Rev.AI job instead of uploading the audio again. That lets the UTR path
/// reuse the same preflight submission mechanism as other Rust-owned Rev flows.
pub(crate) async fn infer_revai_utr(
    audio_path: &Path,
    lang: &LanguageCode3,
    rev_job_id: Option<&str>,
) -> Result<Vec<AsrTimingToken>, ServerError> {
    let api_key =
        load_revai_api_key().map_err(|error| ServerError::Validation(error.to_string()))?;
    let audio_path = audio_path.to_path_buf();
    // UTR always uses a concrete language — wrap in LanguageSpec::Resolved
    // for the shared fetch_revai_transcript signature.
    let lang_spec = LanguageSpec::Resolved(lang.clone());
    let rev_job_id = rev_job_id.map(str::to_string);

    tokio::task::spawn_blocking(move || {
        let result = fetch_revai_transcript(
            &api_key,
            &audio_path,
            &lang_spec,
            NumSpeakers(1),
            rev_job_id.as_deref(),
        )
        .map_err(|error| ServerError::Validation(error.to_string()))?;
        Ok(transcript_to_utr_tokens(&result.transcript))
    })
    .await
    .map_err(|error| ServerError::Validation(format!("Rev.AI task join error: {error}")))?
}

/// Project the shared Rev.AI transcript model into the simplified timed-token
/// shape consumed by `batchalign-chat-ops` UTR injection.
fn transcript_to_utr_tokens(transcript: &crate::revai::Transcript) -> Vec<AsrTimingToken> {
    extract_timed_words(transcript)
        .into_iter()
        .map(|word| AsrTimingToken {
            text: word.word,
            start_ms: word.start_ms,
            end_ms: word.end_ms,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::transcript_to_utr_tokens;

    #[test]
    fn transcript_projection_discards_blank_rev_tokens() {
        let transcript: crate::revai::Transcript = serde_json::from_str(
            r#"{
                "monologues": [{
                    "speaker": 0,
                    "elements": [
                        {"type": "text", "value": "hello", "ts": 0.1, "end_ts": 0.4},
                        {"type": "text", "value": "   ", "ts": 0.5, "end_ts": 0.8},
                        {"type": "text", "value": "world", "ts": 0.9, "end_ts": 1.2}
                    ]
                }]
            }"#,
        )
        .unwrap();

        let tokens = transcript_to_utr_tokens(&transcript);
        assert_eq!(tokens.len(), 2);
        assert_eq!(tokens[0].text, "hello");
        assert_eq!(tokens[0].start_ms, 100);
        assert_eq!(tokens[1].text, "world");
        assert_eq!(tokens[1].end_ms, 1200);
    }
}
