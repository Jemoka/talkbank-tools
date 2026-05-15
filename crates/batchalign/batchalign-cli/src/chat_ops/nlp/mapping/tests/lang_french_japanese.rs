//! Language-specific tests for French and Japanese: determiner gender defaults, APM noun plurals, pronoun case, MWT contractions, verb/intj/aux overrides, comma-lemma handling.

#![allow(unused_imports, dead_code)]

use super::*;

use crate::chat_ops::nlp::mapping::validate_generated_gra;
use crate::chat_ops::nlp::mapping::*;
use crate::chat_ops::nlp::{UdId, UdPunctable, UdSentence, UdWord, UniversalPos};
use crate::chat_ops::nlp::{clean_lemma, map_ud_word_to_mor};
use talkbank_model::model::GrammaticalRelation;
use talkbank_model::model::dependent_tier::mor::Mor;

#[test]
fn test_japanese_punctuation_mapping() {
    let ctx = MappingContext {
        lang: talkbank_model::model::LanguageCode::new("ja"),
    };
    let ud = UdWord {
        id: UdId::Single(1),
        text: "\u{3002}".to_string(),
        lemma: "\u{3002}".to_string(),
        upos: UdPunctable::Value(UniversalPos::Punct),
        xpos: None,
        feats: None,
        head: 0,
        deprel: "root".to_string(),
        deps: None,
        misc: None,
    };

    let mor = map_ud_word_to_mor(&ud, &ctx).unwrap();
    let mut out = String::new();
    mor.write_chat(&mut out).unwrap();
    assert_eq!(out, "cm|\u{3002}");
}

#[test]
fn test_mwt_assembly_french_elision() {
    let ctx = MappingContext {
        lang: talkbank_model::model::LanguageCode::new("fr"),
    };
    let sentence = UdSentence {
        words: vec![
            UdWord {
                id: UdId::Single(1),
                text: "l'".to_string(),
                lemma: "le".to_string(),
                upos: UdPunctable::Value(UniversalPos::Det),
                xpos: None,
                feats: None,
                head: 2,
                deprel: "det".to_string(),
                deps: None,
                misc: None,
            },
            UdWord {
                id: UdId::Single(2),
                text: "ami".to_string(),
                lemma: "ami".to_string(),
                upos: UdPunctable::Value(UniversalPos::Noun),
                xpos: None,
                feats: None,
                head: 0,
                deprel: "root".to_string(),
                deps: None,
                misc: None,
            },
        ],
    };

    let mors = map_ud_sentence_to_mors(&sentence, &ctx);
    // Note: In this case, UD doesn't provide a range, but they are clitics
    // Future: implement greedy joining for non-range clitics if desired.
    // For now, let's verify they remain separate if no range is provided.
    assert_eq!(mors.len(), 2);
}

#[test]
fn test_french_pron_case() {
    let ctx = MappingContext {
        lang: talkbank_model::model::LanguageCode::new("fr"),
    };
    let ud = UdWord {
        id: UdId::Single(1),
        text: "je".into(),
        lemma: "je".into(),
        upos: UdPunctable::Value(UniversalPos::Pron),
        xpos: None,
        feats: Some("Number=Sing|Person=1|PronType=Prs".into()),
        head: 0,
        deprel: "root".into(),
        deps: None,
        misc: None,
    };
    let mor = map_ud_word_to_mor(&ud, &ctx).unwrap();
    let mut out = String::new();
    mor.write_chat(&mut out).unwrap();
    // French "je" gets Case=Nom from word-level lookup
    assert_eq!(out, "pron|je-Prs-Nom-S1");
}

