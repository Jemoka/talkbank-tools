//! Payload collection and `%mor`/`%gra` mutation passes.
//!
//! [`collect_payloads`] walks a `ChatFile`, builds the per-utterance
//! [`MorphosyntaxBatchItem`] list that gets sent to the Stanza worker,
//! and classifies every utterance with zero alignable content into a
//! [`MorOutcome`]. The mutation helpers ([`clear_morphosyntax`],
//! [`remove_empty_morphosyntax_placeholders`],
//! [`clear_morphosyntax_selective`], [`validate_mor_alignment`]) and
//! the small [`prepare_text`] adapter live here too because they share
//! the same iteration shape over `ChatFile.lines`.

use talkbank_model::WriteChat;
use talkbank_model::alignment::helpers::TierDomain;
use talkbank_model::model::{Line, SpeakerCode};

use crate::extract::{self, ExtractedWord};

use super::outcome::{MorOutcome, MorOutcomeKind, classify_not_applicable};
use super::types::MultilingualPolicy;

// The Stanza placeholder constant moved to
// `talkbank_model::ChatCleanedText::stanza_placeholder()` as the only
// blessed exception to provenance sealing. See its doc comment for
// details and the post-Stanza synthesis recognition logic in
// `morphosyntax/synthesis/`.

/// Batch item for morphosyntax NLP processing.
#[derive(Clone, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
pub struct MorphosyntaxBatchItem {
    /// Word texts for NLP processing. Each word is provenance-sealed
    /// `ChatCleanedText` derived from a parsed `Word` or `Separator` —
    /// or, for non-`@s` special-form positions, the blessed
    /// `ChatCleanedText::stanza_placeholder()` constant.
    #[schemars(with = "Vec<String>")]
    pub words: Vec<talkbank_model::ChatCleanedText>,
    /// Utterance terminator. Typed; serializes to its CHAT surface form
    /// (`.`, `?`, `!`, etc.) over the IPC boundary so the Stanza worker
    /// continues to receive a plain string.
    #[serde(
        serialize_with = "serialize_terminator_as_chat_str",
        deserialize_with = "deserialize_terminator_from_chat_str"
    )]
    #[schemars(with = "String")]
    pub terminator: talkbank_model::Terminator,
    /// Special form and language per word: (form_type, resolved_language).
    #[serde(serialize_with = "serialize_special_forms")]
    #[schemars(with = "Vec<(Option<String>, Option<String>)>")]
    pub special_forms: Vec<(
        Option<talkbank_model::model::FormType>,
        Option<talkbank_model::validation::LanguageResolution>,
    )>,
    /// Language code for this utterance (ISO 639-3).
    #[schemars(with = "String")]
    pub lang: talkbank_model::model::LanguageCode,
}

fn serialize_terminator_as_chat_str<S: serde::Serializer>(
    term: &talkbank_model::Terminator,
    serializer: S,
) -> Result<S::Ok, S::Error> {
    serializer.serialize_str(&term.to_string())
}

fn deserialize_terminator_from_chat_str<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<talkbank_model::Terminator, D::Error> {
    use serde::de::Error;
    let s = <String as serde::Deserialize>::deserialize(deserializer)?;
    talkbank_model::Terminator::try_from_chat_str(s.trim())
        .ok_or_else(|| D::Error::custom(format!("unrecognized CHAT terminator string: {s:?}")))
}

fn serialize_special_forms<S: serde::Serializer>(
    forms: &[(
        Option<talkbank_model::model::FormType>,
        Option<talkbank_model::validation::LanguageResolution>,
    )],
    serializer: S,
) -> Result<S::Ok, S::Error> {
    use serde::ser::SerializeSeq;

    let mut seq = serializer.serialize_seq(Some(forms.len()))?;
    for (form_type, lang_res) in forms {
        let ft_str: Option<String> = form_type.as_ref().map(|ft| {
            let mut buf = String::new();
            #[allow(clippy::expect_used)]
            ft.write_chat(&mut buf)
                .expect("writing CHAT to a String should be infallible");
            buf
        });
        let lang_str: Option<String> = lang_res
            .as_ref()
            .and_then(|lr| lr.languages().first().map(|lc| lc.to_string()));
        seq.serialize_element(&(ft_str, lang_str))?;
    }
    seq.end()
}

