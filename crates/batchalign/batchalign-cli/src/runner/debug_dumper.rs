//! Pipeline debug artifact writer.
//!
//! When constructed with a directory path, writes structured CHAT/JSON
//! artifacts at each pipeline stage for offline replay and test fixture
//! generation. When constructed without a path, all methods are zero-cost
//! no-ops. This is the single testability seam for stage decomposition.

use std::path::{Path, PathBuf};

use crate::chat_ops::fa::utr::{AsrTimingToken, UtrResult};
use serde::{Deserialize, Serialize};
use tracing::{debug, info};

use crate::api::DurationMs;
use crate::types::traces::FaGroupTrace;

/// Pipeline debug artifact writer.
///
/// When constructed with a directory path, writes structured CHAT/JSON
/// artifacts at each pipeline stage for offline replay and test fixture
/// generation. When constructed without a path, all methods are zero-cost
/// no-ops.
pub(crate) struct DebugDumper {
    dir: Option<PathBuf>,
}

/// Per-group FA dump data for offline replay.
#[allow(dead_code)]
#[derive(Serialize, Deserialize)]
pub(crate) struct FaGroupDumpData {
    /// Audio window start in milliseconds.
    pub audio_start_ms: DurationMs,
    /// Audio window end in milliseconds.
    pub audio_end_ms: DurationMs,
    /// Words in this group.
    pub words: Vec<String>,
    /// Per-word timing pairs from FA inference.
    pub timings: Vec<Option<TimingPair>>,
}

/// A start/end timing pair in milliseconds.
#[allow(dead_code)]
#[derive(Serialize, Deserialize)]
pub(crate) struct TimingPair {
    /// Word start time in milliseconds.
    pub start_ms: i64,
    /// Word end time in milliseconds.
    pub end_ms: i64,
}

impl DebugDumper {
    /// Create a new dumper. If `dir` is `None`, all methods are no-ops.
    pub(crate) fn new(dir: Option<&Path>) -> Self {
        Self {
            dir: dir.map(PathBuf::from),
        }
    }

    /// Create a disabled dumper (all methods are no-ops).
    #[cfg(test)]
    pub(crate) fn disabled() -> Self {
        Self { dir: None }
    }

    /// Whether dumping is enabled.
    pub(crate) fn is_enabled(&self) -> bool {
        self.dir.is_some()
    }

    /// Ensure the dump directory exists, returning it. Logs and returns `None`
    /// on failure.
    fn ensure_dir(&self) -> Option<&Path> {
        let dir = self.dir.as_deref()?;
        if let Err(e) = std::fs::create_dir_all(dir) {
            debug!(%e, "failed to create debug dir");
            return None;
        }
        Some(dir)
    }

    /// Extract the file stem from a filename for use in dump file names.
    fn stem(filename: &str) -> &str {
        Path::new(filename)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("unknown")
    }

    /// Dump CHAT text before UTR injection.
    pub(crate) fn dump_utr_input(&self, filename: &str, chat_text: &str) {
        let Some(dir) = self.ensure_dir() else {
            return;
        };
        let stem = Self::stem(filename);
        let path = dir.join(format!("{stem}_utr_input.cha"));
        if let Err(e) = std::fs::write(&path, chat_text) {
            debug!(%e, "failed to write UTR debug CHAT input");
        }
    }

    /// Dump ASR timing tokens used for UTR injection.
    pub(crate) fn dump_utr_tokens(&self, filename: &str, tokens: &[AsrTimingToken]) {
        let Some(dir) = self.ensure_dir() else {
            return;
        };
        let stem = Self::stem(filename);
        let path = dir.join(format!("{stem}_utr_tokens.json"));
        match serde_json::to_string_pretty(tokens) {
            Ok(json) => {
                if let Err(e) = std::fs::write(&path, json) {
                    debug!(%e, "failed to write UTR debug tokens");
                }
            }
            Err(e) => debug!(%e, "failed to serialize UTR tokens"),
        }

        info!(
            %filename,
            tokens = %path.display(),
            "UTR debug data dumped"
        );
    }

    /// Dump CHAT text and UtrResult after UTR injection.
    pub(crate) fn dump_utr_output(&self, filename: &str, chat_text: &str, utr_result: &UtrResult) {
        let Some(dir) = self.ensure_dir() else {
            return;
        };
        let stem = Self::stem(filename);

        let chat_path = dir.join(format!("{stem}_utr_output.cha"));
        if let Err(e) = std::fs::write(&chat_path, chat_text) {
            debug!(%e, "failed to write UTR debug CHAT output");
        }

        let result_path = dir.join(format!("{stem}_utr_result.json"));
        match serde_json::to_string_pretty(utr_result) {
            Ok(json) => {
                if let Err(e) = std::fs::write(&result_path, json) {
                    debug!(%e, "failed to write UTR result JSON");
                }
            }
            Err(e) => debug!(%e, "failed to serialize UTR result"),
        }
    }

