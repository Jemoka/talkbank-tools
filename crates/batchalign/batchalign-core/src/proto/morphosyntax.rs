//! Morphosyntax proto types — drive Stanza-style UD POS / `%mor` / `%gra` tagging.
//!
//! !!! HAND-MIRRORED with `python/batchalign/_core/proto.py::MorphosyntaxInput,
//! MorphosyntaxOutput, MorphosyntaxToken, MorphosyntaxUnit, GraTerminator,
//! MorphosyntaxUtterance`. !!!
//!
//! ## Structured, never stringly
//!
//! The backend emits a fully *structured* morphological analysis: per main-tier
//! word, a head morpho-unit plus any `~`-joined post-clitics, and per chunk a
//! `%gra` dependency triple. It never emits rendered `%mor` / `%gra` tier text.
//! The [`MorphosyntaxTaskRunner`](crate::taskrunners) turns this structure into
//! typed `talkbank_model` `MorTier` / `GraTier` values via
//! `talkbank_model::alignment::try_align_mor_gra` and serializes them with the
//! official CHAT writer. There is deliberately no pre-rendered-string escape
//! hatch in this pipeline — building CHAT text by string concatenation is
//! forbidden (see `CLAUDE.md`).
//!
//! ## Per-utterance dispatch
//!
//! Unlike BA2's whole-file pipeline, the rewrite dispatches **one utterance at a
//! time** to the backend. The backend keeps its language-specific resources
//! (Stanza pipeline) loaded across calls; per-call payloads carry only what the
//! tagger needs for this utterance: pre-split tokens (so the tagger preserves
//! the upstream tokenization unless `retokenize`), the raw text (for
//! `retokenize=true`), and the resolved language.

use crate::cache::{CacheKey, hash_serialized};
use crate::proto::asr::LanguageSpec;
use crate::register_proto_schema;
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

/// One morpho-unit: a single `pos|lemma-feat...` analysis together with its
/// `%gra` dependency relation.
///
/// A unit is the atom of `%mor`: it renders to exactly one
/// [`MorWord`](talkbank_model::model::MorWord) (`pos|lemma` plus hyphen-joined
/// features) and occupies exactly one `%gra` *chunk*. A plain word is one unit;
/// a clitic/MWT word (e.g. `it's`) is a [`MorphosyntaxToken`] holding several
/// units, each its own chunk.
///
/// The `%gra` triple (`index` / `head` / `deprel`) is carried verbatim from the
/// backend rather than recomputed in Rust: BA2's dependency numbering has
/// quirks (skipped tokens shift indices; a ROOT's head renders as the trailing
/// chunk index) that the backend already reproduces. Passing the computed
/// triple keeps parity without re-deriving that logic in two places.
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct MorphosyntaxUnit {
    /// CHAT part-of-speech (lowercase UD, e.g. `pron`, `verb`, `det`). May carry
    /// a leading `0` for an untranscribed/zero form (BA2 convention).
    pub pos: String,
    /// Cleaned lemma / stem (UD). Never contains `&` fusional markers.
    pub lemma: String,
    /// Ordered morphological features, hyphen-joined after the lemma by the
    /// writer (`verb|run-Past-S3`). Each entry is one feature token (`Past`,
    /// `S3`, …) with no separators.
    pub features: Vec<String>,
    /// 1-based `%gra` chunk index for this unit (BA2 numbering, skip-adjusted).
    pub index: u32,
    /// `%gra` head: the chunk index this unit attaches to (`0` = root in UD;
    /// BA2's trailing-chunk quirk for ROOT is preserved verbatim).
    pub head: u32,
    /// `%gra` dependency relation label (uppercased, `:`→`-`, e.g. `NSUBJ`,
    /// `ROOT`, `AUX`).
    pub deprel: String,
}

