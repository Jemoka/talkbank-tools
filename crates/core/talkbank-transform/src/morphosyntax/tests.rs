//! Unit tests for the morphosyntax module's classifier, outcome
//! conversion, UD type round-trips, and small text-sanitization
//! helpers.

#![cfg(test)]

use super::*;
use talkbank_model::alignment::helpers::{MorAlignableWordCount, MorItemCount};
use talkbank_model::model::ChatFile;
use talkbank_model::model::{LanguageCode, SpeakerCode, Utterance};
use talkbank_parser::TreeSitterParser;

use crate::inject::{MisalignmentClass, MisalignmentDiagnostic};
use crate::parse::parse_lenient;

fn parse_chat(text: &str) -> ChatFile {
    let parser = TreeSitterParser::new().expect("parser init");
    parser.parse_chat_file(text).unwrap()
}

fn validate_morphosyntax(chat: &mut ChatFile) {
    use talkbank_model::ParseValidateOptions;
    let opts = ParseValidateOptions::default().with_alignment();
    if let Err(e) = talkbank_model::validate_chat_file_with_options(chat, &opts) {
        panic!("Morphosyntax validation failed: {:#?}", e);
    }
}

fn one_utterance(main_tier: &str) -> String {
    format!(
        "@UTF8\n\
         @Begin\n\
         @Languages:\teng\n\
         @Participants:\tCHI Target_Child\n\
         @ID:\teng|test|CHI||female|||Target_Child|||\n\
         *CHI:\t{main_tier}\n\
         @End\n"
    )
}

fn first_utterance(chat: &ChatFile) -> &Utterance {
    for line in &chat.lines {
        if let talkbank_model::model::Line::Utterance(u) = line {
            return u;
        }
    }
    panic!("no utterance in chat file")
}

#[test]
fn classify_filler_only() {
    let mut chat = parse_chat(&one_utterance("&-hmm ."));
    assert_eq!(
        classify_not_applicable(first_utterance(&chat)),
        NotApplicableReason::FillerOnly,
    );
    validate_morphosyntax(&mut chat);
}

#[test]
fn classify_multiple_fillers() {
    let chat = parse_chat(&one_utterance("&-hmm &-hmm ."));
    assert_eq!(
        classify_not_applicable(first_utterance(&chat)),
        NotApplicableReason::FillerOnly,
    );
}

#[test]
fn classify_fragment_only() {
    let chat = parse_chat(&one_utterance("&+le ."));
    assert_eq!(
        classify_not_applicable(first_utterance(&chat)),
        NotApplicableReason::FragmentOnly,
    );
}

#[test]
fn classify_nonword_only() {
    let chat = parse_chat(&one_utterance("&~ach ."));
    assert_eq!(
        classify_not_applicable(first_utterance(&chat)),
        NotApplicableReason::NonwordOnly,
    );
}

#[test]
fn classify_untranscribed_only() {
    let chat = parse_chat(&one_utterance("xxx ."));
    assert_eq!(
        classify_not_applicable(first_utterance(&chat)),
        NotApplicableReason::UntranscribedOnly,
    );
}

#[test]
fn classify_mixed_nonlinguistic() {
    let chat = parse_chat(&one_utterance("&-hmm &+le ."));
    assert_eq!(
        classify_not_applicable(first_utterance(&chat)),
        NotApplicableReason::MixedNonLinguistic,
    );
}

#[test]
fn to_decision_record_aligned_is_none() {
    let outcome = MorOutcome {
        line_idx: 5,
        speaker: SpeakerCode::new("CHI"),
        kind: MorOutcomeKind::Aligned { n_words: 3 },
    };
    assert!(outcome.to_decision_record().is_none());
}

#[test]
fn to_decision_record_not_applicable_has_reason() {
    let outcome = MorOutcome {
        line_idx: 5,
        speaker: SpeakerCode::new("CHI"),
        kind: MorOutcomeKind::NotApplicable {
            reason: NotApplicableReason::FillerOnly,
        },
    };
    let d = outcome.to_decision_record().unwrap();
    assert!(matches!(
        d.strategy,
        crate::decisions::DecisionStrategy::Morphosyntax(
            crate::decisions::MorphosyntaxStrategy::NotApplicable
        )
    ));
    assert_eq!(d.reason, "reason=filler_only");
    assert!(!d.needs_review);
}

