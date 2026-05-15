//! Inject morphosyntax data into a CHAT AST utterance.
//!
//! After parsing %mor and %gra strings into typed Rust structures,
//! this module adds full tier structs to the utterance's dependent tiers.
//!
//! [`inject_morphosyntax`] validates alignment before injecting:
//! - MOR item count must match alignable word count (Mor domain).
//! - GRA relation count must match MOR chunk count.
//!
//! A count mismatch is always a bug in one of the three stages that
//! together make the 1-to-1 invariant hold (CHAT extraction via
//! `counts_for_tier`, Stanza tokenizer realignment via
//! `tok_ctx.original_words`, UD→Mor mapping with MWT reassembly).
//! The typed [`MisalignmentDiagnostic`] error carries the input words
//! and the expected/actual counts so the caller can triage without
//! re-running. See `morphosyntax/outcome.rs` for the full outcome model.

use talkbank_model::WriteChat;
use talkbank_model::alignment::helpers::{MorAlignableWordCount, MorItemCount};
use talkbank_model::model::{DependentTier, GrammaticalRelation, Mor, MorTier, Utterance};

use crate::extract::collect_utterance_content;

pub use crate::dependent_tiers::replace_or_add_tier;

/// Diagnostic data for a misalignment bug — enough to triage the failing stage
/// without re-running the pipeline.
#[derive(Debug, Clone, thiserror::Error)]
#[error(
    "morphotag misalignment: class={} expected={expected} actual={actual} \
     chat_words={chat_words:?} stanza_tokens={stanza_tokens_after_mapping:?}",
    suspected_class.as_str()
)]
pub struct MisalignmentDiagnostic {
    /// The Mor-alignable words extracted from the CHAT main tier.
    pub chat_words: Vec<String>,
    /// The tokens produced after tokenizer realignment and mapping.
    pub stanza_tokens_after_mapping: Vec<String>,
    /// Expected count: CHAT Mor-alignable words.
    pub expected: MorAlignableWordCount,
    /// Actual count: `%mor` items produced by mapping.
    pub actual: MorItemCount,
    /// Best-effort classification of the likely failing stage.
    pub suspected_class: MisalignmentClass,
}

/// Best-effort classification of which pipeline stage caused a morphotag
/// misalignment bug.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MisalignmentClass {
    /// Stanza realignment context was not applied.
    RealignmentSkipped,
    /// MWT Range reassembly consumed the wrong number of tokens.
    MwtReassemblyBug,
    /// Terminator filtering dropped too many or too few punctuation tokens.
    TerminatorFilterBug,
    /// Code-switched dispatch disagreed with CHAT main-tier counts.
    LanguageDispatchIssue,
    /// `%mor` chunk count and `%gra` relation count disagreed at injection
    /// time. Upstream sentence-mapping likely failed to append a terminator
    /// PUNCT relation (or appended one for an utterance that doesn't have a
    /// terminator chunk). dona@s bug class.
    MorGraCountMismatch,
    /// Diagnostic data is insufficient to classify more precisely.
    Unknown,
}

impl MisalignmentClass {
    /// Short label for `%xalign` tier output.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::RealignmentSkipped => "realignment_skipped",
            Self::MwtReassemblyBug => "mwt_reassembly_bug",
            Self::TerminatorFilterBug => "terminator_filter_bug",
            Self::LanguageDispatchIssue => "language_dispatch_issue",
            Self::MorGraCountMismatch => "mor_gra_count_mismatch",
            Self::Unknown => "unknown",
        }
    }
}

