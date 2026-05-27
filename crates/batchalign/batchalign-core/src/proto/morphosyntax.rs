//! Morphosyntax proto types — drive Stanza-style UD POS / `%mor` / `%gra` tagging.
//!
//! !!! HAND-MIRRORED with `python/batchalign/_core/proto.py::MorphosyntaxInput,
//! MorphosyntaxOutput, MorphosyntaxUtterance, MorphosyntaxToken, TaggedUtterance`. !!!
//!
//! ## Per-utterance dispatch
//!
//! Unlike BA2's whole-file pipeline, the rewrite dispatches **one utterance at a
//! time** to the backend. The backend keeps its language-specific resources
//! (Stanza pipeline, BERT segmenter) loaded across calls; per-call payloads
//! carry only what the tagger needs for this utterance: pre-split tokens
//! (so the tagger preserves the upstream tokenization unless `retokenize`),
//! the raw text (for retokenize=true), and the resolved language.
//!
//! `MorphosyntaxUtterance` / `TaggedUtterance` are retained as standalone
//! data shapes for the Python parity probe (`tests/proto_parity.rs`) and for
//! cases where engines want to ship grouped batches; the runtime dispatch path
//! is per-utterance.

use crate::register_proto_schema;
use crate::proto::asr::LanguageSpec;
use crate::utils::SourceId;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// A grouped utterance shape kept for parity / batching code paths.
///
/// The Rust runner dispatches per-utterance (see [`MorphosyntaxInput`]); this
/// type exists so the Python side and any engine wrapping a multi-utterance
/// batch keep a stable name.
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct MorphosyntaxUtterance {
    /// Speaker code (e.g. `"CHI"`, `"MOT"`) for downstream reattachment.
    pub speaker: String,
    /// Plain-text utterance the tagger should analyze.
    pub text: String,
}

/// One tagged token in a tagged utterance.
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct MorphosyntaxToken {
    /// Surface form.
    pub text: String,
    /// Lemma (UD).
    pub lemma: String,
    /// UD POS tag — emitted as the `%mor` POS field verbatim.
    pub upos: String,
    /// UD features in `Key=Value` or flat form, lossless. Emitted hyphen-joined
    /// after the lemma (`verb|run-Past-S3`). Never use `&` markers.
    pub features: Vec<String>,
    /// Optional dependency head index (1-based within the utterance, 0 for ROOT).
    #[serde(default)]
    pub head: Option<u32>,
    /// Optional UD dependency relation label (e.g. `nsubj`, `root`, `punct`).
    #[serde(default)]
    pub deprel: Option<String>,
}

/// One utterance after tagging — carries optional rendered `%mor` and `%gra`
/// strings the runner can drop into the AST directly.
///
/// Retained as a stable parity name; the per-call runtime shape is
/// [`MorphosyntaxOutput`].
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct TaggedUtterance {
    /// Echoes the input order; runners use this to reattach.
    pub speaker: String,
    /// Tokens in document order.
    pub tokens: Vec<MorphosyntaxToken>,
    /// Pre-rendered `%mor` content (no leading `%mor:` label).
    #[serde(default)]
    pub mor: Option<String>,
    /// Pre-rendered `%gra` content.
    #[serde(default)]
    pub gra: Option<String>,
}

/// Per-utterance input to the morphosyntax backend.
///
/// One `MorphosyntaxInput` is dispatched per main-tier utterance in the
/// document. The backend (Stanza, …) keeps its loaded language pipeline
/// across calls.
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct MorphosyntaxInput {
    /// Identity dedupe key for the parent CHAT file.
    pub source_id: SourceId,
    /// Zero-based index of this utterance within the file (the order
    /// produced by `ChatFile::utterances()`).
    pub utterance_id: u32,
    /// Language hint — frequently [`LanguageSpec::PerFile`].
    pub language: LanguageSpec,
    /// Pre-segmented tokens from the upstream AST.
    ///
    /// When `retokenize` is `false` the backend MUST use these tokens
    /// verbatim and produce exactly `tokens.len()` `%mor` items.
    pub tokens: Vec<String>,
    /// When `true`, the backend may re-split tokens (e.g. expanding
    /// `gonna` → `going to`). When `false`, the upstream tokenization is
    /// authoritative — this is the default for CHAT pipelines where the
    /// main tier already encodes word boundaries.
    #[serde(default)]
    pub retokenize: bool,
    /// Plain-text utterance — populated for `retokenize=true` so the
    /// backend can apply its own segmenter (it falls back to
    /// `tokens.join(" ")` when empty).
    #[serde(default)]
    pub text: String,
}

/// Per-utterance output from the morphosyntax backend.
///
/// One per [`MorphosyntaxInput`]; `utterance_id` echoes input so the runner
/// can reattach defensively even if the engine reorders.
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct MorphosyntaxOutput {
    /// Echoes the input's `source_id` for routing.
    pub source_id: SourceId,
    /// Echoes the input's `utterance_id`.
    pub utterance_id: u32,
    /// Tokens in document order. When `retokenize=true` this may differ
    /// in length from the input `tokens`.
    pub tokens: Vec<MorphosyntaxToken>,
    /// Pre-rendered `%mor` content (no leading `%mor:` label). If absent,
    /// the runner renders from `tokens`.
    #[serde(default)]
    pub mor: Option<String>,
    /// Pre-rendered `%gra` content. If absent, the runner renders from
    /// `tokens` (using head/deprel where available).
    #[serde(default)]
    pub gra: Option<String>,
}

register_proto_schema!(MorphosyntaxUtterance);
register_proto_schema!(MorphosyntaxToken);
register_proto_schema!(TaggedUtterance);
register_proto_schema!(MorphosyntaxInput);
register_proto_schema!(MorphosyntaxOutput);
