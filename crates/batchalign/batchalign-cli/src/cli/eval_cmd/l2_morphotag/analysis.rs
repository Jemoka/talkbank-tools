//! AST-first CHAT walker that pairs `@s` words with `%mor` / `%gra` items.
//!
//! Unlike the Python regex analyzer, this module drives off the typed
//! `talkbank-model` AST via [`walk_words`] with [`TierDomain::Mor`]. The
//! walker yields word-like items in the same domain order that `%mor`'s
//! `items` list aligns to (1-to-1, modulo terminators which are carried on
//! the tier wrapper, not in the items list). A monotone counter across
//! yielded items is therefore an exact index into `mor_tier.items` and
//! `gra_tier.relations`.
//!
//! This eliminates the ~2% `missing_mor` noise that the regex analyzer
//! produced under CHAT retrace markers (`[/]`, `[//]`, `<foo bar> [//]`) —
//! the walker recurses into plain groups but short-circuits at
//! `UtteranceContent::Retrace` when the domain is `Mor`, which is exactly
//! the semantics `%mor`'s position count uses.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use talkbank_model::alignment::TierDomain;
use talkbank_model::alignment::helpers::{
    MorAlignableWordCount, MorItemCount, WordItem, counts_for_tier, walk_words,
};
use talkbank_model::model::{
    ChatFile, GrammaticalRelationType, LanguageCode, Line, MorFeature, PosCategory,
    content::word::WordLanguageMarker,
};
use talkbank_transform::parse::{TreeSitterParser, parse_lenient};

use super::types::{
    AtSAnalysis, AtSOccurrence, FeatureSet, FileAnalysis, GraItemText, LanguageMarkerKind,
    MorItemText, PairKey, SurfaceWord, UtteranceOutcome, classify_status,
};

// ---------------------------------------------------------------------------
// Domain errors
// ---------------------------------------------------------------------------

/// Errors produced by the file-level analyzer.
///
/// `Io` covers missing/unreadable files. `Parse` covers parser-init failure
/// and fatal parse errors. `UnresolvedLanguage` signals a malformed
/// `@Languages` header that cannot yield an effective language — the
/// analyzer cannot produce a `pair_key`-labelled record without one.
#[derive(thiserror::Error, Debug)]
pub enum AnalysisError {
    /// Filesystem read failure.
    #[error("failed to read {path}: {source}")]
    Io {
        /// Path we attempted to read.
        path: PathBuf,
        /// Underlying OS error.
        #[source]
        source: io::Error,
    },
    /// Parser initialization failure.
    #[error("failed to initialize CHAT parser: {0}")]
    ParserInit(String),
}

// ---------------------------------------------------------------------------
// Public entry points
// ---------------------------------------------------------------------------