    /// Dump FA grouping plan and pre-FA CHAT text.
    #[allow(dead_code)]
    pub(crate) fn dump_fa_grouping(
        &self,
        filename: &str,
        groups: &[FaGroupTrace],
        chat_text: &str,
    ) {
        let Some(dir) = self.ensure_dir() else {
            return;
        };
        let stem = Self::stem(filename);

        let chat_path = dir.join(format!("{stem}_fa_input.cha"));
        if let Err(e) = std::fs::write(&chat_path, chat_text) {
            debug!(%e, "failed to write FA debug CHAT input");
        }

        let grouping_path = dir.join(format!("{stem}_fa_grouping.json"));
        match serde_json::to_string_pretty(groups) {
            Ok(json) => {
                if let Err(e) = std::fs::write(&grouping_path, json) {
                    debug!(%e, "failed to write FA grouping JSON");
                }
            }
            Err(e) => debug!(%e, "failed to serialize FA grouping"),
        }

        info!(
            %filename,
            num_groups = groups.len(),
            "FA grouping debug data dumped"
        );
    }

    /// Dump per-group FA result (words + timings).
    #[allow(dead_code)]
    pub(crate) fn dump_fa_group_result(
        &self,
        filename: &str,
        group_idx: usize,
        data: &FaGroupDumpData,
    ) {
        let Some(dir) = self.ensure_dir() else {
            return;
        };
        let stem = Self::stem(filename);
        let path = dir.join(format!("{stem}_fa_group_{group_idx}.json"));
        match serde_json::to_string_pretty(data) {
            Ok(json) => {
                if let Err(e) = std::fs::write(&path, json) {
                    debug!(%e, group = group_idx, "failed to write FA group result");
                }
            }
            Err(e) => debug!(%e, group = group_idx, "failed to serialize FA group result"),
        }
    }

    /// Dump final aligned CHAT text after FA.
    pub(crate) fn dump_fa_output(&self, filename: &str, chat_text: &str) {
        let Some(dir) = self.ensure_dir() else {
            return;
        };
        let stem = Self::stem(filename);
        let path = dir.join(format!("{stem}_fa_output.cha"));
        if let Err(e) = std::fs::write(&path, chat_text) {
            debug!(%e, "failed to write FA debug CHAT output");
        }
    }

    // -------------------------------------------------------------------
    // Transcribe pipeline debug artifacts
    // -------------------------------------------------------------------

    /// Dump raw ASR response JSON after ASR inference.
    pub(crate) fn dump_asr_response(&self, filename: &str, response: &impl serde::Serialize) {
        let Some(dir) = self.ensure_dir() else {
            return;
        };
        let stem = Self::stem(filename);
        let path = dir.join(format!("{stem}_asr_response.json"));
        match serde_json::to_string_pretty(response) {
            Ok(json) => {
                if let Err(e) = std::fs::write(&path, json) {
                    debug!(%e, "failed to write ASR response JSON");
                }
            }
            Err(e) => debug!(%e, "failed to serialize ASR response"),
        }
        info!(%filename, response = %path.display(), "ASR response debug data dumped");
    }

    /// Dump CHAT text after CHAT assembly (post-ASR, pre-utseg).
    pub(crate) fn dump_post_asr_chat(&self, filename: &str, chat_text: &str) {
        let Some(dir) = self.ensure_dir() else {
            return;
        };
        let stem = Self::stem(filename);
        let path = dir.join(format!("{stem}_post_asr.cha"));
        if let Err(e) = std::fs::write(&path, chat_text) {
            debug!(%e, "failed to write post-ASR CHAT");
        }
    }

    /// Dump CHAT text before utterance segmentation.
    pub(crate) fn dump_pre_utseg_chat(&self, filename: &str, chat_text: &str) {
        let Some(dir) = self.ensure_dir() else {
            return;
        };
        let stem = Self::stem(filename);
        let path = dir.join(format!("{stem}_pre_utseg.cha"));
        if let Err(e) = std::fs::write(&path, chat_text) {
            debug!(%e, "failed to write pre-utseg CHAT");
        }
    }

    /// Dump CHAT text after utterance segmentation.
    pub(crate) fn dump_post_utseg_chat(&self, filename: &str, chat_text: &str) {
        let Some(dir) = self.ensure_dir() else {
            return;
        };
        let stem = Self::stem(filename);
        let path = dir.join(format!("{stem}_post_utseg.cha"));
        if let Err(e) = std::fs::write(&path, chat_text) {
            debug!(%e, "failed to write post-utseg CHAT");
        }
    }

