//! Parse and validate raw Stanza `doc.to_dict()` output into typed UD structures.
//!
//! After the CHAT divorce, Python workers return Stanza's native `to_dict()`
//! output (a `Vec<Vec<serde_json::Value>>` — one sentence of word dicts per
//! utterance). This module parses that raw output into typed UD structures,
//! applying the same validation that previously lived in Python.

use super::{UdResponse, UdSentence, UdWord, validate_and_clean};

/// Parse raw Stanza `doc.to_dict()` output into a [`UdResponse`].
///
/// Stanza's `to_dict()` returns a list of sentences, each a list of word dicts.
/// This function deserializes each word dict into a [`UdWord`], applying
/// validation and cleanup.
///
/// # Errors
///
/// Returns `Err` if a word dict cannot be deserialized into [`UdWord`].
pub fn parse_raw_stanza_output(
    raw_sentences: &[serde_json::Value],
) -> Result<UdResponse, StanzaParseError> {
    let mut sentences = Vec::with_capacity(raw_sentences.len());

    for (sent_idx, sent_value) in raw_sentences.iter().enumerate() {
        let word_dicts = sent_value.as_array().ok_or(StanzaParseError::NotAnArray {
            sentence_idx: sent_idx,
        })?;

        let mut words = Vec::with_capacity(word_dicts.len());
        for (word_idx, raw_word) in word_dicts.iter().enumerate() {
            let mut word: UdWord = serde_json::from_value(normalize_word_dict(raw_word.clone()))
                .map_err(|e| StanzaParseError::WordParse {
                    sentence_idx: sent_idx,
                    word_idx,
                    source: e,
                })?;

            validate_and_clean(&mut word);
            words.push(word);
        }

        sentences.push(UdSentence { words });
    }

    Ok(UdResponse { sentences })
}

/// Diagnostic for a single problematic word in raw Stanza output.
#[derive(Debug, Clone)]
pub struct StanzaWordDiagnostic {
    /// Sentence index in the raw output.
    pub sentence_idx: usize,
    /// Word index within the sentence.
    pub word_idx: usize,
    /// The field that has a problem.
    pub field: String,
    /// Human-readable description of the issue.
    pub issue: String,
}

impl std::fmt::Display for StanzaWordDiagnostic {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "sentence {} word {}: field '{}' — {}",
            self.sentence_idx, self.word_idx, self.field, self.issue
        )
    }
}