#[test]
fn to_decision_record_misalignment_has_diagnostic() {
    let outcome = MorOutcome {
        line_idx: 5,
        speaker: SpeakerCode::new("CHI"),
        kind: MorOutcomeKind::MisalignmentBug(MisalignmentDiagnostic {
            chat_words: vec!["hello".into(), "world".into()],
            stanza_tokens_after_mapping: vec!["hello".into()],
            expected: MorAlignableWordCount::new(2),
            actual: MorItemCount::new(1),
            suspected_class: MisalignmentClass::TerminatorFilterBug,
        }),
    };
    let d = outcome.to_decision_record().unwrap();
    assert!(matches!(
        d.strategy,
        crate::decisions::DecisionStrategy::Morphosyntax(
            crate::decisions::MorphosyntaxStrategy::MisalignmentBug
        )
    ));
    assert!(d.needs_review);
    assert!(d.reason.contains("class=terminator_filter_bug"));
    assert!(d.reason.contains("expected=2"));
    assert!(d.reason.contains("actual=1"));
}

#[test]
fn universal_pos_round_trips_to_chat_name_and_back() {
    for v in [
        UniversalPos::Adj,
        UniversalPos::Adp,
        UniversalPos::Adv,
        UniversalPos::Aux,
        UniversalPos::Cconj,
        UniversalPos::Det,
        UniversalPos::Intj,
        UniversalPos::Noun,
        UniversalPos::Num,
        UniversalPos::Part,
        UniversalPos::Pron,
        UniversalPos::Propn,
        UniversalPos::Punct,
        UniversalPos::Sconj,
        UniversalPos::Verb,
    ] {
        let name = v.to_chat_pos_name();
        assert_eq!(UniversalPos::from_pos_name(name), Some(v));
    }
    assert_eq!(UniversalPos::Sym.to_chat_pos_name(), "x");
    assert_eq!(UniversalPos::X.to_chat_pos_name(), "x");
    assert_eq!(UniversalPos::from_pos_name("x"), Some(UniversalPos::X));
    assert_eq!(UniversalPos::from_pos_name("sym"), Some(UniversalPos::X));
}

#[test]
fn universal_pos_accepts_case_insensitive_names() {
    assert_eq!(
        UniversalPos::from_pos_name("NOUN"),
        Some(UniversalPos::Noun)
    );
    assert_eq!(
        UniversalPos::from_pos_name("noun"),
        Some(UniversalPos::Noun)
    );
    assert_eq!(
        UniversalPos::from_pos_name("Noun"),
        Some(UniversalPos::Noun)
    );
    assert_eq!(UniversalPos::from_pos_name("notreal"), None);
}

#[test]
fn stanza_language_support_matches_expected_examples() {
    for code in ["eng", "spa", "fra", "deu", "zho", "jpn", "rus", "ara"] {
        assert!(is_stanza_supported(&LanguageCode::new(code)));
    }
    for code in ["que", "jam", "nan", "taq", "und", "xmm", "jav", "wuu"] {
        assert!(!is_stanza_supported(&LanguageCode::new(code)));
    }
    assert!(is_stanza_supported(&LanguageCode::new("yue")));
    assert!(is_stanza_supported(&LanguageCode::new("cmn")));
    for code in ["ben", "kan", "mal", "msa", "tgl", "ltz"] {
        assert!(!is_stanza_supported(&LanguageCode::new(code)));
    }
}

#[test]
fn dep_rel_roundtrips_known_variants() {
    for rel in [
        "root",
        "nsubj",
        "nsubj:pass",
        "obj",
        "aux",
        "aux:pass",
        "cop",
        "case",
        "nmod:poss",
        "det",
        "cc",
        "conj",
        "compound",
        "compound:prt",
        "amod",
        "advmod",
        "punct",
        "discourse",
        "mark",
        "expl",
    ] {
        assert_eq!(DepRel::parse(rel).as_str(), rel);
    }
}