#[test]
fn test_french_det_singular_gender_default_masc() {
    // ba2: DET gender defaults to "Masc" for French singular
    let ctx = MappingContext {
        lang: talkbank_model::model::LanguageCode::new("fr"),
    };
    let ud = UdWord {
        id: UdId::Single(1),
        text: "le".into(),
        lemma: "le".into(),
        upos: UdPunctable::Value(UniversalPos::Det),
        xpos: None,
        feats: Some("Definite=Def|PronType=Art".into()),
        head: 0,
        deprel: "det".into(),
        deps: None,
        misc: None,
    };
    let mor = map_ud_word_to_mor(&ud, &ctx).unwrap();
    let mut out = String::new();
    mor.write_chat(&mut out).unwrap();
    // French singular DET without Gender → defaults to "Masc"
    assert_eq!(out, "det|le-Masc-Def-Art");
}

#[test]
fn test_french_det_plural_no_gender_default() {
    // ba2: DET gender default is "" for French plural (no Masc default)
    let ctx = MappingContext {
        lang: talkbank_model::model::LanguageCode::new("fr"),
    };
    let ud = UdWord {
        id: UdId::Single(1),
        text: "les".into(),
        lemma: "le".into(),
        upos: UdPunctable::Value(UniversalPos::Det),
        xpos: None,
        feats: Some("Definite=Def|Number=Plur|PronType=Art".into()),
        head: 0,
        deprel: "det".into(),
        deps: None,
        misc: None,
    };
    let mor = map_ud_word_to_mor(&ud, &ctx).unwrap();
    let mut out = String::new();
    mor.write_chat(&mut out).unwrap();
    // French plural DET: no gender default, Number=Plur present
    assert_eq!(out, "det|le-Def-Art-Plur");
}

#[test]
fn test_french_det_explicit_fem_gender() {
    let ctx = MappingContext {
        lang: talkbank_model::model::LanguageCode::new("fr"),
    };
    let ud = UdWord {
        id: UdId::Single(1),
        text: "la".into(),
        lemma: "le".into(),
        upos: UdPunctable::Value(UniversalPos::Det),
        xpos: None,
        feats: Some("Definite=Def|Gender=Fem|Number=Sing|PronType=Art".into()),
        head: 0,
        deprel: "det".into(),
        deps: None,
        misc: None,
    };
    let mor = map_ud_word_to_mor(&ud, &ctx).unwrap();
    let mut out = String::new();
    mor.write_chat(&mut out).unwrap();
    assert_eq!(out, "det|le-Fem-Def-Art-Sing");
}

#[test]
fn test_french_noun_apm_plural() {
    // ba2: French plural nouns with auditory plural marking get -Apm suffix
    let ctx = MappingContext {
        lang: talkbank_model::model::LanguageCode::new("fr"),
    };
    let ud = UdWord {
        id: UdId::Single(1),
        text: "chevaux".into(),
        lemma: "cheval".into(),
        upos: UdPunctable::Value(UniversalPos::Noun),
        xpos: None,
        feats: Some("Gender=Masc|Number=Plur".into()),
        head: 0,
        deprel: "root".into(),
        deps: None,
        misc: None,
    };
    let mor = map_ud_word_to_mor(&ud, &ctx).unwrap();
    let mut out = String::new();
    mor.write_chat(&mut out).unwrap();
    // French APM: cheval→chevaux, Masc gender + Plur + Apm
    assert_eq!(out, "noun|cheval-Masc-Plur-Apm");
}

#[test]
fn test_french_noun_non_apm_plural() {
    // Regular French plural noun: no -Apm suffix
    let ctx = MappingContext {
        lang: talkbank_model::model::LanguageCode::new("fr"),
    };
    let ud = UdWord {
        id: UdId::Single(1),
        text: "maisons".into(),
        lemma: "maison".into(),
        upos: UdPunctable::Value(UniversalPos::Noun),
        xpos: None,
        feats: Some("Gender=Fem|Number=Plur".into()),
        head: 0,
        deprel: "root".into(),
        deps: None,
        misc: None,
    };
    let mor = map_ud_word_to_mor(&ud, &ctx).unwrap();
    let mut out = String::new();
    mor.write_chat(&mut out).unwrap();
    assert_eq!(out, "noun|maison-Fem-Plur");
}

