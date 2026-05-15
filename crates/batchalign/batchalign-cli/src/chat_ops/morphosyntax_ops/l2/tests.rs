//! Tests for the L2 code-switching morphotag module.

use super::*;
use talkbank_model::model::{FormType, LanguageCode};
use talkbank_model::validation::LanguageResolution;

use crate::chat_ops::nlp::UniversalPos;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn make_special_forms(
    specs: &[Option<&str>],
) -> Vec<(Option<FormType>, Option<LanguageResolution>)> {
    specs
        .iter()
        .map(|spec| {
            let lang_res = spec.map(|code| LanguageResolution::Single(LanguageCode::new(code)));
            (None, lang_res)
        })
        .collect()
}

fn make_words(texts: &[&str]) -> Vec<talkbank_model::ChatCleanedText> {
    // Test fixture path: `ChatCleanedText::test_unchecked` is the
    // explicit test escape hatch. The `test_` prefix and `#[doc(hidden)]`
    // flag it as fixture-only; production code must construct
    // `ChatCleanedText` via the AST-derived constructors.
    texts
        .iter()
        .map(|s| talkbank_model::ChatCleanedText::test_unchecked(*s))
        .collect()
}

fn ud(s: &str) -> UdDeprel {
    UdDeprel::new(s)
}

fn make_primary(
    deprel: &str,
    upos: UniversalPos,
    head_upos: UniversalPos,
) -> PrimaryStructuralInfo {
    PrimaryStructuralInfo {
        deprel: UdDeprel::new(deprel),
        upos: Some(upos),
        head: 1,
        dependent_deprels: vec![],
        head_upos: Some(head_upos),
    }
}

#[allow(
    dead_code,
    reason = "retained as a fixture helper for future L2 unit tests"
)]
fn make_ud_word(
    text: &str,
    lemma: &str,
    upos: UniversalPos,
    feats: Option<&str>,
) -> crate::chat_ops::nlp::UdWord {
    crate::chat_ops::nlp::UdWord {
        id: crate::chat_ops::nlp::UdId::Single(1),
        text: text.to_string(),
        lemma: lemma.to_string(),
        upos: crate::chat_ops::nlp::UdPunctable::Value(upos),
        xpos: None,
        feats: feats.map(|s| s.to_string()),
        head: 0,
        deprel: "root".to_string(),
        deps: None,
        misc: None,
    }
}

/// Build a minimal Mor for merge tests.
fn make_mor(pos: &str, lemma: &str) -> talkbank_model::model::dependent_tier::mor::Mor {
    use talkbank_model::model::dependent_tier::mor::{MorStem, MorWord, PosCategory};
    talkbank_model::model::dependent_tier::mor::Mor::new(MorWord::new(
        PosCategory::new(pos),
        MorStem::new(lemma),
    ))
}

fn make_deferred(line_idx: usize, word_idx: usize, lang: &str) -> L2DeferredPosition {
    L2DeferredPosition {
        line_idx,
        word_idx,
        target_lang: LanguageCode::new(lang),
        primary: PrimaryStructuralInfo {
            deprel: UdDeprel::new("flat"),
            upos: Some(UniversalPos::Noun),
            head: 0,
            dependent_deprels: vec![],
            head_upos: None,
        },
    }
}