#[test]
fn dep_rel_preserves_unknown_values() {
    let rel = DepRel::parse("orphan");
    assert_eq!(rel, DepRel::Other("orphan".to_string()));
    assert_eq!(rel.as_str(), "orphan");
}

#[test]
fn verb_form_roundtrips() {
    for value in ["Fin", "Part", "Ger", "Inf", "Sup", "Conv", "Vnoun"] {
        assert_eq!(VerbForm::parse(value).as_str(), value);
    }
}

#[test]
fn has_verb_form_fin_matches_expected_cases() {
    assert!(has_verb_form_fin(Some(
        "Mood=Ind|Number=Sing|Person=3|Tense=Pres|VerbForm=Fin"
    )));
    assert!(has_verb_form_fin(Some("VerbForm=Fin")));
    assert!(!has_verb_form_fin(Some("Tense=Pres|VerbForm=Part")));
    assert!(!has_verb_form_fin(None));
}

#[test]
fn has_key_value_matches_exact_pairs() {
    let feats = Some("Mood=Ind|Number=Sing|Person=3|VerbForm=Fin");
    assert!(has_key_value(feats, "VerbForm", "Fin"));
    assert!(has_key_value(feats, "Number", "Sing"));
    assert!(has_key_value(feats, "Person", "3"));
    assert!(!has_key_value(feats, "Tense", "Past"));
    assert!(!has_key_value(None, "VerbForm", "Fin"));
}

#[test]
fn canonical_ud_feat_bundles_are_alphabetical() {
    for bundle in [FINITE_COPULA_PRES_3SG, PRESENT_PARTICIPLE] {
        let keys: Vec<&str> = bundle
            .split('|')
            .map(|pair| pair.split('=').next().expect("feature key"))
            .collect();
        let mut sorted = keys.clone();
        sorted.sort();
        assert_eq!(keys, sorted, "bundle {bundle:?} is not alphabetized");
    }
}

#[test]
fn bogus_lemma_detection_matches_expected_cases() {
    assert!(is_bogus_lemma("hello", "."));
    assert!(is_bogus_lemma("world", ","));
    assert!(is_bogus_lemma("cat", "–"));
    assert!(!is_bogus_lemma("hello", "hello"));
    assert!(!is_bogus_lemma("hello", ""));
    assert!(!is_bogus_lemma(".", "."));
    assert!(!is_bogus_lemma(",", "--"));
    assert!(!is_bogus_lemma("running", "run"));
    assert!(!is_bogus_lemma("cats", "cat"));
}

#[test]
fn validate_and_clean_fixes_pad_deprel_and_bogus_lemma() {
    let mut word = UdWord {
        id: UdId::Single(1),
        text: "hello".to_string(),
        lemma: ".".to_string(),
        upos: UdPunctable::Value(UniversalPos::Intj),
        xpos: None,
        feats: None,
        head: 0,
        deprel: "<pad>".to_string(),
        deps: None,
        misc: None,
    };

    validate_and_clean(&mut word);

    assert_eq!(word.lemma, "hello");
    assert_eq!(word.deprel, "dep");
}

#[test]
fn sanitize_mor_text_replaces_structural_separators() {
    assert_eq!(sanitize_mor_text("foo|bar"), "foo_bar");
    assert_eq!(sanitize_mor_text("a#b-c&d$e~f"), "a_b_c_d_e_f");
}

#[test]
fn sanitize_mor_text_strips_whitespace() {
    assert_eq!(sanitize_mor_text("ふ す"), "ふす");
    assert_eq!(sanitize_mor_text(" hello world "), "helloworld");
    assert_eq!(sanitize_mor_text("a\tb\nc"), "abc");
}

#[test]
fn sanitize_mor_text_handles_combined_issues() {
    assert_eq!(sanitize_mor_text("foo | bar"), "foo_bar");
    assert_eq!(sanitize_mor_text("ふ す#test"), "ふす_test");
}