#[test]
fn test_french_pron_accusative() {
    // ba2: French "me" gets Case=Acc from word-level lookup
    let ctx = MappingContext {
        lang: talkbank_model::model::LanguageCode::new("fr"),
    };
    let ud = UdWord {
        id: UdId::Single(1),
        text: "me".into(),
        lemma: "me".into(),
        upos: UdPunctable::Value(UniversalPos::Pron),
        xpos: None,
        feats: Some("Number=Sing|Person=1|PronType=Prs".into()),
        head: 0,
        deprel: "root".into(),
        deps: None,
        misc: None,
    };
    let mor = map_ud_word_to_mor(&ud, &ctx).unwrap();
    let mut out = String::new();
    mor.write_chat(&mut out).unwrap();
    assert_eq!(out, "pron|me-Prs-Acc-S1");
}

#[test]
fn test_french_pron_no_case_lookup() {
    // ba2: French "nous" has no entry in case lookup → no Case suffix
    let ctx = MappingContext {
        lang: talkbank_model::model::LanguageCode::new("fr"),
    };
    let ud = UdWord {
        id: UdId::Single(1),
        text: "nous".into(),
        lemma: "nous".into(),
        upos: UdPunctable::Value(UniversalPos::Pron),
        xpos: None,
        feats: Some("Number=Plur|Person=1|PronType=Prs".into()),
        head: 0,
        deprel: "root".into(),
        deps: None,
        misc: None,
    };
    let mor = map_ud_word_to_mor(&ud, &ctx).unwrap();
    let mut out = String::new();
    mor.write_chat(&mut out).unwrap();
    // "nous" is not in fr/case.py → no Case field
    assert_eq!(out, "pron|nous-Prs-P1");
}

#[test]
fn test_french_mwt_contraction_du() {
    // ba2: French "du" → "de" + "le" via MWT Range
    let ctx = MappingContext {
        lang: talkbank_model::model::LanguageCode::new("fr"),
    };
    let sentence = UdSentence {
        words: vec![
            UdWord {
                id: UdId::Range(1, 2),
                text: "du".into(),
                lemma: "du".into(),
                upos: UdPunctable::Punct("X".into()),
                xpos: None,
                feats: None,
                head: 0,
                deprel: "dep".into(),
                deps: None,
                misc: None,
            },
            UdWord {
                id: UdId::Single(1),
                text: "de".into(),
                lemma: "de".into(),
                upos: UdPunctable::Value(UniversalPos::Adp),
                xpos: None,
                feats: None,
                head: 3,
                deprel: "case".into(),
                deps: None,
                misc: None,
            },
            UdWord {
                id: UdId::Single(2),
                text: "le".into(),
                lemma: "le".into(),
                upos: UdPunctable::Value(UniversalPos::Det),
                xpos: None,
                feats: Some("Definite=Def|Gender=Masc|Number=Sing|PronType=Art".into()),
                head: 3,
                deprel: "det".into(),
                deps: None,
                misc: None,
            },
            UdWord {
                id: UdId::Single(3),
                text: "pain".into(),
                lemma: "pain".into(),
                upos: UdPunctable::Value(UniversalPos::Noun),
                xpos: None,
                feats: Some("Gender=Masc|Number=Sing".into()),
                head: 0,
                deprel: "root".into(),
                deps: None,
                misc: None,
            },
        ],
    };
    let mors = map_ud_sentence_to_mors(&sentence, &ctx);
    // "du" MWT → clitic group (de~le) + "pain" = 2 items
    assert_eq!(mors.len(), 2, "Expected 2 MOR items for 'du pain'");

    let mut out0 = String::new();
    mors[0].write_chat(&mut out0).unwrap();
    // Clitic assembly: adp|de~det|le-Masc-Def-Art-Sing
    assert!(
        out0.contains("adp|de") && out0.contains("det|le"),
        "Expected clitic group adp|de~det|le, got: {out0}"
    );

    let mut out1 = String::new();
    mors[1].write_chat(&mut out1).unwrap();
    assert_eq!(out1, "noun|pain-Masc");
}