/// Diagnose problems in raw Stanza output without attempting full deserialization.
///
/// Scans each word dict for known issues: missing required fields, null values,
/// `<pad>` sentinels, and type mismatches. Returns a list of diagnostics — one
/// per problem found. An empty list means the output looks structurally valid.
pub fn diagnose_parse_failure(raw_sentences: &[serde_json::Value]) -> Vec<StanzaWordDiagnostic> {
    let required_string_fields = ["text", "lemma", "upos", "deprel"];
    let mut diagnostics = Vec::new();

    for (si, sent_value) in raw_sentences.iter().enumerate() {
        let Some(words) = sent_value.as_array() else {
            diagnostics.push(StanzaWordDiagnostic {
                sentence_idx: si,
                word_idx: 0,
                field: "sentence".into(),
                issue: "not a JSON array".into(),
            });
            continue;
        };

        for (wi, word) in words.iter().enumerate() {
            let Some(obj) = word.as_object() else {
                diagnostics.push(StanzaWordDiagnostic {
                    sentence_idx: si,
                    word_idx: wi,
                    field: "word".into(),
                    issue: "not a JSON object".into(),
                });
                continue;
            };

            let id_val = obj.get("id");
            let is_range = id_val
                .and_then(|v| v.as_array())
                .is_some_and(|a| a.len() > 1);

            for field in &required_string_fields {
                if is_range {
                    continue;
                }

                match obj.get(*field) {
                    None => {
                        let keys: Vec<&String> = obj.keys().collect();
                        diagnostics.push(StanzaWordDiagnostic {
                            sentence_idx: si,
                            word_idx: wi,
                            field: (*field).into(),
                            issue: format!(
                                "field absent (keys present: {keys:?}). \
                                 Stanza's processor likely failed silently for this token."
                            ),
                        });
                    }
                    Some(v) if v.is_null() => {
                        let text = obj.get("text").and_then(|t| t.as_str()).unwrap_or("?");
                        diagnostics.push(StanzaWordDiagnostic {
                            sentence_idx: si,
                            word_idx: wi,
                            field: (*field).into(),
                            issue: format!(
                                "value is null for word '{text}'. \
                                 Stanza's processor likely failed silently."
                            ),
                        });
                    }
                    Some(v) if !v.is_string() => {
                        diagnostics.push(StanzaWordDiagnostic {
                            sentence_idx: si,
                            word_idx: wi,
                            field: (*field).into(),
                            issue: format!(
                                "expected string, got {}",
                                match v {
                                    serde_json::Value::Number(_) => "number",
                                    serde_json::Value::Bool(_) => "bool",
                                    serde_json::Value::Array(_) => "array",
                                    serde_json::Value::Object(_) => "object",
                                    _ => "unknown",
                                }
                            ),
                        });
                    }
                    _ => {}
                }
            }

            if let Some(deprel) = obj.get("deprel").and_then(|v| v.as_str())
                && deprel.starts_with('<')
                && deprel.ends_with('>')
            {
                diagnostics.push(StanzaWordDiagnostic {
                    sentence_idx: si,
                    word_idx: wi,
                    field: "deprel".into(),
                    issue: format!("pad sentinel value '{deprel}'"),
                });
            }

            match id_val {
                None => {
                    diagnostics.push(StanzaWordDiagnostic {
                        sentence_idx: si,
                        word_idx: wi,
                        field: "id".into(),
                        issue: "field absent".into(),
                    });
                }
                Some(v) if !(v.is_number() || v.is_array()) => {
                    diagnostics.push(StanzaWordDiagnostic {
                        sentence_idx: si,
                        word_idx: wi,
                        field: "id".into(),
                        issue: format!("expected number or array, got {v}"),
                    });
                }
                _ => {}
            }
        }
    }

    diagnostics
}

/// Errors from parsing raw Stanza output.
#[derive(Debug, thiserror::Error)]
pub enum StanzaParseError {
    /// A sentence value was not a JSON array.
    #[error("sentence {sentence_idx} is not a JSON array")]
    NotAnArray {
        /// Index of the sentence in the raw output.
        sentence_idx: usize,
    },
    /// A word dict could not be parsed into [`UdWord`].
    #[error("sentence {sentence_idx} word {word_idx}: {source}")]
    WordParse {
        /// Sentence index.
        sentence_idx: usize,
        /// Word index within the sentence.
        word_idx: usize,
        /// Underlying deserialization error.
        source: serde_json::Error,
    },
}