#[test]
fn sanitize_mor_text_passthroughs_clean_text() {
    assert_eq!(sanitize_mor_text("hello"), "hello");
    assert_eq!(sanitize_mor_text("ふす"), "ふす");
}

/// CA-prosody terminators on the main tier must be substituted with
/// Period in the morphotag payload so synthesized `%mor` is valid CHAT.
#[test]
fn ca_arrow_terminator_must_normalize_to_period_in_morphotag_payload() {
    use crate::morphosyntax::payload::{collect_payloads, declared_languages};
    use crate::morphosyntax::types::MultilingualPolicy;
    let parser = TreeSitterParser::new().unwrap();
    let chat = "@UTF8\n\
                @Begin\n\
                @Languages:\teng\n\
                @Participants:\tPAR Participant\n\
                @ID:\teng|test|PAR|||||Participant|||\n\
                *PAR:\tyes →\n\
                @End\n";
    let (chat_file, _) = parse_lenient(&parser, chat);

    let primary = LanguageCode::new("eng");
    let langs = declared_languages(&chat_file, &primary);
    let items =
        collect_payloads(&chat_file, &primary, &langs, MultilingualPolicy::ProcessAll).batch_items;

    assert_eq!(
        items.len(),
        1,
        "expected one batch item for the single utterance"
    );
    let (_, _, ref item, _) = items[0];

    assert!(
        matches!(item.terminator, talkbank_model::Terminator::Period { .. }),
        "got: {:?}",
        item.terminator,
    );
}

/// `@s` in a `cat,spa` document with `primary_lang="eng"` (the
/// dispatch-layer fallback fabricated by `infer_batched.rs:79` when
/// `WorkerLanguage::Unspecified`) must resolve to the file's secondary
/// declared language `spa`, not to the bogus `eng`.
///
/// Pre-2026-05-02 this resolved to `Single("eng")` because
/// `collect_payloads` computed `tier_language = utt_lang.or(Some(primary_lang))`,
/// skipping the `declared_languages.first()` step that `utterance_lang`
/// already used. Combined with the resolver's then-fabricated
/// `Single(tier_lang)` sentinel, every `@s` token in a batch-default-eng
/// run produced fake `Single("eng")` resolutions that routed L2
/// secondary dispatch through the wrong Stanza pipeline.
#[test]
fn collect_payloads_resolves_at_s_against_file_languages_not_batch_default() {
    use crate::parse::parse_lenient;

    let parser = TreeSitterParser::new().unwrap();
    let chat = include_str!("../../../../../resources/fixtures/cat_spa_dona_at_s.cha");
    let (chat_file, _) = parse_lenient(&parser, chat);

    let primary = LanguageCode::new("eng"); // simulated batch default
    let langs = declared_languages(&chat_file, &primary);
    let items =
        collect_payloads(&chat_file, &primary, &langs, MultilingualPolicy::ProcessAll).batch_items;

    assert_eq!(items.len(), 1);
    let (_, _, ref item, _) = items[0];
    assert_eq!(
        item.lang.as_str(),
        "cat",
        "dispatch lang must come from file header"
    );

    let dona_idx = item
        .words
        .iter()
        .position(|w| w.as_ref() == "dona")
        .expect("payload must include the dona word");

    let (_, ref resolved) = item.special_forms[dona_idx];
    let resolved = resolved
        .as_ref()
        .expect("dona@s must produce a language resolution");
    let resolved_langs: Vec<&str> = resolved.languages().iter().map(|c| c.as_str()).collect();
    assert_eq!(
        resolved_langs,
        vec!["spa"],
        "@s on cat-tier with declared [cat, spa] must resolve to spa, never eng",
    );
}

#[test]
fn lang2_normalizes_common_codes() {
    assert_eq!(lang2("eng"), "en");
    assert_eq!(lang2("fra"), "fr");
    assert_eq!(lang2("jpn"), "ja");
    assert_eq!(lang2("deu"), "de");
    assert_eq!(lang2("heb"), "he");
    assert_eq!(lang2("en"), "en");
}