/// A collected batch item with its position in the `ChatFile`, for injection.
pub type BatchItemWithPosition = (usize, usize, MorphosyntaxBatchItem, Vec<ExtractedWord>);

/// Validation warning for a single utterance.
#[derive(Debug)]
pub struct AlignmentWarning {
    /// Zero-based line index in the `ChatFile`.
    pub line_idx: usize,
    /// Main tier word count (alignable words in the Mor domain).
    pub main_count: usize,
    /// `%mor` item count.
    pub mor_count: usize,
}

impl std::fmt::Display for AlignmentWarning {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "line {}: main tier has {} alignable words but %mor has {} items",
            self.line_idx, self.main_count, self.mor_count,
        )
    }
}

/// Result of walking a `ChatFile` for morphotag payload collection.
pub struct PayloadCollection {
    /// Utterances that will be sent to the NLP worker.
    pub batch_items: Vec<BatchItemWithPosition>,
    /// Utterances that had zero Mor-alignable content.
    pub not_applicable: Vec<MorOutcome>,
    /// Total number of utterance lines in the file.
    pub total_utterances: usize,
}

/// Walk utterances, build typed payloads, and classify every utterance that had
/// zero Mor-alignable content into a `MorOutcome`.
pub fn collect_payloads(
    chat_file: &talkbank_model::model::ChatFile,
    primary_lang: &talkbank_model::model::LanguageCode,
    declared_languages: &[talkbank_model::model::LanguageCode],
    multilingual_policy: MultilingualPolicy,
) -> PayloadCollection {
    if crate::parse::is_ca(chat_file) {
        return PayloadCollection {
            batch_items: Vec::new(),
            not_applicable: Vec::new(),
            total_utterances: 0,
        };
    }

    let total_utts = chat_file
        .lines
        .iter()
        .filter(|l| matches!(l, Line::Utterance(_)))
        .count();

    let mut batch_items: Vec<BatchItemWithPosition> = Vec::new();
    let mut not_applicable: Vec<MorOutcome> = Vec::new();
    let mut utt_idx = 0usize;

    for (line_idx, line) in chat_file.lines.iter().enumerate() {
        let utt = match line {
            Line::Utterance(u) => u,
            _ => continue,
        };

        let utterance_lang = utt.main.content.language_code.clone().unwrap_or_else(|| {
            declared_languages
                .first()
                .cloned()
                .unwrap_or_else(|| primary_lang.clone())
        });

        let skip = multilingual_policy.should_skip_non_primary()
            && utt.main.content.language_code.is_some()
            && utt.main.content.language_code.as_ref() != Some(primary_lang);

        let has_mor = utt.dependent_tiers.iter().any(|t| match t {
            talkbank_model::model::DependentTier::Mor(m) => !m.items().is_empty(),
            _ => false,
        });

        if !skip && !has_mor {
            let mut words = Vec::new();
            extract::collect_utterance_content(
                &utt.main.content.content,
                TierDomain::Mor,
                &mut words,
            );

            if !words.is_empty() {
                // CA-mode utterances may legitimately lack a main-tier
                // terminator. Stanza needs a sentence-final tagging
                // signal regardless, so synthesize Period — matches the
                // BA2 default. See `docs/coding-standards.md` rule 6d
                // for why this isn't a sentinel: it's the canonical
                // Stanza-input default explicitly chosen for the
                // ambiguous case, not a stand-in for a parse failure.
                //
                // CA-prosody arrows (`→`, `↗`, `↘`, `≋`, `≈`, …) are
                // content separators per CHECK; the parser mis-types
                // them as `Terminator::Ca*` (BUG-009) and a raw copy
                // would emit them into `%mor` as invalid CHAT.
                // Normalize to Period for the morphotag payload only;
                // the main tier still round-trips the original variant.
                let terminator_typed: talkbank_model::Terminator =
                    match utt.main.content.terminator.clone() {
                        None
                        | Some(talkbank_model::Terminator::CaRisingToHigh { .. })
                        | Some(talkbank_model::Terminator::CaRisingToMid { .. })
                        | Some(talkbank_model::Terminator::CaLevel { .. })
                        | Some(talkbank_model::Terminator::CaFallingToMid { .. })
                        | Some(talkbank_model::Terminator::CaFallingToLow { .. })
                        | Some(talkbank_model::Terminator::CaTechnicalBreak { .. })
                        | Some(talkbank_model::Terminator::CaTechnicalBreakLinker { .. })
                        | Some(talkbank_model::Terminator::CaNoBreak { .. })
                        | Some(talkbank_model::Terminator::CaNoBreakLinker { .. }) => {
                            talkbank_model::Terminator::Period {
                                span: talkbank_model::Span::DUMMY,
                            }
                        }
                        Some(t) => t,
                    };

                // Resolution must use the same language as dispatch.
                // Pre-2026-05-02 this had a separate `or(Some(primary_lang))`
                // fallback that skipped the `declared_languages.first()`
                // step used by `utterance_lang` above. For a Catalan/Spanish
                // file with no per-utterance precoding and a job-level
                // `primary_lang="eng"` (fabricated by the dispatch layer
                // when `WorkerLanguage::Unspecified`), the two paths
                // disagreed: dispatch ran as `cat`, resolution ran as
                // `eng`. The mismatch produced an `Unresolved` (after
                // today's resolver rule-6d fix) for every `@s` position
                // and a fabricated `Single("eng")` before the fix —
                // which is the dona@s observed bug.
                let tier_language = Some(&utterance_lang);

                let special_forms: Vec<(
                    Option<talkbank_model::model::FormType>,
                    Option<talkbank_model::validation::LanguageResolution>,
                )> = words
                    .iter()
                    .map(|w| {
                        let resolved_lang = if let Some(ref lang_marker) = w.lang {
                            use talkbank_model::model::Word;
                            use talkbank_model::validation::resolve_word_language;

                            let mut temp_word =
                                Word::new_unchecked(w.text.as_str(), w.text.as_str());
                            temp_word.lang = Some(lang_marker.clone());

                            let outcome = resolve_word_language(
                                &temp_word,
                                tier_language,
                                declared_languages,
                            );
                            for err in &outcome.diagnostics {
                                tracing::warn!(error = %err, "word language resolution issue");
                            }
                            Some(outcome.resolution)
                        } else {
                            None
                        };

                        (w.form_type.clone(), resolved_lang)
                    })
                    .collect();

                // Pre-Stanza placeholder substitution for special-form
                // words (excluding `@s`, which the L2 secondary-dispatch
                // path handles). Stanza sees the placeholder, not the
                // non-word, so the surrounding parse stays clean. The
                // synthesis pass in `inject_results` replaces the
                // placeholder's analysis with form-type-derived MOR.
                let word_texts: Vec<talkbank_model::ChatCleanedText> = words
                    .iter()
                    .zip(special_forms.iter())
                    .map(|(w, (form_type, resolved_lang))| {
                        if form_type.is_some() && resolved_lang.is_none() {
                            talkbank_model::ChatCleanedText::stanza_placeholder()
                        } else {
                            w.text.clone()
                        }
                    })
                    .collect();

                batch_items.push((
                    line_idx,
                    utt_idx,
                    MorphosyntaxBatchItem {
                        words: word_texts,
                        terminator: terminator_typed,
                        special_forms,
                        lang: utterance_lang,
                    },
                    words,
                ));
            } else {
                not_applicable.push(MorOutcome {
                    line_idx,
                    speaker: SpeakerCode::new(utt.main.speaker.as_str()),
                    kind: MorOutcomeKind::NotApplicable {
                        reason: classify_not_applicable(utt),
                    },
                });
            }
        }

        utt_idx += 1;
    }

    PayloadCollection {
        batch_items,
        not_applicable,
        total_utterances: total_utts,
    }
}