    /// Dump CHAT text before morphosyntax.
    pub(crate) fn dump_pre_morphosyntax_chat(&self, filename: &str, chat_text: &str) {
        let Some(dir) = self.ensure_dir() else {
            return;
        };
        let stem = Self::stem(filename);
        let path = dir.join(format!("{stem}_pre_morphosyntax.cha"));
        if let Err(e) = std::fs::write(&path, chat_text) {
            debug!(%e, "failed to write pre-morphosyntax CHAT");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn disabled_dumper_is_noop() {
        let dumper = DebugDumper::disabled();
        assert!(!dumper.is_enabled());
        // These should all return immediately without error
        let chat = "@UTF8\n@Begin\n@End";
        dumper.dump_utr_input("test.cha", chat);
        dumper.dump_utr_tokens("test.cha", &[]);
        dumper.dump_utr_output(
            "test.cha",
            chat,
            &UtrResult {
                injected: 0,
                skipped: 0,
                unmatched: 0,
                decisions: Vec::new(),
            },
        );
        dumper.dump_fa_output("test.cha", chat);
        dumper.dump_asr_response("test.wav", &serde_json::json!({"tokens": []}));
        dumper.dump_post_asr_chat("test.wav", chat);
        dumper.dump_pre_utseg_chat("test.wav", chat);
        dumper.dump_post_utseg_chat("test.wav", chat);
        dumper.dump_pre_morphosyntax_chat("test.wav", chat);
        dumper.dump_fa_group_result(
            "test.cha",
            0,
            &FaGroupDumpData {
                audio_start_ms: DurationMs(0),
                audio_end_ms: DurationMs(1000),
                words: vec!["hello".into()],
                timings: vec![Some(TimingPair {
                    start_ms: 0,
                    end_ms: 500,
                })],
            },
        );
    }

    #[test]
    fn enabled_dumper_writes_expected_files() {
        let dir = tempfile::tempdir().expect("tempdir");
        let dumper = DebugDumper::new(Some(dir.path()));

        assert!(dumper.is_enabled());

        let chat = "@UTF8\n@Begin\n*CHI:\thello .\n@End";
        let tokens = vec![AsrTimingToken {
            text: "hello".into(),
            start_ms: 100,
            end_ms: 500,
        }];
        let utr_result = UtrResult {
            injected: 1,
            skipped: 0,
            unmatched: 0,
            decisions: Vec::new(),
        };

        dumper.dump_utr_input("sample.cha", chat);
        dumper.dump_utr_tokens("sample.cha", &tokens);
        dumper.dump_utr_output("sample.cha", chat, &utr_result);
        dumper.dump_fa_output("sample.cha", chat);
        dumper.dump_asr_response(
            "sample.wav",
            &serde_json::json!({"tokens": [{"text": "hello"}]}),
        );
        dumper.dump_post_asr_chat("sample.wav", chat);
        dumper.dump_pre_utseg_chat("sample.wav", chat);
        dumper.dump_post_utseg_chat("sample.wav", chat);
        dumper.dump_pre_morphosyntax_chat("sample.wav", chat);
        dumper.dump_fa_group_result(
            "sample.cha",
            0,
            &FaGroupDumpData {
                audio_start_ms: DurationMs(0),
                audio_end_ms: DurationMs(1000),
                words: vec!["hello".into()],
                timings: vec![Some(TimingPair {
                    start_ms: 100,
                    end_ms: 500,
                })],
            },
        );

        // Verify files exist
        assert!(dir.path().join("sample_utr_input.cha").exists());
        assert!(dir.path().join("sample_utr_tokens.json").exists());
        assert!(dir.path().join("sample_utr_output.cha").exists());
        assert!(dir.path().join("sample_utr_result.json").exists());
        assert!(dir.path().join("sample_fa_output.cha").exists());
        assert!(dir.path().join("sample_fa_group_0.json").exists());
        assert!(dir.path().join("sample_asr_response.json").exists());
        assert!(dir.path().join("sample_post_asr.cha").exists());
        assert!(dir.path().join("sample_pre_utseg.cha").exists());
        assert!(dir.path().join("sample_post_utseg.cha").exists());
        assert!(dir.path().join("sample_pre_morphosyntax.cha").exists());

        // Verify tokens roundtrip
        let tokens_json = std::fs::read_to_string(dir.path().join("sample_utr_tokens.json"))
            .expect("read tokens");
        let parsed: Vec<AsrTimingToken> = serde_json::from_str(&tokens_json).expect("parse tokens");
        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed[0].text, "hello");
        assert_eq!(parsed[0].start_ms, 100);

        // Verify FA group roundtrip
        let group_json =
            std::fs::read_to_string(dir.path().join("sample_fa_group_0.json")).expect("read group");
        let parsed: FaGroupDumpData = serde_json::from_str(&group_json).expect("parse group");
        assert_eq!(parsed.audio_start_ms, DurationMs(0));
        assert_eq!(parsed.audio_end_ms, DurationMs(1000));
        assert_eq!(parsed.words, vec!["hello"]);
        assert!(parsed.timings[0].is_some());
    }
}