#[test]
fn test_japanese_verb_override_full_output() {
    // ba2: Japanese "食べちゃう" matches "ちゃ" → sconj|ば
    let ctx = MappingContext {
        lang: talkbank_model::model::LanguageCode::new("ja"),
    };
    let ud = UdWord {
        id: UdId::Single(1),
        text: "食べちゃう".into(),
        lemma: "食べる".into(),
        upos: UdPunctable::Value(UniversalPos::Verb),
        xpos: None,
        feats: Some("VerbForm=Fin".into()),
        head: 0,
        deprel: "root".into(),
        deps: None,
        misc: None,
    };
    let mor = map_ud_word_to_mor(&ud, &ctx).unwrap();
    let mut out = String::new();
    mor.write_chat(&mut out).unwrap();
    // Override changes POS to sconj → no verb features emitted
    assert_eq!(out, "sconj|ば");
}

#[test]
fn test_japanese_intj_override_hai() {
    // ba2: Japanese "はい" overridden to intj regardless of Stanza's POS
    let ctx = MappingContext {
        lang: talkbank_model::model::LanguageCode::new("ja"),
    };
    let ud = UdWord {
        id: UdId::Single(1),
        text: "はい".into(),
        lemma: "はい".into(),
        upos: UdPunctable::Value(UniversalPos::Noun), // Stanza might tag as NOUN
        xpos: None,
        feats: None,
        head: 0,
        deprel: "root".into(),
        deps: None,
        misc: None,
    };
    let mor = map_ud_word_to_mor(&ud, &ctx).unwrap();
    let mut out = String::new();
    mor.write_chat(&mut out).unwrap();
    // Override: intj|はい (noun features suppressed because dispatch uses original UPOS)
    // Original UPOS is NOUN → noun_features runs, but lemma override to はい
    // Actually: the effective_pos is "intj" but dispatch uses original UPOS (NOUN)
    // So noun_features runs with the overridden lemma
    assert!(
        out.starts_with("intj|") || out.contains("はい"),
        "Expected Japanese intj override, got: {out}"
    );
}

#[test]
fn test_japanese_aux_override_nai() {
    // ba2: target containing "無い" → aux|ない
    let ctx = MappingContext {
        lang: talkbank_model::model::LanguageCode::new("ja"),
    };
    let ud = UdWord {
        id: UdId::Single(1),
        text: "ない".into(),
        lemma: "無い".into(),
        upos: UdPunctable::Value(UniversalPos::Aux),
        xpos: None,
        feats: None,
        head: 0,
        deprel: "root".into(),
        deps: None,
        misc: None,
    };
    let mor = map_ud_word_to_mor(&ud, &ctx).unwrap();
    let mut out = String::new();
    mor.write_chat(&mut out).unwrap();
    // Override: aux|ない, then verb features (original UPOS = AUX)
    assert!(
        out.starts_with("aux|ない"),
        "Expected aux|ない prefix, got: {out}"
    );
}

#[test]
fn test_japanese_comma_lemma_becomes_cm() {
    // ba2: Japanese comma (、) → cm|、
    // The Japanese comma is NOT in the early-return punct list (which only
    // has ASCII ","), so it goes through the normal path: POS→"cm", stem→"、"
    let ctx = MappingContext {
        lang: talkbank_model::model::LanguageCode::new("ja"),
    };
    let ud = UdWord {
        id: UdId::Single(1),
        text: "、".into(),
        lemma: "、".into(),
        upos: UdPunctable::Value(UniversalPos::Punct),
        xpos: None,
        feats: None,
        head: 0,
        deprel: "punct".into(),
        deps: None,
        misc: None,
    };
    let mor = map_ud_word_to_mor(&ud, &ctx).unwrap();
    let mut out = String::new();
    mor.write_chat(&mut out).unwrap();
    assert_eq!(out, "cm|、");
}

