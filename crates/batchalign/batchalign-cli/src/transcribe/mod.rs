//! Server-side transcribe orchestrator.
//!
//! Owns the full audio-to-CHAT lifecycle:
//! raw ASR inference → Rust normalization → post-processing → CHAT assembly
//! → optional utseg → optional morphosyntax.
//!
//! Split into submodules:
//! - [`types`] — ASR response types, backend selection, transcribe options
//! - [`infer`] — ASR and speaker inference dispatch to worker backends
//! - [`asr_output`] — ASR response conversion, participant IDs, CHAT helpers

mod asr_output;
mod infer;
pub mod types;

// Re-export the public API so callers don't need to know about the split.
pub(crate) use asr_output::*;
pub(crate) use infer::*;
pub use types::*;

use std::path::Path;

use crate::error::ServerError;
use crate::pipeline::PipelineServices;
use crate::pipeline::transcribe::run_transcribe_pipeline;
use crate::runner::util::ProgressSender;

// ---------------------------------------------------------------------------
// Orchestrator
// ---------------------------------------------------------------------------

/// Process a single audio file through the transcribe pipeline.
///
/// Returns the final serialized CHAT text.
///
/// # Pipeline stages
///
/// 1. **ASR inference** — invoke the selected ASR backend, get raw tokens
/// 2. **Post-processing** — compound merging, number expansion, retokenization
/// 3. **CHAT assembly** — build `ChatFile` AST from utterances
/// 4. **Utterance segmentation** (optional) — BERT-based re-segmentation
/// 5. **Morphosyntax** (optional) — POS/dependency tagging
pub(crate) async fn process_transcribe(
    audio_path: &Path,
    services: PipelineServices<'_>,
    opts: &TranscribeOptions,
    progress: Option<ProgressSender>,
    debug_dir: Option<&Path>,
) -> Result<String, ServerError> {
    run_transcribe_pipeline(audio_path, services, opts, progress, debug_dir).await
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::{DurationSeconds, LanguageCode3, LanguageSpec, WorkerLanguage};
    use talkbank_transform::asr_postprocess::{self, SpeakerIndex};
    use talkbank_transform::build_chat::{self, TranscriptDescription};
    use talkbank_transform::serialize::to_chat_string;

    #[test]
    fn asr_backend_mapping_distinguishes_live_v2_worker_modes() {
        assert_eq!(AsrBackend::from_engine_name("rev"), AsrBackend::RustRevAi);
        assert_eq!(
            AsrBackend::from_engine_name("tencent"),
            AsrBackend::Worker(AsrWorkerMode::HkTencentV2)
        );
        assert_eq!(
            AsrBackend::from_engine_name("aliyun"),
            AsrBackend::Worker(AsrWorkerMode::HkAliyunV2)
        );
        assert_eq!(
            AsrBackend::from_engine_name("funaudio"),
            AsrBackend::Worker(AsrWorkerMode::HkFunaudioV2)
        );
        assert_eq!(
            AsrBackend::from_engine_name("whisper_oai"),
            AsrBackend::Worker(AsrWorkerMode::LocalWhisperV2)
        );
    }

    /// `Auto` jobs reach the ASR worker with no resolved fallback. The
    /// downstream parser must surface a typed error if the worker
    /// returns no language either — no silent eng fallback.
    #[test]
    fn asr_auto_uses_auto_worker_language_with_no_fallback() {
        let (worker_lang, fallback_lang) =
            asr_worker_languages(&LanguageSpec::Auto).expect("Auto is a legal transcribe spec");
        assert_eq!(worker_lang, WorkerLanguage::Auto);
        assert!(
            fallback_lang.is_none(),
            "Auto must have no concrete fallback — Stanza header must be \
             driven by the ASR response, not silently substituted with eng",
        );
    }

    #[test]
    fn asr_resolved_language_preserves_worker_and_fallback_values() {
        let lang = LanguageSpec::Resolved(LanguageCode3::fra());
        let (worker_lang, fallback_lang) =
            asr_worker_languages(&lang).expect("Resolved is a legal transcribe spec");
        assert_eq!(worker_lang, WorkerLanguage::from(LanguageCode3::fra()));
        assert_eq!(fallback_lang, Some(LanguageCode3::fra()));
    }

    /// `PerFile` is reserved for morphotag/translate/coref; transcribe
    /// must reject it at the ASR-language helper level so a malformed
    /// submission can't silently degrade.
    #[test]
    fn asr_per_file_is_rejected() {
        let err = asr_worker_languages(&LanguageSpec::PerFile)
            .expect_err("transcribe must reject PerFile");
        let msg = err.to_string();
        assert!(msg.contains("PerFile"), "{msg}");
    }

    fn sample_transcribe_options(backend: AsrBackend) -> TranscribeOptions {
        TranscribeOptions {
            backend,
            diarize: false,
            speaker_backend: None,
            lang: LanguageCode3::fra().into(),
            num_speakers: 1,
            with_utseg: false,
            with_morphosyntax: false,
            override_media_cache: false,
            allow_stanza_fallback_utseg: false,
            write_wor: false,
            media_name: Some("sample".into()),
            rev_job_id: None,
        }
    }

    #[test]
    fn build_empty_chat_text_is_valid_chat() {
        let text =
            build_empty_chat_text(&sample_transcribe_options(AsrBackend::RustRevAi)).unwrap();

        // Legacy comment format removed; provenance is injected at pipeline
        // level (not in build_empty_chat_text). Verify the output is valid CHAT.
        assert!(text.contains("@Begin"));
        assert!(text.contains("@End"));
    }

    #[test]
    fn insert_transcribe_comment_inserts_header_before_utterances() {
        let desc = TranscriptDescription {
            langs: vec!["fra".into()],
            participants: vec![build_chat::ParticipantDesc {
                id: "PAR".into(),
                name: None,
                role: "Participant".to_string(),
                corpus: String::new(),
            }],
            media_name: Some("sample".into()),
            media_type: Some("audio".into()),
            utterances: vec![build_chat::UtteranceDesc {
                speaker: "PAR".into(),
                words: Some(vec![build_chat::WordDesc {
                    text: asr_postprocess::ChatWordText::try_from("bonjour").expect("legal word"),
                    start_ms: Some(0),
                    end_ms: Some(500),
                    kind: asr_postprocess::WordKind::Regular,
                }]),
                text: None,
                start_ms: None,
                end_ms: None,
                lang: None,
            }],
            write_wor: false,
        };
        let chat_file = build_chat::build_chat(&desc).unwrap();
        let text = to_chat_string(&chat_file);

        let text = insert_transcribe_comment(
            &text,
            &sample_transcribe_options(AsrBackend::Worker(AsrWorkerMode::LocalWhisperV2)),
        );

        let comment_pos = text
            .find("@Comment:\tBatchalign, ASR Engine whisper. Unchecked output of ASR model, DO NOT USE.")
            .expect("must contain updated comment format");
        let utterance_pos = text.find("\n*PAR:").unwrap();
        assert!(comment_pos < utterance_pos);
    }

    /// The transcribe @Comment must include "DO NOT USE" per a user's request.
    /// Users must not trust unchecked ASR output.
    #[test]
    fn transcribe_comment_includes_do_not_use() {
        let chat_text = "@UTF8\n@Begin\n@Languages:\teng\n\
            @Participants:\tPAR Participant\n\
            @ID:\teng|test|PAR|||||Participant|||\n\
            *PAR:\thello .\n@End\n";
        let opts = sample_transcribe_options(AsrBackend::Worker(AsrWorkerMode::LocalWhisperV2));
        let text = insert_transcribe_comment(chat_text, &opts);
        let comment = text
            .lines()
            .find(|l| l.contains("Unchecked output"))
            .expect("must have an 'Unchecked output' comment");
        assert!(
            comment.contains("DO NOT USE"),
            "@Comment must include 'DO NOT USE', got: {comment}"
        );
    }

    /// The transcribe @Comment must not contain a hardcoded semver version.
    /// Use commit hash or omit version entirely.
    #[test]
    fn transcribe_comment_no_hardcoded_version() {
        let chat_text = "@UTF8\n@Begin\n@Languages:\teng\n\
            @Participants:\tPAR Participant\n\
            @ID:\teng|test|PAR|||||Participant|||\n\
            *PAR:\thello .\n@End\n";
        let opts = sample_transcribe_options(AsrBackend::Worker(AsrWorkerMode::LocalWhisperV2));
        let text = insert_transcribe_comment(chat_text, &opts);
        for line in text.lines() {
            if line.starts_with("@Comment:") && line.contains("Batchalign") {
                assert!(
                    !line.contains("0.1.0"),
                    "@Comment must not contain '0.1.0', got: {line}"
                );
            }
        }
    }

    #[test]
    fn test_convert_asr_response_groups_by_speaker() {
        let response = AsrResponse {
            tokens: vec![
                AsrToken {
                    text: "hello".into(),
                    start_s: Some(DurationSeconds(0.0)),
                    end_s: Some(DurationSeconds(0.5)),
                    speaker: Some("0".into()),
                    confidence: None,
                },
                AsrToken {
                    text: "world".into(),
                    start_s: Some(DurationSeconds(0.5)),
                    end_s: Some(DurationSeconds(1.0)),
                    speaker: Some("0".into()),
                    confidence: None,
                },
                AsrToken {
                    text: "hi".into(),
                    start_s: Some(DurationSeconds(1.0)),
                    end_s: Some(DurationSeconds(1.5)),
                    speaker: Some("1".into()),
                    confidence: None,
                },
            ],
            lang: LanguageCode3::eng(),
            source_monologues: None,
        };

        let output = convert_asr_response(&response);
        assert_eq!(output.monologues.len(), 2);
        assert_eq!(output.monologues[0].speaker, SpeakerIndex(0));
        assert_eq!(output.monologues[0].elements.len(), 2);
        assert_eq!(output.monologues[1].speaker, SpeakerIndex(1));
        assert_eq!(output.monologues[1].elements.len(), 1);
    }

    #[test]
    fn test_convert_asr_response_handles_speaker_change_and_back() {
        let response = AsrResponse {
            tokens: vec![
                AsrToken {
                    text: "a".into(),
                    start_s: Some(DurationSeconds(0.0)),
                    end_s: Some(DurationSeconds(0.3)),
                    speaker: Some("0".into()),
                    confidence: None,
                },
                AsrToken {
                    text: "b".into(),
                    start_s: Some(DurationSeconds(0.3)),
                    end_s: Some(DurationSeconds(0.6)),
                    speaker: Some("1".into()),
                    confidence: None,
                },
                AsrToken {
                    text: "c".into(),
                    start_s: Some(DurationSeconds(0.6)),
                    end_s: Some(DurationSeconds(0.9)),
                    speaker: Some("0".into()),
                    confidence: None,
                },
            ],
            lang: LanguageCode3::eng(),
            source_monologues: None,
        };

        let output = convert_asr_response(&response);
        assert_eq!(output.monologues.len(), 3);
        assert_eq!(output.monologues[0].speaker, 0);
        assert_eq!(output.monologues[1].speaker, 1);
        assert_eq!(output.monologues[2].speaker, 0);
    }

    #[test]
    fn test_convert_asr_response_empty() {
        let response = AsrResponse {
            tokens: vec![],
            lang: LanguageCode3::eng(),
            source_monologues: None,
        };
        let output = convert_asr_response(&response);
        assert!(output.monologues.is_empty());
    }

    #[test]
    fn test_convert_asr_response_no_speaker_defaults_to_zero() {
        let response = AsrResponse {
            tokens: vec![AsrToken {
                text: "hello".into(),
                start_s: Some(DurationSeconds(0.0)),
                end_s: Some(DurationSeconds(0.5)),
                speaker: None,
                confidence: None,
            }],
            lang: LanguageCode3::eng(),
            source_monologues: None,
        };

        let output = convert_asr_response(&response);
        assert_eq!(output.monologues.len(), 1);
        assert_eq!(output.monologues[0].speaker, 0);
    }

    /// Regression test for an operator's bug report (2026-03-18): bare
    /// `batchalign3 transcribe` with no `--diarization` flag must still
    /// produce multi-speaker output when the ASR engine (Rev.AI) returns
    /// speaker-labeled monologues.
    ///
    /// In batchalign2, `process_generation()` unconditionally reads
    /// `utterance["speaker"]` from Rev.AI monologues. The `--diarize` flag
    /// only controls whether a *separate* Pyannote stage runs. BA3 must
    /// match this: speaker labels from the ASR engine are always used.
    #[test]
    fn test_convert_asr_response_always_uses_speaker_labels() {
        let response = AsrResponse {
            tokens: vec![
                AsrToken {
                    text: "hello".into(),
                    start_s: Some(DurationSeconds(0.0)),
                    end_s: Some(DurationSeconds(0.5)),
                    speaker: Some("0".into()),
                    confidence: None,
                },
                AsrToken {
                    text: "world".into(),
                    start_s: Some(DurationSeconds(0.5)),
                    end_s: Some(DurationSeconds(1.0)),
                    speaker: Some("1".into()),
                    confidence: None,
                },
            ],
            lang: LanguageCode3::eng(),
            source_monologues: None,
        };

        // Speaker labels must be respected regardless of any diarization flag.
        // Previously this test asserted the opposite (1 monologue, speaker 0),
        // which enshrined the bug.
        let output = convert_asr_response(&response);
        assert_eq!(
            output.monologues.len(),
            2,
            "each speaker change must start a new monologue"
        );
        assert_eq!(output.monologues[0].speaker, 0);
        assert_eq!(output.monologues[0].elements.len(), 1);
        assert_eq!(output.monologues[1].speaker, 1);
        assert_eq!(output.monologues[1].elements.len(), 1);
    }

    #[test]
    fn test_parse_speaker_label_accepts_suffix_format() {
        assert_eq!(parse_speaker_label("1"), Some(1));
        assert_eq!(parse_speaker_label("SPEAKER_2"), Some(2));
        assert_eq!(parse_speaker_label("not-a-speaker"), None);
    }

    #[test]
    fn test_generate_participant_ids() {
        let utterances = vec![
            asr_postprocess::Utterance {
                speaker: SpeakerIndex(0),
                words: vec![],
                lang: None,
            },
            asr_postprocess::Utterance {
                speaker: SpeakerIndex(1),
                words: vec![],
                lang: None,
            },
        ];
        let ids = generate_participant_ids(&utterances, 2);
        assert_eq!(ids, vec!["PAR0", "PAR1"]);
    }

    #[test]
    fn test_generate_participant_ids_many_speakers() {
        let utterances = vec![asr_postprocess::Utterance {
            speaker: SpeakerIndex(9),
            words: vec![],
            lang: None,
        }];
        let ids = generate_participant_ids(&utterances, 10);
        assert_eq!(ids.len(), 10);
        assert_eq!(ids[0], "PAR0");
        assert_eq!(ids[8], "PAR8");
        assert_eq!(ids[9], "PAR9");
    }

    #[test]
    fn test_generate_standard_participant_ids_uses_chat_defaults_then_sp() {
        let ids = generate_standard_participant_ids(5);
        assert_eq!(ids, vec!["PAR0", "PAR1", "PAR2", "PAR3", "PAR4"]);
    }

    // -----------------------------------------------------------------------
    // Canned-response integration tests
    //
    // Exercise the full conversion chain with realistic ASR payloads:
    //   AsrResponse → convert_asr_response() → process_raw_asr()
    //   → generate_participant_ids() → transcript_from_asr_utterances()
    //   → build_chat() → to_chat_string()
    //
    // These catch bugs that unit tests on individual stages miss — the same
    // class of bugs that echo-worker integration tests failed to expose.
    // -----------------------------------------------------------------------

    /// Build a realistic canned Rev.AI-style response: 2 speakers, ~20 tokens
    /// each, with timing and speaker labels. Simulates a short interview.
    fn canned_revai_two_speaker_response() -> AsrResponse {
        AsrResponse {
            tokens: vec![
                // Speaker 0 — first turn
                AsrToken {
                    text: "so".into(),
                    start_s: Some(DurationSeconds(0.24)),
                    end_s: Some(DurationSeconds(0.42)),
                    speaker: Some("0".into()),
                    confidence: Some(0.99),
                },
                AsrToken {
                    text: "tell".into(),
                    start_s: Some(DurationSeconds(0.42)),
                    end_s: Some(DurationSeconds(0.60)),
                    speaker: Some("0".into()),
                    confidence: Some(0.98),
                },
                AsrToken {
                    text: "me".into(),
                    start_s: Some(DurationSeconds(0.60)),
                    end_s: Some(DurationSeconds(0.72)),
                    speaker: Some("0".into()),
                    confidence: Some(0.99),
                },
                AsrToken {
                    text: "about".into(),
                    start_s: Some(DurationSeconds(0.72)),
                    end_s: Some(DurationSeconds(0.96)),
                    speaker: Some("0".into()),
                    confidence: Some(0.97),
                },
                AsrToken {
                    text: "your".into(),
                    start_s: Some(DurationSeconds(0.96)),
                    end_s: Some(DurationSeconds(1.14)),
                    speaker: Some("0".into()),
                    confidence: Some(0.98),
                },
                AsrToken {
                    text: "experience".into(),
                    start_s: Some(DurationSeconds(1.14)),
                    end_s: Some(DurationSeconds(1.68)),
                    speaker: Some("0".into()),
                    confidence: Some(0.96),
                },
                AsrToken {
                    text: "with".into(),
                    start_s: Some(DurationSeconds(1.68)),
                    end_s: Some(DurationSeconds(1.86)),
                    speaker: Some("0".into()),
                    confidence: Some(0.98),
                },
                AsrToken {
                    text: "the".into(),
                    start_s: Some(DurationSeconds(1.86)),
                    end_s: Some(DurationSeconds(1.98)),
                    speaker: Some("0".into()),
                    confidence: Some(0.99),
                },
                AsrToken {
                    text: "program.".into(),
                    start_s: Some(DurationSeconds(1.98)),
                    end_s: Some(DurationSeconds(2.52)),
                    speaker: Some("0".into()),
                    confidence: Some(0.95),
                },
                // Speaker 1 — response
                AsrToken {
                    text: "well".into(),
                    start_s: Some(DurationSeconds(3.00)),
                    end_s: Some(DurationSeconds(3.24)),
                    speaker: Some("1".into()),
                    confidence: Some(0.97),
                },
                AsrToken {
                    text: "I".into(),
                    start_s: Some(DurationSeconds(3.24)),
                    end_s: Some(DurationSeconds(3.36)),
                    speaker: Some("1".into()),
                    confidence: Some(0.99),
                },
                AsrToken {
                    text: "started".into(),
                    start_s: Some(DurationSeconds(3.36)),
                    end_s: Some(DurationSeconds(3.72)),
                    speaker: Some("1".into()),
                    confidence: Some(0.98),
                },
                AsrToken {
                    text: "about".into(),
                    start_s: Some(DurationSeconds(3.72)),
                    end_s: Some(DurationSeconds(3.96)),
                    speaker: Some("1".into()),
                    confidence: Some(0.97),
                },
                AsrToken {
                    text: "3".into(),
                    start_s: Some(DurationSeconds(3.96)),
                    end_s: Some(DurationSeconds(4.14)),
                    speaker: Some("1".into()),
                    confidence: Some(0.96),
                },
                AsrToken {
                    text: "years".into(),
                    start_s: Some(DurationSeconds(4.14)),
                    end_s: Some(DurationSeconds(4.38)),
                    speaker: Some("1".into()),
                    confidence: Some(0.98),
                },
                AsrToken {
                    text: "ago.".into(),
                    start_s: Some(DurationSeconds(4.38)),
                    end_s: Some(DurationSeconds(4.68)),
                    speaker: Some("1".into()),
                    confidence: Some(0.95),
                },
                AsrToken {
                    text: "it".into(),
                    start_s: Some(DurationSeconds(4.80)),
                    end_s: Some(DurationSeconds(4.92)),
                    speaker: Some("1".into()),
                    confidence: Some(0.99),
                },
                AsrToken {
                    text: "was".into(),
                    start_s: Some(DurationSeconds(4.92)),
                    end_s: Some(DurationSeconds(5.10)),
                    speaker: Some("1".into()),
                    confidence: Some(0.98),
                },
                AsrToken {
                    text: "really".into(),
                    start_s: Some(DurationSeconds(5.10)),
                    end_s: Some(DurationSeconds(5.40)),
                    speaker: Some("1".into()),
                    confidence: Some(0.97),
                },
                AsrToken {
                    text: "helpful".into(),
                    start_s: Some(DurationSeconds(5.40)),
                    end_s: Some(DurationSeconds(5.82)),
                    speaker: Some("1".into()),
                    confidence: Some(0.96),
                },
                // Speaker 0 — follow-up
                AsrToken {
                    text: "that".into(),
                    start_s: Some(DurationSeconds(6.00)),
                    end_s: Some(DurationSeconds(6.18)),
                    speaker: Some("0".into()),
                    confidence: Some(0.98),
                },
                AsrToken {
                    text: "sounds".into(),
                    start_s: Some(DurationSeconds(6.18)),
                    end_s: Some(DurationSeconds(6.48)),
                    speaker: Some("0".into()),
                    confidence: Some(0.97),
                },
                AsrToken {
                    text: "great".into(),
                    start_s: Some(DurationSeconds(6.48)),
                    end_s: Some(DurationSeconds(6.78)),
                    speaker: Some("0".into()),
                    confidence: Some(0.99),
                },
                // Speaker 1 — closing
                AsrToken {
                    text: "yeah".into(),
                    start_s: Some(DurationSeconds(7.00)),
                    end_s: Some(DurationSeconds(7.24)),
                    speaker: Some("1".into()),
                    confidence: Some(0.98),
                },
                AsrToken {
                    text: "I".into(),
                    start_s: Some(DurationSeconds(7.24)),
                    end_s: Some(DurationSeconds(7.36)),
                    speaker: Some("1".into()),
                    confidence: Some(0.99),
                },
                AsrToken {
                    text: "would".into(),
                    start_s: Some(DurationSeconds(7.36)),
                    end_s: Some(DurationSeconds(7.56)),
                    speaker: Some("1".into()),
                    confidence: Some(0.97),
                },
                AsrToken {
                    text: "recommend".into(),
                    start_s: Some(DurationSeconds(7.56)),
                    end_s: Some(DurationSeconds(8.04)),
                    speaker: Some("1".into()),
                    confidence: Some(0.96),
                },
                AsrToken {
                    text: "it".into(),
                    start_s: Some(DurationSeconds(8.04)),
                    end_s: Some(DurationSeconds(8.16)),
                    speaker: Some("1".into()),
                    confidence: Some(0.99),
                },
            ],
            lang: LanguageCode3::eng(),
            source_monologues: None,
        }
    }

    /// Build a canned Whisper-style response: no speaker labels, single
    /// contiguous stream of tokens with timing.
    fn canned_whisper_no_speaker_response() -> AsrResponse {
        AsrResponse {
            tokens: vec![
                AsrToken {
                    text: "the".into(),
                    start_s: Some(DurationSeconds(0.0)),
                    end_s: Some(DurationSeconds(0.18)),
                    speaker: None,
                    confidence: Some(0.95),
                },
                AsrToken {
                    text: "quick".into(),
                    start_s: Some(DurationSeconds(0.18)),
                    end_s: Some(DurationSeconds(0.42)),
                    speaker: None,
                    confidence: Some(0.93),
                },
                AsrToken {
                    text: "brown".into(),
                    start_s: Some(DurationSeconds(0.42)),
                    end_s: Some(DurationSeconds(0.66)),
                    speaker: None,
                    confidence: Some(0.94),
                },
                AsrToken {
                    text: "fox".into(),
                    start_s: Some(DurationSeconds(0.66)),
                    end_s: Some(DurationSeconds(0.90)),
                    speaker: None,
                    confidence: Some(0.96),
                },
                AsrToken {
                    text: "jumps".into(),
                    start_s: Some(DurationSeconds(0.90)),
                    end_s: Some(DurationSeconds(1.20)),
                    speaker: None,
                    confidence: Some(0.95),
                },
                AsrToken {
                    text: "over".into(),
                    start_s: Some(DurationSeconds(1.20)),
                    end_s: Some(DurationSeconds(1.44)),
                    speaker: None,
                    confidence: Some(0.97),
                },
                AsrToken {
                    text: "the".into(),
                    start_s: Some(DurationSeconds(1.44)),
                    end_s: Some(DurationSeconds(1.56)),
                    speaker: None,
                    confidence: Some(0.98),
                },
                AsrToken {
                    text: "lazy".into(),
                    start_s: Some(DurationSeconds(1.56)),
                    end_s: Some(DurationSeconds(1.86)),
                    speaker: None,
                    confidence: Some(0.94),
                },
                AsrToken {
                    text: "dog.".into(),
                    start_s: Some(DurationSeconds(1.86)),
                    end_s: Some(DurationSeconds(2.22)),
                    speaker: None,
                    confidence: Some(0.96),
                },
                AsrToken {
                    text: "then".into(),
                    start_s: Some(DurationSeconds(2.40)),
                    end_s: Some(DurationSeconds(2.58)),
                    speaker: None,
                    confidence: Some(0.93),
                },
                AsrToken {
                    text: "it".into(),
                    start_s: Some(DurationSeconds(2.58)),
                    end_s: Some(DurationSeconds(2.70)),
                    speaker: None,
                    confidence: Some(0.97),
                },
                AsrToken {
                    text: "sat".into(),
                    start_s: Some(DurationSeconds(2.70)),
                    end_s: Some(DurationSeconds(2.94)),
                    speaker: None,
                    confidence: Some(0.95),
                },
                AsrToken {
                    text: "down".into(),
                    start_s: Some(DurationSeconds(2.94)),
                    end_s: Some(DurationSeconds(3.18)),
                    speaker: None,
                    confidence: Some(0.96),
                },
            ],
            lang: LanguageCode3::eng(),
            source_monologues: None,
        }
    }

    /// Run the full canned-response conversion chain and return CHAT text.
    ///
    /// Mirrors the pipeline stages in `pipeline/transcribe.rs`:
    /// `convert_asr_response` → `process_raw_asr` → `generate_participant_ids`
    /// → `transcript_from_asr_utterances` → `build_chat` → `to_chat_string`.
    fn run_canned_response_to_chat(
        response: &AsrResponse,
        num_speakers: usize,
        media_name: Option<&str>,
    ) -> String {
        let asr_output = convert_asr_response(response);
        let utterances = asr_postprocess::process_raw_asr(&asr_output, &response.lang);
        let participant_ids = generate_participant_ids(&utterances, num_speakers);
        let desc = build_chat::transcript_from_asr_utterances(
            &utterances,
            &participant_ids,
            &[response.lang.to_string()],
            media_name,
            false,
        )
        .expect("test: transcript_from_asr_utterances should succeed");
        let chat_file = build_chat::build_chat(&desc).expect("build_chat must succeed");
        to_chat_string(&chat_file)
    }

    /// Full pipeline test: canned Rev.AI 2-speaker response produces valid
    /// multi-speaker CHAT with correct headers and timing.
    #[test]
    fn canned_revai_response_produces_multi_speaker_chat() {
        let response = canned_revai_two_speaker_response();
        let chat = run_canned_response_to_chat(&response, 2, Some("interview.mp3"));

        // Must have 2 @Participants entries (PAR0 + PAR1, generic numbered codes)
        let participants_line = chat
            .lines()
            .find(|l| l.starts_with("@Participants:"))
            .expect("@Participants header missing");
        assert!(
            participants_line.contains("PAR0") && participants_line.contains("PAR1"),
            "expected PAR0 and PAR1 in @Participants, got: {participants_line}"
        );

        // Must have 2 @ID lines
        let id_count = chat.lines().filter(|l| l.starts_with("@ID:")).count();
        assert_eq!(id_count, 2, "expected 2 @ID lines, got {id_count}");

        // Must have utterances from both speakers
        let par0_count = chat.lines().filter(|l| l.starts_with("*PAR0:")).count();
        let par1_count = chat.lines().filter(|l| l.starts_with("*PAR1:")).count();
        assert!(
            par0_count >= 1,
            "expected at least 1 *PAR0 utterance, got {par0_count}"
        );
        assert!(
            par1_count >= 1,
            "expected at least 1 *PAR1 utterance, got {par1_count}"
        );

        // Timing bullets must be present (the \x15 delimiters)
        assert!(
            chat.contains('\x15'),
            "timing bullets missing from output CHAT"
        );

        // @Media header
        assert!(
            chat.contains("@Media:\tinterview, audio"),
            "expected @Media header with stripped extension"
        );

        // Must reparse cleanly
        let parser = talkbank_transform::parse::TreeSitterParser::new().unwrap();
        let (_parsed, errors) = talkbank_transform::parse::parse_lenient(&parser, &chat);
        assert!(
            errors.is_empty(),
            "generated CHAT must reparse cleanly: {errors:?}"
        );
    }

    /// Full pipeline test: canned Whisper response (no speaker labels) produces
    /// single-speaker CHAT with exactly 1 participant.
    #[test]
    fn canned_whisper_response_produces_single_speaker_chat() {
        let response = canned_whisper_no_speaker_response();
        let chat = run_canned_response_to_chat(&response, 1, Some("recording.wav"));

        // Must have exactly 1 participant
        let id_count = chat.lines().filter(|l| l.starts_with("@ID:")).count();
        assert_eq!(
            id_count, 1,
            "expected 1 @ID line for single-speaker, got {id_count}"
        );

        // All utterances must be from PAR0 (speaker 0)
        let non_par0_utts: Vec<&str> = chat
            .lines()
            .filter(|l| l.starts_with('*') && !l.starts_with("*PAR0:"))
            .collect();
        assert!(
            non_par0_utts.is_empty(),
            "all utterances should be *PAR0 for single-speaker, found: {non_par0_utts:?}"
        );

        // Must have at least 1 utterance
        let par0_count = chat.lines().filter(|l| l.starts_with("*PAR0:")).count();
        assert!(
            par0_count >= 1,
            "expected at least 1 *PAR0 utterance, got {par0_count}"
        );

        // Timing bullets must be present
        assert!(
            chat.contains('\x15'),
            "timing bullets missing from single-speaker output"
        );

        // Must reparse cleanly
        let parser = talkbank_transform::parse::TreeSitterParser::new().unwrap();
        let (_parsed, errors) = talkbank_transform::parse::parse_lenient(&parser, &chat);
        assert!(
            errors.is_empty(),
            "generated CHAT must reparse cleanly: {errors:?}"
        );
    }

    /// Regression test: Rev.AI response with speaker labels must produce
    /// multi-speaker output regardless of the diarization flag.
    ///
    /// This is the end-to-end version of the
    /// `test_convert_asr_response_always_uses_speaker_labels` unit test.
    /// It exercises the full chain through CHAT serialization to catch
    /// any stage that might collapse speakers.
    #[test]
    fn canned_revai_speaker_labels_produce_multi_speaker_regardless_of_diarize_flag() {
        let response = canned_revai_two_speaker_response();

        // The pipeline does not consult opts.diarize during
        // convert_asr_response → process_raw_asr → build_chat. Verify this
        // by running the same canned data through the conversion chain.
        let chat = run_canned_response_to_chat(&response, 2, Some("test.mp3"));

        // Count distinct speaker codes in utterance lines
        let speaker_codes: std::collections::BTreeSet<&str> = chat
            .lines()
            .filter(|l| l.starts_with('*'))
            .filter_map(|l| l.split(':').next())
            .map(|code| code.trim_start_matches('*'))
            .collect();
        assert!(
            speaker_codes.len() >= 2,
            "Rev.AI response with speaker labels must produce at least 2 distinct speakers \
             in the output CHAT, but only found: {speaker_codes:?}. \
             This was an operator's bug report: speaker labels from ASR must always be used."
        );
    }

    /// Whisper response (no speaker labels) should produce single-speaker
    /// output even when num_speakers > 1 — without dedicated diarization,
    /// Whisper tokens all default to speaker 0.
    #[test]
    fn canned_whisper_no_labels_stays_single_speaker_even_with_high_num_speakers() {
        let response = canned_whisper_no_speaker_response();
        // Pass num_speakers=3 — but since there are no labels, all tokens
        // map to speaker 0 and only PAR appears in the output.
        let chat = run_canned_response_to_chat(&response, 3, None);

        let speaker_codes: std::collections::BTreeSet<&str> = chat
            .lines()
            .filter(|l| l.starts_with('*'))
            .filter_map(|l| l.split(':').next())
            .map(|code| code.trim_start_matches('*'))
            .collect();
        assert_eq!(
            speaker_codes.len(),
            1,
            "Whisper response without speaker labels should produce exactly 1 speaker, got: {speaker_codes:?}"
        );
        assert!(
            speaker_codes.contains("PAR0"),
            "sole speaker should be PAR0"
        );
    }

    /// Verify that number expansion works end-to-end in the canned Rev.AI
    /// response (the token "3" should become "three" in the output).
    #[test]
    fn canned_revai_response_expands_numbers() {
        let response = canned_revai_two_speaker_response();
        let chat = run_canned_response_to_chat(&response, 2, None);

        assert!(
            chat.contains("three"),
            "number '3' in canned response should be expanded to 'three' in CHAT output"
        );
        // The raw digit should not appear as a standalone word
        let has_raw_digit = chat
            .lines()
            .any(|l| l.starts_with('*') && l.split_whitespace().any(|w| w == "3"));
        assert!(
            !has_raw_digit,
            "raw digit '3' should not appear as a standalone word in utterance lines"
        );
    }

    /// Verify that embedded sentence-ending punctuation in canned responses
    /// (e.g. "program." or "ago.") splits correctly into utterance boundaries.
    #[test]
    fn canned_revai_response_splits_on_embedded_periods() {
        let response = canned_revai_two_speaker_response();
        let asr_output = convert_asr_response(&response);
        let utterances = asr_postprocess::process_raw_asr(&asr_output, &response.lang);

        // "program." and "ago." should create utterance boundaries, so we
        // expect more than 2 utterances from the 4-turn conversation.
        assert!(
            utterances.len() >= 3,
            "expected at least 3 utterances from embedded-period splitting, got {}",
            utterances.len()
        );

        // Every utterance must end with a terminator
        for (i, utt) in utterances.iter().enumerate() {
            let last = utt.words.last().expect("utterance should have words");
            assert!(
                matches!(last.text.as_str(), "." | "?" | "!"),
                "utterance {i} should end with a terminator, got: {:?}",
                last.text
            );
        }
    }
}