/// Inject parsed Mor items and GRA relations into an utterance.
///
/// Validates alignment before injecting — catches count mismatches at the
/// point of corruption rather than deferring to pre-serialization validation.
///
/// # Errors
///
/// Returns a [`MisalignmentDiagnostic`] when the 1-to-1 invariant is
/// violated (Mor count ≠ Mor-alignable CHAT word count). The diagnostic
/// carries `chat_words`, `expected`, `actual`, and a placeholder
/// `suspected_class` of [`MisalignmentClass::Unknown`]. Callers that have
/// additional context (Stanza tokens, whether retokenization was used,
/// whether the utterance was code-switched) may enrich the diagnostic
/// before surfacing it.
/// # Instrumentation contract
///
/// Every call to this function emits at least one `tracing` event so an
/// operator running with `--verbose` (filter ≥ info) can reconstruct the
/// inject path taken for any utterance. Events:
///
/// | Branch                                   | Level | Target message                        |
/// |------------------------------------------|-------|---------------------------------------|
/// | Entry (always)                           | info  | `inject_morphosyntax: enter`          |
/// | mors empty (skip)                        | info  | `inject_morphosyntax: skip empty`     |
/// | word/mor count mismatch (returns Err)    | warn  | `MOR count mismatch ...`              |
/// | %mor-only (no %gra) install              | info  | `inject_morphosyntax: mor-only`       |
/// | %mor/%gra count mismatch (returns Err)   | warn  | `%mor/%gra count mismatch ...`        |
/// | aligned (success)                        | info  | `inject_morphosyntax: aligned`        |
///
/// At the daemon's default filter (`warn`), only the two error events fire.
/// Run with `batchalign3 -v` (info) to see entry + per-branch outcomes, or
/// `-vv` (debug) for the original `alignment check` debug counts.
#[tracing::instrument(
    level = "info",
    skip_all,
    fields(utterance = %utterance.main.to_chat_string()),
)]
pub fn inject_morphosyntax(
    utterance: &mut Utterance,
    mors: Vec<Mor>,
    terminator: talkbank_model::Terminator,
    gra_relations: Vec<GrammaticalRelation>,
) -> Result<(), MisalignmentDiagnostic> {
    tracing::info!(
        mor_count = mors.len(),
        gra_count = gra_relations.len(),
        "inject_morphosyntax: enter",
    );
    if mors.is_empty() {
        tracing::info!("inject_morphosyntax: skip empty");
        return Ok(());
    }

    // Validate: MOR count must match alignable word count. The canonical
    // definition of N lives on `Utterance` in `talkbank-model` and applies
    // the CHAT manual's alignment rules — see
    // `talkbank-model::model::file::utterance::accessors::mor_alignable_word_count`.
    // The chat_words Vec is built only for diagnostic output on mismatch.
    // Typed counts: the model method returns `MorAlignableWordCount`,
    // a domain newtype distinct from `MorItemCount` / plain `usize`, so
    // any future refactor that swaps one for the other at the inject
    // boundary fails to compile rather than silently misaligning.
    let word_count = utterance.mor_alignable_word_count();
    let mor_count = talkbank_model::alignment::helpers::MorItemCount::new(mors.len());
    tracing::debug!(
        word_count = word_count.get(),
        mor_count = mor_count.get(),
        "inject_morphosyntax: alignment check"
    );
    if word_count.get() != mor_count.get() {
        // Materialize the actual word list only when we need it for
        // the diagnostic — the fast path avoids the allocation.
        let mut extracted = Vec::new();
        collect_utterance_content(
            &utterance.main.content.content,
            talkbank_model::alignment::helpers::TierDomain::Mor,
            &mut extracted,
        );
        // Mismatch class:
        //
        // The 1-to-1 invariant (CHAT extract `counts_for_tier` output N,
        // Stanza realignment produces N tokens, UD→Mor mapping keeps N
        // chunks) is violated. This is always a bug in extraction,
        // realignment, or mapping — never an expected divergence.
        // Return a typed diagnostic; the outer layer logs visibly and
        // absorbs at the file boundary.
        let utt_text = utterance.main.to_chat_string();
        tracing::warn!(
            word_count = word_count.get(),
            mor_count = mor_count.get(),
            utterance = %utt_text,
            "MOR count mismatch — returning MisalignmentDiagnostic"
        );
        return Err(MisalignmentDiagnostic {
            chat_words: extracted
                .iter()
                .map(|w| w.text.as_ref().to_string())
                .collect(),
            // The injector only sees the mapped Mors, not the raw Stanza
            // tokens — leaving empty here and letting the caller enrich
            // with Stanza-side context if available.
            stanza_tokens_after_mapping: Vec::new(),
            expected: word_count,
            actual: mor_count,
            suspected_class: MisalignmentClass::Unknown,
        });
    }

    // No-gra case: install MorTier alone. Some pipeline paths legitimately
    // produce a `%mor` without a paired `%gra` (e.g., utseg).
    if gra_relations.is_empty() {
        tracing::info!(mor_count = mors.len(), "inject_morphosyntax: mor-only");
        let mor_tier = MorTier::new_mor(mors, terminator);
        replace_or_add_tier(&mut utterance.dependent_tiers, DependentTier::Mor(mor_tier));
        return Ok(());
    }

    // Co-construct (MorTier, GraTier) via try_align_mor_gra so the chunk-
    // relation alignment is enforced at construction time. The terminator's
    // PUNCT relation is the last entry of gra_relations (per upstream
    // AppendTrailingPunct policy in build_gra_and_validate); pop it for the
    // typed terminator slot.
    let mut item_relations = gra_relations;
    let Some(terminator_relation) = item_relations.pop() else {
        // Unreachable: checked non-empty above.
        return Ok(());
    };
    let slot = talkbank_model::alignment::MorGraTerminatorSlot {
        terminator,
        relation: terminator_relation,
    };

    let (mor_tier, gra_tier) = match talkbank_model::alignment::try_align_mor_gra(
        mors,
        item_relations,
        slot,
        talkbank_model::Span::DUMMY,
    ) {
        Ok(pair) => pair,
        Err(talkbank_model::alignment::MorGraConstructionError::CountMismatch {
            mor_chunks,
            gra_relations,
        }) => {
            // dona@s bug class: upstream produced misaligned counts.
            // Surface as MisalignmentDiagnostic; the mor_chunks / gra_relations
            // counts are stuffed into the existing typed fields semantically
            // (they are usize counts, even if the field types were named for
            // the word/mor case originally).
            tracing::warn!(
                mor_chunks,
                gra_relations,
                utterance = %utterance.main.to_chat_string(),
                "%mor/%gra count mismatch — returning MisalignmentDiagnostic",
            );
            return Err(MisalignmentDiagnostic {
                chat_words: Vec::new(),
                stanza_tokens_after_mapping: Vec::new(),
                expected: talkbank_model::alignment::helpers::MorAlignableWordCount::new(
                    mor_chunks,
                ),
                actual: talkbank_model::alignment::helpers::MorItemCount::new(gra_relations),
                suspected_class: MisalignmentClass::MorGraCountMismatch,
            });
        }
    };

    let mor_chunks = mor_tier.count_chunks();
    let gra_relations_count = gra_tier.relations().len();
    tracing::info!(
        mor_chunks,
        gra_relations = gra_relations_count,
        "inject_morphosyntax: aligned",
    );
    replace_or_add_tier(&mut utterance.dependent_tiers, DependentTier::Mor(mor_tier));
    replace_or_add_tier(&mut utterance.dependent_tiers, DependentTier::Gra(gra_tier));

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use talkbank_model::model::{ChatFile, Line, WriteChat};
    use talkbank_parser::TreeSitterParser;

    const HELLO_CHAT: &str = "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tCHI Target_Child\n\
@ID:\teng|test|CHI||female|||Target_Child|||\n*CHI:\thello .\n@End\n";

    fn parse_chat(text: &str) -> ChatFile {
        let parser = TreeSitterParser::new().unwrap();
        parser.parse_chat_file(text).unwrap()
    }

    fn get_utterance(chat: &mut ChatFile, idx: usize) -> &mut Utterance {
        let mut utt_idx = 0;
        for line in &mut chat.lines {
            if let Line::Utterance(utt) = line {
                if utt_idx == idx {
                    return utt;
                }
                utt_idx += 1;
            }
        }
        panic!("Utterance {idx} not found");
    }

    #[test]
    fn test_replace_or_add_tier_user_defined() {
        use talkbank_model::model::{NonEmptyString, UserDefinedDependentTier};

        let mut tiers = smallvec::smallvec![];

        // Add %xtra
        let xtra1 = DependentTier::UserDefined(UserDefinedDependentTier {
            label: NonEmptyString::new("xtra").unwrap(),
            content: NonEmptyString::new("first").unwrap(),
            span: talkbank_model::Span::DUMMY,
        });
        replace_or_add_tier(&mut tiers, xtra1);
        assert_eq!(tiers.len(), 1);

        // Replace %xtra with new content
        let xtra2 = DependentTier::UserDefined(UserDefinedDependentTier {
            label: NonEmptyString::new("xtra").unwrap(),
            content: NonEmptyString::new("second").unwrap(),
            span: talkbank_model::Span::DUMMY,
        });
        replace_or_add_tier(&mut tiers, xtra2);
        assert_eq!(tiers.len(), 1); // replaced, not appended

        // Verify content was replaced
        if let DependentTier::UserDefined(ud) = &tiers[0] {
            assert_eq!(ud.content.as_ref(), "second");
        } else {
            panic!("Expected UserDefined tier");
        }

        // Add %xcod (different label) — should NOT replace %xtra
        let xcod = DependentTier::UserDefined(UserDefinedDependentTier {
            label: NonEmptyString::new("xcod").unwrap(),
            content: NonEmptyString::new("code").unwrap(),
            span: talkbank_model::Span::DUMMY,
        });
        replace_or_add_tier(&mut tiers, xcod);
        assert_eq!(tiers.len(), 2); // appended, not replaced
    }

    #[test]
    fn test_replace_or_add_tier_replaces_existing_wor() {
        use talkbank_model::model::WorTier;

        let mut tiers = smallvec::smallvec![DependentTier::Wor(WorTier::default())];
        let replacement = DependentTier::Wor(WorTier::from_words(vec![
            talkbank_model::model::Word::simple("hello"),
        ]));

        replace_or_add_tier(&mut tiers, replacement);

        assert_eq!(tiers.len(), 1);
        let DependentTier::Wor(wor) = &tiers[0] else {
            panic!("expected %wor tier");
        };
        assert_eq!(wor.words().count(), 1);
    }

    #[test]
    fn test_inject_empty_mors_is_noop() {
        let mut chat = parse_chat(HELLO_CHAT);
        let output_before = chat.to_chat_string();

        let utt = get_utterance(&mut chat, 0);
        inject_morphosyntax(
            utt,
            Vec::new(),
            talkbank_model::Terminator::Period {
                span: talkbank_model::Span::DUMMY,
            },
            Vec::new(),
        )
        .unwrap();

        let output_after = chat.to_chat_string();
        assert_eq!(output_before, output_after);
    }

    /// A Mor-count / main-tier word-count mismatch must surface as `Err` —
    /// warning-and-continuing hides mapping bugs silently.
    #[test]
    fn inject_morphosyntax_count_mismatch_returns_err() {
        use talkbank_model::model::dependent_tier::mor::{MorStem, MorWord, PosCategory};

        // eng_hello_female.cha's first utterance is `hello .` — one
        // alignable word on the main tier. Supply TWO Mor items to force
        // a mismatch (2 vs 1).
        let mut chat = parse_chat(HELLO_CHAT);
        let utt = get_utterance(&mut chat, 0);

        let mor = |pos: &str, lemma: &str| {
            Mor::new(MorWord::new(PosCategory::new(pos), MorStem::new(lemma)))
        };
        let too_many_mors = vec![mor("intj", "hello"), mor("intj", "extra")];

        let result = inject_morphosyntax(
            utt,
            too_many_mors,
            talkbank_model::Terminator::Period {
                span: talkbank_model::Span::DUMMY,
            },
            Vec::new(),
        );

        assert!(
            result.is_err(),
            "expected Err on count mismatch; got Ok (silent-skip regression)"
        );
        let diag = result.unwrap_err();
        // Typed diagnostic should carry the expected/actual counts and
        // the CHAT words, not a stringly message.
        assert_eq!(
            diag.expected.get(),
            1,
            "CHAT has 1 alignable word (hello); got {}",
            diag.expected
        );
        assert_eq!(
            diag.actual.get(),
            2,
            "test passed 2 Mor items; got {}",
            diag.actual
        );
        assert_eq!(
            diag.chat_words,
            vec!["hello".to_string()],
            "chat_words must reflect what CHAT extraction actually found"
        );
    }

    /// Mor/Gra count mismatch (mors_total_chunks + 1 != gra_relations.len())
    /// must surface as an Err — silent emission of a misaligned pair was the
    /// dona@s bug class.
    ///
    /// Setup: HELLO_CHAT's first utterance is `hello .` (1 alignable word).
    /// Supply 1 Mor (matching word count, so the upstream check passes) and
    /// gra_relations of length 1 — short by one (missing the terminator's
    /// PUNCT entry that AppendTrailingPunct should have produced upstream).
    /// MorTier with terminator counts 2 chunks; GraTier has 1 relation;
    /// inject must reject.
    #[test]
    fn inject_morphosyntax_gra_count_mismatch_returns_err() {
        use talkbank_model::model::dependent_tier::mor::{MorStem, MorWord, PosCategory};

        let mut chat = parse_chat(HELLO_CHAT);
        let utt = get_utterance(&mut chat, 0);

        let mor = |pos: &str, lemma: &str| {
            Mor::new(MorWord::new(PosCategory::new(pos), MorStem::new(lemma)))
        };
        let mors = vec![mor("intj", "hello")];
        // Upstream produced gras for items only — forgot to append the
        // terminator's PUNCT relation. Mor count_chunks() == 2 (1 item +
        // sidecar terminator), gras.len() == 1. Inject must reject.
        let item_only_gras = vec![GrammaticalRelation::new(1, 0, "ROOT")];

        let result = inject_morphosyntax(
            utt,
            mors,
            talkbank_model::Terminator::Period {
                span: talkbank_model::Span::DUMMY,
            },
            item_only_gras,
        );

        assert!(
            result.is_err(),
            "expected Err on mor/gra count mismatch; got Ok (silent-misalignment regression \
             — this is the dona@s bug class)"
        );
    }
}