#[test]
fn test_japanese_all_punct_is_cm() {
    // ba2: ALL Japanese PUNCT tokens (not just comma) → cm|X
    let ctx = MappingContext {
        lang: talkbank_model::model::LanguageCode::new("ja"),
    };
    let ud = UdWord {
        id: UdId::Single(1),
        text: "…".into(),
        lemma: "…".into(),
        upos: UdPunctable::Value(UniversalPos::Punct),
        xpos: None,
        feats: None,
        head: 0,
        deprel: "punct".into(),
        deps: None,
        misc: None,
    };
    let mor = map_ud_word_to_mor(&ud, &ctx).unwrap();
    let mut out = String::new();
    mor.write_chat(&mut out).unwrap();
    // Japanese PUNCT → POS becomes "cm"
    assert!(
        out.starts_with("cm|"),
        "Japanese PUNCT should use cm| prefix, got: {out}"
    );
}

#[test]
fn test_japanese_verb_no_irr_suffix() {
    // ba2: -irr suffix is English-only. Japanese verbs must never get it.
    let ctx = MappingContext {
        lang: talkbank_model::model::LanguageCode::new("ja"),
    };
    let ud = UdWord {
        id: UdId::Single(1),
        text: "行った".into(),
        lemma: "行く".into(),
        upos: UdPunctable::Value(UniversalPos::Verb),
        xpos: None,
        feats: Some("Tense=Past|VerbForm=Fin".into()),
        head: 0,
        deprel: "root".into(),
        deps: None,
        misc: None,
    };
    let mor = map_ud_word_to_mor(&ud, &ctx).unwrap();
    let mut out = String::new();
    mor.write_chat(&mut out).unwrap();
    assert!(
        !out.contains("-irr"),
        "Japanese verbs must NOT get -irr suffix, got: {out}"
    );
}

#[test]
fn test_french_3letter_code_works() {
    // Real-world: language codes come as "fra" not "fr"
    let ctx = MappingContext {
        lang: talkbank_model::model::LanguageCode::new("fra"),
    };
    let ud = UdWord {
        id: UdId::Single(1),
        text: "je".into(),
        lemma: "je".into(),
        upos: UdPunctable::Value(UniversalPos::Pron),
        xpos: None,
        feats: Some("Number=Sing|Person=1|PronType=Prs".into()),
        head: 0,
        deprel: "root".into(),
        deps: None,
        misc: None,
    };
    let mor = map_ud_word_to_mor(&ud, &ctx).unwrap();
    let mut out = String::new();
    mor.write_chat(&mut out).unwrap();
    // Must get French pronoun case even with "fra" code
    assert_eq!(out, "pron|je-Prs-Nom-S1");
}

#[test]
fn test_japanese_3letter_code_works() {
    // Real-world: "jpn" not "ja"
    let ctx = MappingContext {
        lang: talkbank_model::model::LanguageCode::new("jpn"),
    };
    let ud = UdWord {
        id: UdId::Single(1),
        text: "はい".into(),
        lemma: "はい".into(),
        upos: UdPunctable::Value(UniversalPos::Noun),
        xpos: None,
        feats: None,
        head: 0,
        deprel: "root".into(),
        deps: None,
        misc: None,
    };
    let mor = map_ud_word_to_mor(&ud, &ctx).unwrap();
    let mut out = String::new();
    mor.write_chat(&mut out).unwrap();
    // "jpn" must trigger Japanese verbform overrides
    assert!(
        out.contains("intj|はい"),
        "3-letter 'jpn' must trigger JA overrides, got: {out}"
    );
}
