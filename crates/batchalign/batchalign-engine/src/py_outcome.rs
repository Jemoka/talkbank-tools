//! `PyOutcome` — Python-facing wrapper around `BAValue`.
//!
//! `BAValue` is a plain Rust enum (no `#[pyclass]`) because several of its
//! variants carry types (`Chat<Validated>` wrapping `talkbank_model::ChatFile`,
//! `MetricsArtifact` with a `serde_json::Value` map) that cannot trivially be
//! exposed as pyclasses. The Python surface only needs to know: which source
//! produced the outcome, whether it failed, and how to write the result. This
//! wrapper exposes exactly that.

use std::path::PathBuf;
use std::sync::Mutex;

use batchalign_core::{BAError, BAValue, SourceId};
use pyo3::exceptions::{PyRuntimeError, PyValueError};
use pyo3::prelude::*;
use talkbank_model::alignment::helpers::{walk_words_mut, WordItemMut};
use talkbank_model::model::DependentTier;
use talkbank_model::Line;

/// Opaque PyO3 handle around a single pipeline outcome.
///
/// The wrapped `BAValue` is held in a `Mutex<Option<...>>` so `write()` can
/// take it by value (it serializes the CHAT or metrics to disk). After
/// `write()`, the cell is empty — calling `write()` twice raises.
#[pyclass(name = "Outcome", module = "batchalign._core")]
pub struct PyOutcome {
    inner: Mutex<Option<BAValue>>,
    source_id: SourceId,
    kind_str: String,
    failed: bool,
}

impl PyOutcome {
    pub fn from_value(value: BAValue) -> Self {
        let source_id = value.source_id();
        let kind_str = value.kind().to_string();
        let failed = value.is_failed();
        PyOutcome {
            inner: Mutex::new(Some(value)),
            source_id,
            kind_str,
            failed,
        }
    }
}

#[pymethods]
impl PyOutcome {
    /// Identifier of the source that produced this outcome.
    #[getter]
    fn source_id(&self) -> String {
        self.source_id.as_str().to_owned()
    }

    /// `"media" | "chat" | "paired" | "metrics" | "failed"`.
    #[getter]
    fn kind(&self) -> String {
        self.kind_str.clone()
    }

    /// `True` if this outcome corresponds to a per-value failure.
    #[getter]
    fn is_failed(&self) -> bool {
        self.failed
    }

    /// Human-readable error string when `is_failed()` is true; otherwise `""`.
    #[getter]
    fn error(&self) -> String {
        let guard = match self.inner.lock() {
            Ok(g) => g,
            Err(_) => return String::new(),
        };
        match guard.as_ref() {
            Some(BAValue::Failed { error, .. }) => format!("{error}"),
            _ => String::new(),
        }
    }

    /// Write the outcome to disk. Consumes the outcome (calling twice raises).
    #[pyo3(signature = (path, *, strip_word_timing=false))]
    fn write(&self, path: PathBuf, strip_word_timing: bool) -> PyResult<()> {
        let mut guard = self
            .inner
            .lock()
            .map_err(|_| PyRuntimeError::new_err("outcome lock poisoned"))?;
        let mut value = guard
            .take()
            .ok_or_else(|| PyValueError::new_err("outcome already written"))?;
        if strip_word_timing {
            strip_word_timing_from_value(&mut value);
        }
        match value.write(&path) {
            Ok(()) => Ok(()),
            Err(BAError::Io(e)) => Err(PyRuntimeError::new_err(format!(
                "write {}: {e}",
                path.display()
            ))),
            Err(other) => Err(PyRuntimeError::new_err(format!(
                "write {}: {other}",
                path.display()
            ))),
        }
    }
}

fn strip_word_timing_from_value(value: &mut BAValue) {
    match value {
        BAValue::Chat(chat) => strip_word_timing_from_chat(chat),
        BAValue::Paired(paired) => {
            let (main, _) = paired.as_mut_parts();
            strip_word_timing_from_chat(main);
        }
        BAValue::Cons { head, tail } => {
            strip_word_timing_from_value(head);
            strip_word_timing_from_value(tail);
        }
        BAValue::Failed {
            partial: Some(partial),
            ..
        } => strip_word_timing_from_value(partial),
        _ => {}
    }
}

fn strip_word_timing_from_chat(chat: &mut batchalign_core::Chat) {
    for line in chat.ast_mut().lines.0.iter_mut() {
        let Line::Utterance(utterance) = line else {
            continue;
        };
        utterance
            .dependent_tiers
            .retain(|tier| !matches!(tier, DependentTier::Wor(_)));
        walk_words_mut(
            &mut utterance.main.content.content.0,
            None,
            &mut |item| match item {
                WordItemMut::Word(word) => word.inline_bullet = None,
                WordItemMut::ReplacedWord(replaced) => {
                    replaced.word.inline_bullet = None;
                    for word in &mut replaced.replacement.words {
                        word.inline_bullet = None;
                    }
                }
                WordItemMut::Separator(_) => {}
            },
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use batchalign_core::{Chat, SourceId};
    use talkbank_model::alignment::helpers::{walk_words, WordItem};
    use talkbank_transform::asr_postprocess;
    use talkbank_transform::build_chat::{
        build_chat, ParticipantDesc, TranscriptDescription, UtteranceDesc, WordDesc,
    };

    fn word(text: &str, start_ms: Option<u64>, end_ms: Option<u64>) -> WordDesc {
        WordDesc {
            text: asr_postprocess::ChatWordText::try_from(text).expect("valid CHAT word"),
            start_ms,
            end_ms,
            kind: asr_postprocess::WordKind::Regular,
        }
    }

    #[test]
    fn strip_word_timing_keeps_utterance_bullet() {
        let desc = TranscriptDescription {
            langs: vec!["eng".to_string()],
            participants: vec![ParticipantDesc {
                id: "PAR".to_string(),
                name: None,
                role: "Participant".to_string(),
                corpus: "batchalign".to_string(),
            }],
            media_name: None,
            media_type: None,
            utterances: vec![UtteranceDesc {
                speaker: "PAR".to_string(),
                words: Some(vec![
                    word("hello", Some(0), Some(500)),
                    word("world", Some(500), Some(1000)),
                    word(".", None, None),
                ]),
                text: None,
                start_ms: None,
                end_ms: None,
                lang: None,
            }],
            write_wor: true,
        };

        let collector = talkbank_model::ErrorCollector::new();
        let chat_file = build_chat(&desc).expect("build chat");
        let validated = chat_file.validate_into(&collector, None);
        let mut chat = Chat::from_validated_ast(
            validated,
            SourceId::try_new("strip-test").expect("source id"),
        );

        strip_word_timing_from_chat(&mut chat);

        let utterance = chat.ast().utterances().next().expect("utterance");
        assert!(utterance.main.content.bullet.is_some());
        assert!(utterance.wor_tier().is_none());

        let mut any_word_bullet = false;
        walk_words(&utterance.main.content.content.0, None, &mut |item| {
            if let WordItem::Word(word) = item {
                any_word_bullet |= word.inline_bullet.is_some();
            }
        });
        assert!(!any_word_bullet);
    }
}