/// Extract declared languages from the `@Languages` header, with fallback to
/// `primary_lang` if none were declared.
pub fn declared_languages(
    chat_file: &talkbank_model::model::ChatFile,
    primary_lang: &talkbank_model::model::LanguageCode,
) -> Vec<talkbank_model::model::LanguageCode> {
    if chat_file.languages.is_empty() {
        vec![primary_lang.clone()]
    } else {
        chat_file.languages.0.clone()
    }
}

/// Reset every existing `%mor` and `%gra` tier to an empty body in place,
/// preserving original dependent-tier order.
pub fn clear_morphosyntax(chat_file: &mut talkbank_model::model::ChatFile) {
    // CA files (`@Options: CA`) are pass-through for morphosyntax — same
    // discipline as `@Options: NoAlign` for the align pipeline. The morphotag
    // pipeline short-circuits before reaching this function for CA files
    // (see `pipeline/morphosyntax.rs::should_skip_inference` and
    // `morphosyntax/batch.rs` `dummy_flags`). This guard is a defence in
    // depth: if a future caller invokes `clear_morphosyntax` on a CA file
    // directly, leave the file untouched rather than silently strip
    // existing `%mor` / `%gra` content the researcher placed there.
    if crate::parse::is_ca(chat_file) {
        return;
    }

    for line in chat_file.lines.iter_mut() {
        if let Line::Utterance(utt) = line {
            reset_mor_gra_in_place(utt);
        }
    }
}