fn make_word_cache(
    entries: &[(usize, usize, &str)],
) -> std::collections::HashMap<(usize, usize), talkbank_model::ChatCleanedText> {
    entries
        .iter()
        .map(|(l, w, t)| {
            (
                (*l, *w),
                talkbank_model::ChatCleanedText::test_unchecked(*t),
            )
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Deprel → POS constraint tests
// ---------------------------------------------------------------------------

#[test]
fn deprel_det_constrains_to_det() {
    assert_eq!(
        deprel_to_pos_constraint(&ud("det")),
        PosConstraint::Exact(UniversalPos::Det)
    );
}

#[test]
fn deprel_amod_constrains_to_adj() {
    assert_eq!(
        deprel_to_pos_constraint(&ud("amod")),
        PosConstraint::Exact(UniversalPos::Adj)
    );
}

#[test]
fn deprel_advmod_constrains_to_adv() {
    assert_eq!(
        deprel_to_pos_constraint(&ud("advmod")),
        PosConstraint::Exact(UniversalPos::Adv)
    );
}

#[test]
fn deprel_case_constrains_to_adp() {
    assert_eq!(
        deprel_to_pos_constraint(&ud("case")),
        PosConstraint::Exact(UniversalPos::Adp)
    );
}

#[test]
fn deprel_obj_constrains_to_noun_pron_propn() {
    assert_eq!(
        deprel_to_pos_constraint(&ud("obj")),
        PosConstraint::OneOf(vec![
            UniversalPos::Noun,
            UniversalPos::Pron,
            UniversalPos::Propn
        ])
    );
}

#[test]
fn deprel_nsubj_constrains_to_noun_pron_propn() {
    assert_eq!(
        deprel_to_pos_constraint(&ud("nsubj")),
        PosConstraint::OneOf(vec![
            UniversalPos::Noun,
            UniversalPos::Pron,
            UniversalPos::Propn
        ])
    );
}

#[test]
fn deprel_root_constrains_to_verb_noun_adj() {
    let c = deprel_to_pos_constraint(&ud("root"));
    assert!(c.contains(&UniversalPos::Verb));
    assert!(c.contains(&UniversalPos::Noun));
    assert!(c.contains(&UniversalPos::Adj));
    assert!(!c.contains(&UniversalPos::Adv));
}

#[test]
fn deprel_flat_is_unconstrained() {
    let c = deprel_to_pos_constraint(&ud("flat"));
    assert!(c.contains(&UniversalPos::Propn));
    assert!(c.contains(&UniversalPos::Noun));
    assert!(c.contains(&UniversalPos::Adj));
    assert!(c.contains(&UniversalPos::Adv));
    assert!(c.contains(&UniversalPos::Det));
    assert!(c.contains(&UniversalPos::Verb));
    assert!(c.contains(&UniversalPos::Pron));
}

#[test]
fn deprel_subtype_stripped() {
    assert_eq!(
        deprel_to_pos_constraint(&ud("obl:arg")),
        deprel_to_pos_constraint(&ud("obl"))
    );
}

#[test]
fn unknown_deprel_unconstrained() {
    assert_eq!(
        deprel_to_pos_constraint(&ud("xyzzy")),
        PosConstraint::Unconstrained
    );
}

#[test]
fn constraint_contains_exact() {
    let c = PosConstraint::Exact(UniversalPos::Adv);
    assert!(c.contains(&UniversalPos::Adv));
    assert!(!c.contains(&UniversalPos::Noun));
}

#[test]
fn constraint_contains_unconstrained() {
    let c = PosConstraint::Unconstrained;
    assert!(c.contains(&UniversalPos::Verb));
    assert!(c.contains(&UniversalPos::Adv));
}

#[test]
fn most_likely_exact() {
    assert_eq!(
        PosConstraint::Exact(UniversalPos::Det).most_likely(),
        Some(UniversalPos::Det)
    );
}

#[test]
fn most_likely_oneof_returns_first() {
    let c = PosConstraint::OneOf(vec![UniversalPos::Noun, UniversalPos::Pron]);
    assert_eq!(c.most_likely(), Some(UniversalPos::Noun));
}

#[test]
fn most_likely_unconstrained_is_none() {
    assert_eq!(PosConstraint::Unconstrained.most_likely(), None);
}

// ---------------------------------------------------------------------------
// Dependent refinement tests
// ---------------------------------------------------------------------------

#[test]
fn det_dependent_narrows_to_noun() {
    let base = deprel_to_pos_constraint(&ud("flat"));
    let refined = refine_with_dependents(&base, &[ud("det")]);
    assert!(refined.contains(&UniversalPos::Noun));
    assert!(refined.contains(&UniversalPos::Propn));
    assert!(!refined.contains(&UniversalPos::Adj));
    assert!(!refined.contains(&UniversalPos::Adv));
}

#[test]
fn nsubj_dependent_narrows_to_verb_adj() {
    let base = deprel_to_pos_constraint(&ud("root"));
    let refined = refine_with_dependents(&base, &[ud("nsubj")]);
    assert!(refined.contains(&UniversalPos::Verb));
    assert!(refined.contains(&UniversalPos::Adj));
    assert!(!refined.contains(&UniversalPos::Noun));
}

#[test]
fn no_dependents_preserves_constraint() {
    let base = deprel_to_pos_constraint(&ud("advmod"));
    let refined = refine_with_dependents(&base, &[]);
    assert_eq!(base, refined);
}

// ---------------------------------------------------------------------------
// GRA deprel inference tests
// ---------------------------------------------------------------------------

#[test]
fn infer_advmod_when_adv_head_adj() {
    assert_eq!(
        infer_deprel_from_pos(UniversalPos::Adv, Some(UniversalPos::Adj), false),
        Some(UdDeprel::new("advmod"))
    );
}

#[test]
fn infer_advmod_when_adv_head_verb() {
    assert_eq!(
        infer_deprel_from_pos(UniversalPos::Adv, Some(UniversalPos::Verb), false),
        Some(UdDeprel::new("advmod"))
    );
}

#[test]
fn infer_amod_when_adj_head_noun() {
    assert_eq!(
        infer_deprel_from_pos(UniversalPos::Adj, Some(UniversalPos::Noun), false),
        Some(UdDeprel::new("amod"))
    );
}

#[test]
fn infer_det_when_det_head_noun() {
    assert_eq!(
        infer_deprel_from_pos(UniversalPos::Det, Some(UniversalPos::Noun), false),
        Some(UdDeprel::new("det"))
    );
}

#[test]
fn infer_obj_when_noun_head_verb_no_case() {
    assert_eq!(
        infer_deprel_from_pos(UniversalPos::Noun, Some(UniversalPos::Verb), false),
        Some(UdDeprel::new("obj"))
    );
}

#[test]
fn infer_obl_when_noun_head_verb_with_case() {
    assert_eq!(
        infer_deprel_from_pos(UniversalPos::Noun, Some(UniversalPos::Verb), true),
        Some(UdDeprel::new("obl"))
    );
}

#[test]
fn infer_nmod_when_noun_head_noun() {
    assert_eq!(
        infer_deprel_from_pos(UniversalPos::Noun, Some(UniversalPos::Noun), false),
        Some(UdDeprel::new("nmod"))
    );
}

#[test]
fn no_inference_for_uncommon_combination() {
    assert_eq!(
        infer_deprel_from_pos(UniversalPos::Verb, Some(UniversalPos::Noun), false),
        None
    );
}

#[test]
fn no_inference_when_head_unknown() {
    assert_eq!(infer_deprel_from_pos(UniversalPos::Adv, None, false), None);
}

// ---------------------------------------------------------------------------
// Contiguous span grouping tests
// ---------------------------------------------------------------------------

#[test]
fn single_at_s_word_single_span() {
    let sf = make_special_forms(&[None, None, Some("spa"), None]);
    let words = make_words(&["I", "like", "tienda", "."]);
    let spans = group_l2_spans(&sf, &words);
    assert_eq!(spans.len(), 1);
    assert_eq!(spans[0].word_indices, vec![2]);
    assert_eq!(spans[0].target_lang.as_str(), "spa");
    assert_eq!(spans[0].words, vec!["tienda"]);
}

#[test]
fn contiguous_same_lang_merged() {
    let sf = make_special_forms(&[None, None, Some("spa"), Some("spa"), None]);
    let words = make_words(&["we", "about", "los", "niños", "."]);
    let spans = group_l2_spans(&sf, &words);
    assert_eq!(spans.len(), 1);
    assert_eq!(spans[0].word_indices, vec![2, 3]);
    assert_eq!(spans[0].words, vec!["los", "niños"]);
}

#[test]
fn non_contiguous_separate_spans() {
    let sf = make_special_forms(&[Some("spa"), None, Some("spa")]);
    let words = make_words(&["hola", "and", "adiós"]);
    let spans = group_l2_spans(&sf, &words);
    assert_eq!(spans.len(), 2);
}

#[test]
fn mixed_languages_separate_spans() {
    let sf = make_special_forms(&[Some("spa"), Some("fra")]);
    let words = make_words(&["hola", "bonjour"]);
    let spans = group_l2_spans(&sf, &words);
    assert_eq!(spans.len(), 2);
    assert_eq!(spans[0].target_lang.as_str(), "spa");
    assert_eq!(spans[1].target_lang.as_str(), "fra");
}

#[test]
fn multiple_lang_uses_first() {
    let sf = vec![(
        None,
        Some(LanguageResolution::Multiple(vec![
            LanguageCode::new("eng"),
            LanguageCode::new("spa"),
        ])),
    )];
    let words = make_words(&["ripiado"]);
    let spans = group_l2_spans(&sf, &words);
    assert_eq!(spans.len(), 1);
    assert_eq!(spans[0].target_lang.as_str(), "eng");
}

#[test]
fn ambiguous_lang_uses_first() {
    let sf = vec![(
        None,
        Some(LanguageResolution::Ambiguous(vec![
            LanguageCode::new("eng"),
            LanguageCode::new("spa"),
        ])),
    )];
    let words = make_words(&["bajo"]);
    let spans = group_l2_spans(&sf, &words);
    assert_eq!(spans.len(), 1);
    assert_eq!(spans[0].target_lang.as_str(), "eng");
}

#[test]
fn unresolved_skipped() {
    let sf = vec![(None, Some(LanguageResolution::Unresolved))];
    let words = make_words(&["mystery"]);
    let spans = group_l2_spans(&sf, &words);
    assert!(spans.is_empty());
}

#[test]
fn no_at_s_words_empty() {
    let sf = make_special_forms(&[None, None, None]);
    let words = make_words(&["hello", "world", "."]);
    let spans = group_l2_spans(&sf, &words);
    assert!(spans.is_empty());
}

#[test]
fn all_at_s_same_lang_one_span() {
    let sf = make_special_forms(&[Some("eng"), Some("eng"), Some("eng")]);
    let words = make_words(&["full", "English", "breakfast"]);
    let spans = group_l2_spans(&sf, &words);
    assert_eq!(spans.len(), 1);
    assert_eq!(spans[0].word_indices, vec![0, 1, 2]);
}

#[test]
fn contiguous_then_gap_then_contiguous() {
    let sf = make_special_forms(&[Some("spa"), Some("spa"), None, Some("spa")]);
    let words = make_words(&["los", "niños", "and", "casa"]);
    let spans = group_l2_spans(&sf, &words);
    assert_eq!(spans.len(), 2);
    assert_eq!(spans[0].word_indices, vec![0, 1]);
    assert_eq!(spans[1].word_indices, vec![3]);
}

// ---------------------------------------------------------------------------
// Structural merge tests
// ---------------------------------------------------------------------------

#[test]
fn merge_secondary_within_constraint_uses_secondary() {
    let primary = make_primary("advmod", UniversalPos::Adv, UniversalPos::Adj);
    let pos = resolve_merged_pos(&primary, Some(UniversalPos::Adv));
    assert_eq!(pos, UniversalPos::Adv);
}

#[test]
fn secondary_noun_overrides_advmod_constraint() {
    let primary = make_primary("advmod", UniversalPos::Adv, UniversalPos::Adj);
    let pos = resolve_merged_pos(&primary, Some(UniversalPos::Noun));
    assert_eq!(pos, UniversalPos::Noun);
}

#[test]
fn merge_uses_most_likely_when_both_outside() {
    let primary = make_primary("obj", UniversalPos::Adv, UniversalPos::Verb);
    let pos = resolve_merged_pos(&primary, Some(UniversalPos::Verb));
    assert_eq!(pos, UniversalPos::Noun);
}

#[test]
fn merge_upgrades_flat_deprel() {
    let primary = PrimaryStructuralInfo {
        deprel: UdDeprel::new("flat"),
        upos: Some(UniversalPos::Adj),
        head: 4,
        dependent_deprels: vec![],
        head_upos: Some(UniversalPos::Adj),
    };
    let mor = make_mor("adv", "mucho");
    let result = merge_primary_secondary(
        &primary,
        mor,
        Vec::new(),
        &LanguageCode::new("spa"),
        L2Attachment::InternalRoot,
    );
    assert_eq!(result.mor.main.pos.as_str(), "adv");
    assert_eq!(result.corrected_deprel, Some(UdDeprel::new("advmod")));
}

#[test]
fn merge_does_not_upgrade_non_flat() {
    let primary = make_primary("advmod", UniversalPos::Adv, UniversalPos::Adj);
    let mor = make_mor("adv", "mucho");
    let result = merge_primary_secondary(
        &primary,
        mor,
        Vec::new(),
        &LanguageCode::new("spa"),
        L2Attachment::InternalRoot,
    );
    assert_eq!(result.corrected_deprel, None);
}

#[test]
fn merge_flat_to_obj_noun_head_verb() {
    let primary = PrimaryStructuralInfo {
        deprel: UdDeprel::new("flat"),
        upos: Some(UniversalPos::Noun),
        head: 2,
        dependent_deprels: vec![],
        head_upos: Some(UniversalPos::Verb),
    };
    let mor = make_mor("noun", "tienda");
    let result = merge_primary_secondary(
        &primary,
        mor,
        Vec::new(),
        &LanguageCode::new("spa"),
        L2Attachment::InternalRoot,
    );
    assert_eq!(result.mor.main.pos.as_str(), "noun");
    assert_eq!(result.corrected_deprel, Some(UdDeprel::new("obj")));
    assert_eq!(result.mor.main.lemma.as_str(), "tienda");
}

#[test]
fn merge_flat_to_obl_noun_head_verb_with_case() {
    let primary = PrimaryStructuralInfo {
        deprel: UdDeprel::new("flat"),
        upos: Some(UniversalPos::Noun),
        head: 2,
        dependent_deprels: vec![UdDeprel::new("case")],
        head_upos: Some(UniversalPos::Verb),
    };
    let mor = make_mor("noun", "tienda");
    let result = merge_primary_secondary(
        &primary,
        mor,
        Vec::new(),
        &LanguageCode::new("spa"),
        L2Attachment::InternalRoot,
    );
    assert_eq!(result.corrected_deprel, Some(UdDeprel::new("obl")));
}

#[test]
fn merge_with_det_dependent_narrows_pos() {
    let primary = PrimaryStructuralInfo {
        deprel: UdDeprel::new("flat"),
        upos: Some(UniversalPos::Propn),
        head: 2,
        dependent_deprels: vec![UdDeprel::new("det")],
        head_upos: Some(UniversalPos::Verb),
    };
    let pos = resolve_merged_pos(&primary, Some(UniversalPos::Noun));
    assert_eq!(pos, UniversalPos::Noun);
}

#[test]
fn merge_function_word_overrides_structural_constraint() {
    let primary = PrimaryStructuralInfo {
        deprel: UdDeprel::new("obl"),
        upos: Some(UniversalPos::Propn),
        head: 2,
        dependent_deprels: vec![],
        head_upos: Some(UniversalPos::Verb),
    };
    let pos = resolve_merged_pos(&primary, Some(UniversalPos::Det));
    assert_eq!(pos, UniversalPos::Det);
}

#[test]
fn copula_dependent_rejects_verb_for_predicate_nominal() {
    let primary = PrimaryStructuralInfo {
        deprel: UdDeprel::new("root"),
        upos: Some(UniversalPos::Noun),
        head: 0,
        dependent_deprels: vec![UdDeprel::new("cop"), UdDeprel::new("nsubj")],
        head_upos: None,
    };
    let pos = resolve_merged_pos(&primary, Some(UniversalPos::Verb));
    assert_eq!(pos, UniversalPos::Noun);
}

#[test]
fn copula_dependent_allows_adj_for_predicate() {
    let primary = PrimaryStructuralInfo {
        deprel: UdDeprel::new("root"),
        upos: Some(UniversalPos::Adj),
        head: 0,
        dependent_deprels: vec![UdDeprel::new("cop")],
        head_upos: None,
    };
    let pos = resolve_merged_pos(&primary, Some(UniversalPos::Adj));
    assert_eq!(pos, UniversalPos::Adj);
}

#[test]
fn no_copula_allows_verb_at_root() {
    let primary = PrimaryStructuralInfo {
        deprel: UdDeprel::new("root"),
        upos: Some(UniversalPos::Verb),
        head: 0,
        dependent_deprels: vec![UdDeprel::new("nsubj")],
        head_upos: None,
    };
    let pos = resolve_merged_pos(&primary, Some(UniversalPos::Verb));
    assert_eq!(pos, UniversalPos::Verb);
}

#[test]
fn secondary_noun_overrides_wrong_advmod_deprel() {
    let primary = make_primary("advmod", UniversalPos::Adv, UniversalPos::Adj);
    let pos = resolve_merged_pos(&primary, Some(UniversalPos::Noun));
    assert_eq!(pos, UniversalPos::Noun);
}

#[test]
fn secondary_noun_overrides_wrong_amod_deprel() {
    let primary = make_primary("amod", UniversalPos::Adj, UniversalPos::Noun);
    let pos = resolve_merged_pos(&primary, Some(UniversalPos::Noun));
    assert_eq!(pos, UniversalPos::Noun);
}

#[test]
fn secondary_adv_still_accepted_when_deprel_matches() {
    let primary = make_primary("advmod", UniversalPos::Adv, UniversalPos::Adj);
    let pos = resolve_merged_pos(&primary, Some(UniversalPos::Adv));
    assert_eq!(pos, UniversalPos::Adv);
}

// ---------------------------------------------------------------------------
// Dispatch span grouping tests
// ---------------------------------------------------------------------------

#[test]
fn dispatch_spans_contiguous_same_utterance_merged() {
    let deferred = vec![make_deferred(5, 3, "spa"), make_deferred(5, 4, "spa")];
    let cache = make_word_cache(&[(5, 3, "los"), (5, 4, "niños")]);
    let spans = group_deferred_into_dispatch_spans(&deferred, &cache);
    assert_eq!(spans.len(), 1);
    assert_eq!(spans[0].words, vec!["los", "niños"]);
    assert_eq!(spans[0].global_indices, vec![0, 1]);
}

#[test]
fn dispatch_spans_non_contiguous_same_utterance_separate() {
    let deferred = vec![make_deferred(5, 2, "spa"), make_deferred(5, 6, "spa")];
    let cache = make_word_cache(&[(5, 2, "tienda"), (5, 6, "casa")]);
    let spans = group_deferred_into_dispatch_spans(&deferred, &cache);
    assert_eq!(spans.len(), 2);
}

#[test]
fn dispatch_spans_different_utterances_separate() {
    let deferred = vec![make_deferred(3, 5, "eng"), make_deferred(7, 2, "eng")];
    let cache = make_word_cache(&[(3, 5, "film"), (7, 2, "studies")]);
    let spans = group_deferred_into_dispatch_spans(&deferred, &cache);
    assert_eq!(spans.len(), 2);
}

#[test]
fn dispatch_spans_mixed_languages_separate() {
    let deferred = vec![make_deferred(5, 2, "spa"), make_deferred(5, 3, "fra")];
    let cache = make_word_cache(&[(5, 2, "tienda"), (5, 3, "bonjour")]);
    let spans = group_deferred_into_dispatch_spans(&deferred, &cache);
    assert_eq!(spans.len(), 2);
    assert_eq!(spans[0].target_lang.as_str(), "spa");
    assert_eq!(spans[1].target_lang.as_str(), "fra");
}

#[test]
fn dispatch_spans_three_word_contiguous() {
    let deferred = vec![
        make_deferred(10, 4, "eng"),
        make_deferred(10, 5, "eng"),
        make_deferred(10, 6, "eng"),
    ];
    let cache = make_word_cache(&[(10, 4, "full"), (10, 5, "English"), (10, 6, "breakfast")]);
    let spans = group_deferred_into_dispatch_spans(&deferred, &cache);
    assert_eq!(spans.len(), 1);
    assert_eq!(spans[0].words, vec!["full", "English", "breakfast"]);
    assert_eq!(spans[0].global_indices, vec![0, 1, 2]);
}

// ---------------------------------------------------------------------------
// Phrasal-verb recognition tests (2026-04-15)
//
// Stanza returns `compound:prt` for true verb-particle constructions (wake
// up, give up, figure out). Without sentence-level context in the L2 merge,
// the primary language's deprel constraint (e.g. `advmod` for German parsing
// of foreign @s words) can reject the secondary's VERB tag, and Priority 3
// (closed-class) blindly trusts ADP for the particle. These tests lock in
// the corrected behavior:
//
//   1. Particle — UPOS Part (CHAT `part|up`), GRA deprel `compound:prt`.
//   2. Head — UPOS Verb, overriding primary constraint.
//   3. Non-phrasal ADP (`case`/`advmod`) is unaffected.
// ---------------------------------------------------------------------------

/// Build a UD sentence for "wake up" — VERB root + ADP compound:prt.
///
/// Mirrors exactly what the Stanza English pipeline returns in free-tokenize
/// mode (verified by `scripts/l2-eval/probe_phrasal_verbs.py`).
fn ud_sentence_wake_up() -> crate::chat_ops::nlp::UdSentence {
    crate::chat_ops::nlp::UdSentence {
        words: vec![
            crate::chat_ops::nlp::UdWord {
                id: crate::chat_ops::nlp::UdId::Single(1),
                text: "wake".into(),
                lemma: "wake".into(),
                upos: crate::chat_ops::nlp::UdPunctable::Value(UniversalPos::Verb),
                xpos: None,
                feats: Some("Mood=Imp|VerbForm=Fin".into()),
                head: 0,
                deprel: "root".into(),
                deps: None,
                misc: None,
            },
            crate::chat_ops::nlp::UdWord {
                id: crate::chat_ops::nlp::UdId::Single(2),
                text: "up".into(),
                lemma: "up".into(),
                upos: crate::chat_ops::nlp::UdPunctable::Value(UniversalPos::Adp),
                xpos: None,
                feats: None,
                head: 1,
                deprel: "compound:prt".into(),
                deps: None,
                misc: None,
            },
        ],
    }
}

#[test]
fn phrasal_verb_particle_is_promoted_to_part() {
    // `up` with deprel compound:prt is a verb particle in CHAT convention,
    // not an ADP. Resolve must return Part even though secondary says Adp
    // (which would otherwise trigger the closed-class Priority 3 branch).
    let primary = make_primary("advmod", UniversalPos::Adv, UniversalPos::Verb);
    let sentence = ud_sentence_wake_up();
    let ctx = SecondaryUdContext {
        sentence: &sentence,
        word_position: 1, // "up"
    };
    let pos = resolve_merged_pos_with_context(&primary, Some(UniversalPos::Adp), Some(&ctx));
    assert_eq!(pos, UniversalPos::Part);
}

#[test]
fn phrasal_verb_head_promotes_verb_over_primary_advmod_constraint() {
    // Primary German parser tagged `wake` as advmod; secondary English says
    // VERB with a compound:prt dependent. Promote to VERB despite the
    // constraint mismatch.
    let primary = make_primary("advmod", UniversalPos::Adv, UniversalPos::Verb);
    let sentence = ud_sentence_wake_up();
    let ctx = SecondaryUdContext {
        sentence: &sentence,
        word_position: 0, // "wake"
    };
    let pos = resolve_merged_pos_with_context(&primary, Some(UniversalPos::Verb), Some(&ctx));
    assert_eq!(pos, UniversalPos::Verb);
}

#[test]
fn non_phrasal_adp_case_stays_adp() {
    // Regression: a normal case-marking ADP (no compound:prt relation) must
    // not be promoted to Part.
    let sentence = crate::chat_ops::nlp::UdSentence {
        words: vec![crate::chat_ops::nlp::UdWord {
            id: crate::chat_ops::nlp::UdId::Single(1),
            text: "of".into(),
            lemma: "of".into(),
            upos: crate::chat_ops::nlp::UdPunctable::Value(UniversalPos::Adp),
            xpos: None,
            feats: None,
            head: 2,
            deprel: "case".into(),
            deps: None,
            misc: None,
        }],
    };
    let primary = make_primary("case", UniversalPos::Adp, UniversalPos::Noun);
    let ctx = SecondaryUdContext {
        sentence: &sentence,
        word_position: 0,
    };
    let pos = resolve_merged_pos_with_context(&primary, Some(UniversalPos::Adp), Some(&ctx));
    assert_eq!(pos, UniversalPos::Adp);
}

#[test]
fn phrasal_verb_head_requires_verb_secondary_upos() {
    // Safety: if secondary gave something other than VERB for the head
    // (shouldn't happen but could with model variation), the promotion
    // short-circuits and falls through to the existing priority chain.
    let primary = make_primary("advmod", UniversalPos::Adv, UniversalPos::Verb);
    let sentence = ud_sentence_wake_up();
    let ctx = SecondaryUdContext {
        sentence: &sentence,
        word_position: 0,
    };
    // Secondary says NOUN (not VERB) — Priority 4 (NOUN override) kicks in,
    // returning NOUN, not Verb.
    let pos = resolve_merged_pos_with_context(&primary, Some(UniversalPos::Noun), Some(&ctx));
    assert_eq!(pos, UniversalPos::Noun);
}

#[test]
fn phrasal_verb_particle_merge_sets_compound_prt_deprel() {
    // End-to-end: merge_primary_secondary on the particle should set POS to
    // part AND corrected_deprel to compound:prt so the GRA tier reflects
    // the phrasal-verb structure.
    let primary = make_primary("advmod", UniversalPos::Adv, UniversalPos::Verb);
    let mor = make_mor("adp", "up");
    let sentence = ud_sentence_wake_up();
    let ctx = SecondaryUdContext {
        sentence: &sentence,
        word_position: 1,
    };
    let result = merge_primary_secondary_with_context(
        &primary,
        mor,
        Vec::new(),
        &LanguageCode::new("eng"),
        L2Attachment::InternalRoot,
        Some(&ctx),
    );
    assert_eq!(result.mor.main.pos.as_str(), "part");
    assert_eq!(result.corrected_deprel, Some(UdDeprel::new("compound:prt")));
}