/// One main-tier word's morphology: a head unit followed by `~`-joined
/// post-clitic units.
///
/// Maps 1:1 to a typed [`Mor`](talkbank_model::model::Mor): `units[0]` is the
/// `main` [`MorWord`](talkbank_model::model::MorWord); `units[1..]` are the
/// `post_clitics`. A non-clitic word has exactly one unit.
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct MorphosyntaxToken {
    /// Surface form of the whole word (diagnostics / alignment aid).
    pub text: String,
    /// Head unit (`[0]`) plus any post-clitic units. Always non-empty for a
    /// rendered word.
    pub units: Vec<MorphosyntaxUnit>,
}

/// The trailing terminator's `%gra` relation (BA2 appends one `…|root|PUNCT`
/// relation after the word chunks).
///
/// The terminator *kind* (`.`/`?`/`!`/…) is read from the utterance's typed
/// main-tier terminator by the runner, so only the dependency triple travels
/// here.
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct GraTerminator {
    /// 1-based chunk index of the terminator (one past the last word chunk).
    pub index: u32,
    /// Head chunk index the terminator attaches to (the ROOT chunk).
    pub head: u32,
    /// Relation label — conventionally `PUNCT`.
    pub deprel: String,
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
    /// When `retokenize` is `false` the backend MUST treat these tokens as the
    /// authoritative word boundaries.
    pub tokens: Vec<String>,
    /// When `true`, the backend may re-split tokens (e.g. expanding
    /// `gonna` → `going to`). When `false`, the upstream tokenization is
    /// authoritative — this is the default for CHAT pipelines where the
    /// main tier already encodes word boundaries.
    #[serde(default)]
    pub retokenize: bool,
    /// Utterance text for backend preprocessing. This may retain narrowly
    /// scoped CHAT signals such as a word-level `@s` language switch even
    /// when `tokens` contains only clean alignment surfaces. With
    /// `retokenize=true`, the backend may also use it as segmentation input;
    /// when empty, the backend falls back to `tokens.join(" ")`.
    #[serde(default)]
    pub text: String,
}

impl CacheKey for MorphosyntaxInput {
    /// Excludes `source_id` + `utterance_id` (routing only). Two utterances
    /// with identical tokens / language / mode collapse to one cache entry,
    /// no matter which file or slot they came from.
    fn hash(&self, hasher: &mut blake3::Hasher) {
        #[derive(Serialize)]
        struct K<'a> {
            language: &'a LanguageSpec,
            tokens: &'a [String],
            retokenize: bool,
            text: &'a str,
        }
        hash_serialized(
            &K {
                language: &self.language,
                tokens: &self.tokens,
                retokenize: self.retokenize,
                text: &self.text,
            },
            hasher,
        );
    }
}

/// Per-utterance output from the morphosyntax backend.
///
/// One per [`MorphosyntaxInput`]; `utterance_id` echoes input so the runner
/// can reattach defensively even if the engine reorders. An empty `tokens`
/// list means "no `%mor`/`%gra` for this utterance" (BA2 emits nothing for
/// degenerate/empty analyses) — the runner injects no tiers in that case.
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct MorphosyntaxOutput {
    /// Echoes the input's `source_id` for routing.
    pub source_id: SourceId,
    /// Echoes the input's `utterance_id`.
    pub utterance_id: u32,
    /// One [`MorphosyntaxToken`] per main-tier word, in document order. Each
    /// carries its head unit + post-clitics; together their units form the
    /// `%gra` chunk sequence.
    pub tokens: Vec<MorphosyntaxToken>,
    /// The terminator's `%gra` relation, present whenever `tokens` is
    /// non-empty. `None` only for degenerate analyses with no tiers.
    #[serde(default)]
    pub terminator: Option<GraTerminator>,
}

register_proto_schema!(MorphosyntaxUtterance);
register_proto_schema!(MorphosyntaxUnit);
register_proto_schema!(MorphosyntaxToken);
register_proto_schema!(GraTerminator);
register_proto_schema!(MorphosyntaxInput);
register_proto_schema!(MorphosyntaxOutput);