fn reset_mor_gra_in_place(utterance: &mut talkbank_model::model::Utterance) {
    use talkbank_model::model::dependent_tier::{GraTier, MorTier};
    use talkbank_model::model::{DependentTier, Terminator};

    // Position-preserving so subsequent `replace_or_add_tier` finds
    // the same variant slot; a `retain`-removal would append the
    // re-injected tier at the end and reorder dependent tiers.
    for tier in utterance.dependent_tiers.iter_mut() {
        match tier {
            DependentTier::Mor(_) => {
                *tier = DependentTier::Mor(MorTier::new_mor(
                    Vec::new(),
                    Terminator::Period {
                        span: talkbank_model::Span::DUMMY,
                    },
                ));
            }
            DependentTier::Gra(_) => {
                *tier = DependentTier::Gra(GraTier::new_gra(Vec::new()));
            }
            _ => {}
        }
    }
}

/// Remove any `%mor` or `%gra` tiers that are still empty after the inject pass.
pub fn remove_empty_morphosyntax_placeholders(chat_file: &mut talkbank_model::model::ChatFile) {
    use talkbank_model::model::DependentTier;

    for line in chat_file.lines.iter_mut() {
        if let Line::Utterance(utt) = line {
            utt.dependent_tiers.retain(|tier| match tier {
                DependentTier::Mor(m) => !m.items().is_empty(),
                DependentTier::Gra(g) => !g.relations().is_empty(),
                _ => true,
            });
        }
    }
}

/// Clear `%mor`/`%gra` tiers only from utterances at specific ordinals.
pub fn clear_morphosyntax_selective(
    chat_file: &mut talkbank_model::model::ChatFile,
    utterance_ordinals: &std::collections::HashSet<usize>,
) {
    let mut utt_idx = 0usize;
    for line in chat_file.lines.iter_mut() {
        if let Line::Utterance(utt) = line {
            if utterance_ordinals.contains(&utt_idx) {
                reset_mor_gra_in_place(utt);
            }
            utt_idx += 1;
        }
    }
}

/// Validate that every utterance's `%mor` word count equals the main-tier
/// alignable word count.
pub fn validate_mor_alignment(
    chat_file: &talkbank_model::model::ChatFile,
) -> Vec<AlignmentWarning> {
    use talkbank_model::alignment::helpers::count_tier_positions;
    use talkbank_model::model::DependentTier;

    let mut warnings = Vec::new();

    for (line_idx, line) in chat_file.lines.iter().enumerate() {
        let utt = match line {
            Line::Utterance(u) => u,
            _ => continue,
        };

        let mor_tier = utt.dependent_tiers.iter().find_map(|t| match t {
            DependentTier::Mor(m) => Some(m),
            _ => None,
        });

        let Some(mor) = mor_tier else {
            continue;
        };

        let main_count = count_tier_positions(&utt.main.content.content, TierDomain::Mor);
        let mor_count = mor.len();

        if main_count != mor_count {
            warnings.push(AlignmentWarning {
                line_idx,
                main_count,
                mor_count,
            });
        }
    }

    warnings
}

/// Join words with spaces and strip parentheses for morphosyntax inference.
pub fn prepare_text(words: &[String]) -> String {
    let joined = words.join(" ");
    joined.replace(['(', ')'], "").trim().to_string()
}