/// Normalize a raw Stanza word dict before deserialization.
fn normalize_word_dict(mut value: serde_json::Value) -> serde_json::Value {
    if let Some(obj) = value.as_object_mut() {
        if let Some(id_val) = obj.get("id")
            && let Some(arr) = id_val.as_array()
            && arr.len() == 1
            && let Some(n) = arr[0].as_u64()
        {
            obj.insert("id".to_string(), serde_json::json!(n));
        }

        let is_range = obj
            .get("id")
            .is_some_and(|v| v.as_array().is_some_and(|a| a.len() > 1));

        if is_range {
            obj.entry("lemma").or_insert(serde_json::json!(""));
            obj.entry("upos").or_insert(serde_json::json!("X"));
            obj.entry("head").or_insert(serde_json::json!(0));
            obj.entry("deprel").or_insert(serde_json::json!("dep"));
            obj.entry("feats").or_insert(serde_json::json!(""));
        } else {
            let lemma_empty = obj
                .get("lemma")
                .is_none_or(|v| v.is_null() || v.as_str().is_some_and(|s| s.is_empty()));
            if lemma_empty && let Some(text) = obj.get("text").and_then(|v| v.as_str()) {
                obj.insert("lemma".to_string(), serde_json::json!(text));
            }
        }

        for (field, default) in [("upos", "X"), ("deprel", "dep"), ("feats", "")] {
            if obj.get(field).is_some_and(|v| v.is_null()) {
                obj.insert(field.to_string(), serde_json::json!(default));
            }
        }
    }

    value
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::morphosyntax::UdId;
    use serde_json::json;

    #[test]
    fn parse_raw_stanza_single_sentence() {
        let raw = vec![json!([
            {
                "id": [1],
                "text": "hello",
                "lemma": "hello",
                "upos": "INTJ",
                "head": 0,
                "deprel": "root"
            }
        ])];

        let resp = parse_raw_stanza_output(&raw).unwrap();
        assert_eq!(resp.sentences.len(), 1);
        assert_eq!(resp.sentences[0].words.len(), 1);
        assert_eq!(resp.sentences[0].words[0].text, "hello");
        assert_eq!(resp.sentences[0].words[0].id, UdId::Single(1));
    }

    #[test]
    fn parse_raw_stanza_mwt() {
        let raw = vec![json!([
            {
                "id": [1, 2],
                "text": "du",
                "lemma": "",
                "upos": "X",
                "head": 0,
                "deprel": "root"
            },
            {
                "id": [1],
                "text": "de",
                "lemma": "de",
                "upos": "ADP",
                "head": 3,
                "deprel": "case"
            },
            {
                "id": [2],
                "text": "le",
                "lemma": "le",
                "upos": "DET",
                "head": 3,
                "deprel": "det"
            }
        ])];

        let resp = parse_raw_stanza_output(&raw).unwrap();
        assert_eq!(resp.sentences[0].words.len(), 3);
        assert_eq!(resp.sentences[0].words[0].id, UdId::Range(1, 2));
        assert!(resp.sentences[0].words[0].lemma.is_empty());
    }

    #[test]
    fn parse_raw_stanza_mwt_absent_fields() {
        let raw = vec![json!([
            {
                "id": [4, 5],
                "text": "au",
                "start_char": 20,
                "end_char": 22
            },
            {
                "id": [4],
                "text": "à",
                "lemma": "à",
                "upos": "ADP",
                "head": 6,
                "deprel": "case"
            },
            {
                "id": [5],
                "text": "le",
                "lemma": "le",
                "upos": "DET",
                "head": 6,
                "deprel": "det"
            }
        ])];

        let resp = parse_raw_stanza_output(&raw).unwrap();
        assert_eq!(resp.sentences[0].words.len(), 3);
        assert_eq!(resp.sentences[0].words[0].id, UdId::Range(4, 5));
        assert_eq!(resp.sentences[0].words[0].text, "au");
        assert!(resp.sentences[0].words[0].lemma.is_empty());
    }

    #[test]
    fn parse_raw_stanza_pad_deprel() {
        let raw = vec![json!([
            {
                "id": 1,
                "text": "hello",
                "lemma": "hello",
                "upos": "INTJ",
                "head": 0,
                "deprel": "<pad>"
            }
        ])];

        let resp = parse_raw_stanza_output(&raw).unwrap();
        assert_eq!(resp.sentences[0].words[0].deprel, "dep");
    }

    #[test]
    fn parse_raw_stanza_bogus_lemma() {
        let raw = vec![json!([
            {
                "id": 1,
                "text": "hello",
                "lemma": ".",
                "upos": "INTJ",
                "head": 0,
                "deprel": "root"
            }
        ])];

        let resp = parse_raw_stanza_output(&raw).unwrap();
        assert_eq!(resp.sentences[0].words[0].lemma, "hello");
    }

    #[test]
    fn parse_raw_stanza_default_lemma() {
        let raw = vec![json!([
            {
                "id": 1,
                "text": "hello",
                "upos": "INTJ",
                "head": 0,
                "deprel": "root"
            }
        ])];

        let resp = parse_raw_stanza_output(&raw).unwrap();
        assert_eq!(resp.sentences[0].words[0].lemma, "hello");
    }

    #[test]
    fn parse_raw_stanza_null_lemma_defaults_to_text() {
        let raw = vec![json!([
            {
                "id": 1,
                "text": "au",
                "lemma": null,
                "upos": "ADP",
                "head": 0,
                "deprel": "root"
            }
        ])];

        let resp = parse_raw_stanza_output(&raw).unwrap();
        assert_eq!(resp.sentences[0].words[0].lemma, "au");
    }

    #[test]
    fn parse_raw_stanza_null_upos_does_not_crash() {
        let raw = vec![json!([
            {
                "id": 1,
                "text": "xyz",
                "lemma": "xyz",
                "upos": null,
                "head": 0,
                "deprel": "root"
            }
        ])];

        let result = parse_raw_stanza_output(&raw);
        assert!(result.is_ok(), "null upos should not crash: {result:?}");
    }

    #[test]
    fn parse_raw_stanza_not_array() {
        let raw = vec![json!("not an array")];
        let err = parse_raw_stanza_output(&raw).unwrap_err();
        assert!(err.to_string().contains("not a JSON array"));
    }

    #[test]
    fn diagnose_detects_missing_lemma() {
        let raw = vec![json!([
            {
                "id": 1,
                "text": "au",
                "upos": "ADP",
                "head": 0,
                "deprel": "root"
            }
        ])];
        let diags = diagnose_parse_failure(&raw);
        assert!(!diags.is_empty());
        assert!(
            diags
                .iter()
                .any(|d| d.field == "lemma" && d.issue.contains("absent"))
        );
    }

    #[test]
    fn diagnose_detects_null_lemma() {
        let raw = vec![json!([
            {
                "id": 1,
                "text": "au",
                "lemma": null,
                "upos": "ADP",
                "head": 0,
                "deprel": "root"
            }
        ])];
        let diags = diagnose_parse_failure(&raw);
        assert!(!diags.is_empty());
        assert!(
            diags
                .iter()
                .any(|d| d.field == "lemma" && d.issue.contains("null"))
        );
    }

    #[test]
    fn diagnose_detects_pad_deprel() {
        let raw = vec![json!([
            {
                "id": 1,
                "text": "hello",
                "lemma": "hello",
                "upos": "INTJ",
                "head": 0,
                "deprel": "<pad>"
            }
        ])];
        let diags = diagnose_parse_failure(&raw);
        assert!(
            diags
                .iter()
                .any(|d| d.field == "deprel" && d.issue.contains("pad"))
        );
    }

    #[test]
    fn diagnose_clean_output_produces_no_diagnostics() {
        let raw = vec![json!([
            {
                "id": 1,
                "text": "hello",
                "lemma": "hello",
                "upos": "INTJ",
                "head": 0,
                "deprel": "root"
            }
        ])];
        let diags = diagnose_parse_failure(&raw);
        assert!(
            diags.is_empty(),
            "Clean output should produce no diagnostics: {diags:?}"
        );
    }

    #[test]
    fn diagnose_skips_lemma_for_range_tokens() {
        let raw = vec![json!([
            {
                "id": [1, 2],
                "text": "du",
                "upos": "X",
                "head": 0,
                "deprel": "root"
            }
        ])];
        let diags = diagnose_parse_failure(&raw);
        assert!(
            !diags.iter().any(|d| d.field == "lemma"),
            "Range token should not be flagged for missing lemma: {diags:?}"
        );
    }
}