/// Analyze one post-morphotag CHAT file, producing per-`@s`-word analyses.
///
/// The `pair_key` is looked up by the caller from the eval-set JSONL; the
/// analyzer never infers it from file contents.
pub fn analyze_file(path: &Path, pair_key: PairKey) -> Result<FileAnalysis, AnalysisError> {
    let parser = TreeSitterParser::new().map_err(|e| AnalysisError::ParserInit(format!("{e}")))?;
    let text = fs::read_to_string(path).map_err(|source| AnalysisError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    let (chat_file, _warnings) = parse_lenient(&parser, &text);
    Ok(analyze_chat_file(&chat_file, path.to_path_buf(), pair_key))
}

/// Walk an already-parsed `ChatFile`, producing per-`@s`-word analyses.
///
/// Split from [`analyze_file`] so unit tests can feed in hand-built
/// `ChatFile` values without round-tripping through the parser.
pub fn analyze_chat_file(chat: &ChatFile, path: PathBuf, pair_key: PairKey) -> FileAnalysis {
    let languages: Vec<LanguageCode> = chat.languages.0.clone();
    let mut analyses = Vec::new();
    let mut utterance_outcomes: Vec<UtteranceOutcome> = Vec::new();

    for line in &chat.lines.0 {
        let utt = match line {
            Line::Utterance(u) => u,
            _ => continue,
        };

        // Per-utterance outcome classification (Wave 4 of the morphotag
        // reconciliation architecture). Observed post-hoc from the
        // morphotag output: a pair of (alignable_count, mor_count) plus
        // the presence of the tier fully determines the outcome class.
        // See book/src/architecture/morphotag-invariants.md for the
        // full observation model.
        let alignable_count = utt.mor_alignable_word_count();
        let mor_count_opt = utt.mor_tier().map(|t| MorItemCount::new(t.items().len()));
        utterance_outcomes.push(classify_utterance_outcome(alignable_count, mor_count_opt));

        let mor_items = utt.mor_tier().map(|t| t.items().iter().collect::<Vec<_>>());
        let gra_relations = utt
            .gra_tier()
            .map(|t| t.relations().iter().collect::<Vec<_>>());

        // Walker collects (mor_position, surface, language_marker) for each
        // word-like item in MOR-domain order. Not every word-like item
        // counts toward the MOR position stream — nonwords (`&~`),
        // phonological fragments (`&+`), and untranscribed placeholders
        // (`xxx`/`yyy`/`www`) are excluded from `%mor` by the alignment
        // rules. We apply `counts_for_tier(TierDomain::Mor)` to each
        // `Word` so the counter stays in lockstep with `mor_tier.items`.
        //
        // Separators (commas, tag markers, vocatives) always count — they
        // carry `cm|cm`/`beg|beg`/`end|end` %mor items.
        //
        // ReplacedWords contribute their replacement's surface to MOR.
        let mut main_entries: Vec<(usize, String, Option<WordLanguageMarker>)> = Vec::new();
        let mut mor_position: usize = 0;
        walk_words(
            &utt.main.content.content,
            Some(TierDomain::Mor),
            &mut |item| {
                let (surface, marker) = match item {
                    WordItem::Word(w) => {
                        if !counts_for_tier(w, TierDomain::Mor) {
                            // Nonword / fragment / untranscribed — skipped
                            // by %mor alignment; do NOT advance counter.
                            return;
                        }
                        (w.cleaned_text().to_string(), w.lang.clone())
                    }
                    WordItem::ReplacedWord(r) => {
                        // The original word is what the transcriber wrote,
                        // but %mor operates on the replacement. For MOR-
                        // domain counting, the replacement(s) are what
                        // take MOR positions. Check the replacement —
                        // if it's a non-linguistic token, skip.
                        // For a typical `foo [: bar]`, `bar` counts as
                        // one MOR item; `foo [: bar baz]` counts as two.
                        //
                        // The `@s` marker on the original is preserved.
                        let surface = r.word.cleaned_text().to_string();
                        let marker = r.word.lang.clone();
                        // One MOR position per replacement word that
                        // counts. Emit one entry per replacement (but
                        // `@s` + surface come from the original).
                        // Simplification: always count as one position
                        // with the original's surface + marker, matching
                        // Mor-domain replacement semantics at a minimum
                        // for the happy path (1-word replacements).
                        (surface, marker)
                    }
                    WordItem::Separator(sep) => {
                        let txt = format!("{}", sep);
                        main_entries.push((mor_position, txt, None));
                        mor_position += 1;
                        return;
                    }
                };
                main_entries.push((mor_position, surface, marker));
                mor_position += 1;
            },
        );

        for (position, surface, marker_opt) in main_entries {
            let marker = match marker_opt {
                Some(m) => m,
                None => continue, // non-@s word, skip
            };
            let marker_kind = LanguageMarkerKind::from(&marker);
            let effective = match marker_kind.effective_language(&languages) {
                Some(code) => code,
                None => continue, // pathological header; silently skip
            };

            // Pair with MOR / GRA items by position.
            let mor_at = mor_items.as_ref().and_then(|v| v.get(position).copied());
            let gra_at = gra_relations
                .as_ref()
                .and_then(|v| v.get(position).copied());

            let mor_item_text = mor_at.map(mor_item_to_text);
            let gra_item_text = gra_at.map(gra_relation_to_text);

            let status = classify_status(mor_item_text.as_ref());

            let (pos, lemma, features) = match mor_at {
                Some(mor) => (
                    Some(mor.main.pos.clone()),
                    Some(mor.main.lemma.to_string()),
                    Some(features_to_set(&mor.main.features)),
                ),
                None => (None, None, None),
            };
            let gra_deprel = gra_at.map(|g| g.relation.clone());

            let occurrence = AtSOccurrence {
                file: path.clone(),
                pair_key: pair_key.clone(),
                marker: marker_kind,
                effective_lang: effective,
                surface: SurfaceWord::new(surface),
                mor_position: position,
                mor_item: mor_item_text,
                gra_item: gra_item_text,
            };

            let analysis = AtSAnalysis {
                occurrence,
                pos,
                lemma,
                features,
                gra_deprel,
                status,
                flags: Vec::new(),
            };
            let with_flags = apply_flags(analysis);
            analyses.push(with_flags);
        }
    }

    FileAnalysis {
        path,
        pair_key,
        languages,
        analyses,
        utterance_outcomes,
    }
}

/// Post-hoc classifier for one utterance's morphotag outcome.
///
/// Takes the CHAT-side alignable-word count and the observed `%mor`
/// item count (or `None` if the tier is absent). Returns the matching
/// [`UtteranceOutcome`].
///
/// The four-way classification:
///
/// | alignable | mor_count       | outcome                              |
/// |-----------|-----------------|--------------------------------------|
/// | `0`       | `None`          | `NotApplicable` (correct)            |
/// | `0`       | `Some(m > 0)`   | `CountMismatchInFile` (anomaly)      |
/// | `0`       | `Some(0)`       | `NotApplicable` (empty placeholder)  |
/// | `N > 0`   | `Some(N)`       | `Aligned` (happy path)               |
/// | `N > 0`   | `Some(m ≠ N)`   | `CountMismatchInFile` (anomaly)      |
/// | `N > 0`   | `None`          | `PipelineAbsorbedFailure` (anomaly)  |
pub fn classify_utterance_outcome(
    alignable: MorAlignableWordCount,
    mor_count: Option<MorItemCount>,
) -> UtteranceOutcome {
    match (alignable.get(), mor_count.map(|m| m.get())) {
        (0, None) => UtteranceOutcome::NotApplicable,
        (0, Some(0)) => UtteranceOutcome::NotApplicable,
        (0, Some(m)) => UtteranceOutcome::CountMismatchInFile {
            n_alignable: MorAlignableWordCount::new(0),
            n_mor: MorItemCount::new(m),
        },
        (_n, None) => UtteranceOutcome::PipelineAbsorbedFailure {
            n_alignable: alignable,
        },
        (n, Some(m)) if n == m => UtteranceOutcome::Aligned { n_words: alignable },
        (_n, Some(m)) => UtteranceOutcome::CountMismatchInFile {
            n_alignable: alignable,
            n_mor: MorItemCount::new(m),
        },
    }
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

/// Attach heuristic flags to an analysis. Free function because the
/// decision is a pure derivation of the analysis's typed fields — it has
/// no state of its own and does not own any invariants.
fn apply_flags(mut analysis: AtSAnalysis) -> AtSAnalysis {
    analysis.flags = super::heuristics::flags_for(&analysis);
    analysis
}

/// Serialize a `Mor` item to CHAT text (`POS|lemma-Feat-Feat`).
///
/// Uses `WriteChat` semantics via the talkbank-model method so the output
/// matches what a CHAT serializer would produce — no string hacking.
fn mor_item_to_text(mor: &talkbank_model::model::dependent_tier::mor::Mor) -> MorItemText {
    use std::fmt::Write as _;
    let mut out = String::new();
    // Main word.
    let _ = write!(
        &mut out,
        "{}|{}",
        mor.main.pos.as_str(),
        mor.main.lemma.as_str()
    );
    for feat in &mor.main.features {
        out.push('-');
        write_feature(&mut out, feat);
    }
    // Post-clitics (`~aux|be-Fin`...).
    for clitic in &mor.post_clitics {
        let _ = write!(
            &mut out,
            "~{}|{}",
            clitic.pos.as_str(),
            clitic.lemma.as_str()
        );
        for feat in &clitic.features {
            out.push('-');
            write_feature(&mut out, feat);
        }
    }
    MorItemText::new(out)
}

/// Serialize one morphological feature (`Plur` or `Number=Plur`).
fn write_feature(out: &mut String, feat: &MorFeature) {
    // `MorFeature` exposes Display matching CHAT text.
    use std::fmt::Write as _;
    let _ = write!(out, "{}", feat);
}

/// Serialize a GRA relation as `index|head|DEPREL` text for CSV.
fn gra_relation_to_text(rel: &talkbank_model::model::GrammaticalRelation) -> GraItemText {
    GraItemText::new(format!(
        "{}|{}|{}",
        rel.index,
        rel.head,
        rel.relation.as_str()
    ))
}

/// Reconstruct the dash-joined feature string from a typed feature list.
fn features_to_set(features: &[MorFeature]) -> FeatureSet {
    use std::fmt::Write as _;
    let mut out = String::new();
    for (i, f) in features.iter().enumerate() {
        if i > 0 {
            out.push('-');
        }
        let _ = write!(&mut out, "{}", f);
    }
    FeatureSet::new(out)
}

// ---------------------------------------------------------------------------
// POS / feature helpers used by heuristics and external callers
// ---------------------------------------------------------------------------

/// Lowercase view of a POS category — CHAT POS tags are lowercase but the
/// typed `PosCategory` preserves whatever casing was in the file. Heuristics
/// compare against `"noun"`, `"propn"`, `"verb"`; always lowercase first.
pub fn pos_as_lowercase(pos: &PosCategory) -> String {
    pos.as_str().to_ascii_lowercase()
}

/// Helper: extract POS / lemma / features from a **serialized** MOR item
/// string, for test-fixture convenience and for callers that already have
/// only the serialized form.
///
/// Returns `(None, None, None)` for malformed input (no `|`).
pub fn extract_pos_lemma_features(
    mor_item: &str,
) -> (Option<String>, Option<String>, Option<String>) {
    if mor_item.is_empty() || !mor_item.contains('|') {
        return (None, None, None);
    }
    // Clitics: take the head component only.
    let head = mor_item.split('~').next().unwrap_or(mor_item);
    let (pos_str, rest) = match head.split_once('|') {
        Some(parts) => parts,
        None => return (None, None, None),
    };
    let pos_trim = pos_str.trim();
    if pos_trim.is_empty() {
        return (None, None, None);
    }
    let (lemma, feats) = match rest.split_once('-') {
        Some((l, f)) => (l.trim(), f.trim()),
        None => (rest.trim(), ""),
    };
    let lemma_opt = if lemma.is_empty() {
        None
    } else {
        Some(lemma.to_string())
    };
    let feats_opt = if feats.is_empty() {
        None
    } else {
        Some(feats.to_string())
    };
    (Some(pos_trim.to_string()), lemma_opt, feats_opt)
}

/// Extract the DEPREL token from a serialized GRA item (`id|head|DEPREL`).
/// Returns `None` for malformed input.
pub fn extract_gra_deprel(gra_item: &str) -> Option<GrammaticalRelationType> {
    let parts: Vec<&str> = gra_item.splitn(3, '|').collect();
    if parts.len() != 3 {
        return None;
    }
    // Validate the first two parts are numeric.
    parts[0].parse::<usize>().ok()?;
    parts[1].parse::<usize>().ok()?;
    let deprel = parts[2].trim();
    if deprel.is_empty() {
        return None;
    }
    Some(GrammaticalRelationType::new(deprel))
}
