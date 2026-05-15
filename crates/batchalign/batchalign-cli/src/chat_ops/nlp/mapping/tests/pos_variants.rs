//! Per-POS mapping variants (adp, intj, cconj, sconj, propn, verb, pron, det, adj, noun) plus utility-helper coverage (clean_lemma strip, lang2, irr_suffix, multivalue UD features, French pron case).

#![allow(unused_imports, dead_code)]

use super::*;

use crate::chat_ops::nlp::mapping::validate_generated_gra;
use crate::chat_ops::nlp::mapping::*;
use crate::chat_ops::nlp::{UdId, UdPunctable, UdSentence, UdWord, UniversalPos};
use crate::chat_ops::nlp::{clean_lemma, map_ud_word_to_mor};
use talkbank_model::model::GrammaticalRelation;
use talkbank_model::model::dependent_tier::mor::Mor;

#[test]
fn test_pron_mapping_no_subcategory() {
    // Python uses "pron|lemma" with feature suffixes, NOT xpos-based subcategories
    let ctx = MappingContext {
        lang: talkbank_model::model::LanguageCode::new("en"),
    };
    let ud = UdWord {
        id: UdId::Single(1),
        text: "I".to_string(),
        lemma: "I".to_string(),
        upos: UdPunctable::Value(UniversalPos::Pron),
        xpos: Some("PRP-sub".to_string()),
        feats: None,
        head: 0,
        deprel: "root".to_string(),
        deps: None,
        misc: None,
    };

    let mor = map_ud_word_to_mor(&ud, &ctx).unwrap();
    let mut out = String::new();
    mor.write_chat(&mut out).unwrap();
    // Python: pron|I-Int-S1 (PronType default "Int", Number default "S", Person default "1")
    assert_eq!(out, "pron|I-Int-S1");
}

#[test]
fn test_pos_adp_mapping() {
    let ctx = MappingContext {
        lang: talkbank_model::model::LanguageCode::new("en"),
    };
    let ud = UdWord {
        id: UdId::Single(1),
        text: "in".into(),
        lemma: "in".into(),
        upos: UdPunctable::Value(UniversalPos::Adp),
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
    // Python: adp|in (was "prep|in")
    assert_eq!(out, "adp|in");
}

#[test]
fn test_pos_intj_mapping() {
    let ctx = MappingContext {
        lang: talkbank_model::model::LanguageCode::new("en"),
    };
    let ud = UdWord {
        id: UdId::Single(1),
        text: "wow".into(),
        lemma: "wow".into(),
        upos: UdPunctable::Value(UniversalPos::Intj),
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
    // Python: intj|wow (was "co|wow")
    assert_eq!(out, "intj|wow");
}

#[test]
fn test_pos_cconj_mapping() {
    let ctx = MappingContext {
        lang: talkbank_model::model::LanguageCode::new("en"),
    };
    let ud = UdWord {
        id: UdId::Single(1),
        text: "and".into(),
        lemma: "and".into(),
        upos: UdPunctable::Value(UniversalPos::Cconj),
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
    // Python: cconj|and (was "x|and")
    assert_eq!(out, "cconj|and");
}

#[test]
fn test_pos_sconj_mapping() {
    let ctx = MappingContext {
        lang: talkbank_model::model::LanguageCode::new("en"),
    };
    let ud = UdWord {
        id: UdId::Single(1),
        text: "because".into(),
        lemma: "because".into(),
        upos: UdPunctable::Value(UniversalPos::Sconj),
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
    assert_eq!(out, "sconj|because");
}

#[test]
fn test_pos_propn_mapping() {
    let ctx = MappingContext {
        lang: talkbank_model::model::LanguageCode::new("en"),
    };
    let ud = UdWord {
        id: UdId::Single(1),
        text: "London".into(),
        lemma: "London".into(),
        upos: UdPunctable::Value(UniversalPos::Propn),
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
    // Python: propn|London (was "n:prop|London")
    assert_eq!(out, "propn|London");
}

#[test]
fn test_verb_full_features() {
    let ctx = MappingContext {
        lang: talkbank_model::model::LanguageCode::new("en"),
    };
    let ud = UdWord {
        id: UdId::Single(1),
        text: "walks".into(),
        lemma: "walk".into(),
        upos: UdPunctable::Value(UniversalPos::Verb),
        xpos: None,
        feats: Some("Mood=Ind|Number=Sing|Person=3|Tense=Pres|VerbForm=Fin".into()),
        head: 0,
        deprel: "root".into(),
        deps: None,
        misc: None,
    };
    let mor = map_ud_word_to_mor(&ud, &ctx).unwrap();
    let mut out = String::new();
    mor.write_chat(&mut out).unwrap();
    // VerbForm=Fin, Mood=Ind, Tense=Pres, Number=S, Person=3
    assert_eq!(out, "verb|walk-Fin-Ind-Pres-S3");
}

#[test]
fn test_verb_irregular_past() {
    let ctx = MappingContext {
        lang: talkbank_model::model::LanguageCode::new("en"),
    };
    let ud = UdWord {
        id: UdId::Single(1),
        text: "went".into(),
        lemma: "go".into(),
        upos: UdPunctable::Value(UniversalPos::Verb),
        xpos: None,
        feats: Some("Tense=Past|VerbForm=Fin|Number=Sing|Person=3".into()),
        head: 0,
        deprel: "root".into(),
        deps: None,
        misc: None,
    };
    let mor = map_ud_word_to_mor(&ud, &ctx).unwrap();
    let mut out = String::new();
    mor.write_chat(&mut out).unwrap();
    // "went" is irregular past of "go" → "-irr" suffix
    assert_eq!(out, "verb|go-Fin-Past-S3-irr");
}

#[test]
fn test_pron_with_features() {
    let ctx = MappingContext {
        lang: talkbank_model::model::LanguageCode::new("en"),
    };
    let ud = UdWord {
        id: UdId::Single(1),
        text: "I".into(),
        lemma: "I".into(),
        upos: UdPunctable::Value(UniversalPos::Pron),
        xpos: None,
        feats: Some("Case=Nom|Number=Sing|Person=1|PronType=Prs".into()),
        head: 0,
        deprel: "root".into(),
        deps: None,
        misc: None,
    };
    let mor = map_ud_word_to_mor(&ud, &ctx).unwrap();
    let mut out = String::new();
    mor.write_chat(&mut out).unwrap();
    assert_eq!(out, "pron|I-Prs-Nom-S1");
}

#[test]
fn test_pron_that_no_number() {
    let ctx = MappingContext {
        lang: talkbank_model::model::LanguageCode::new("en"),
    };
    let ud = UdWord {
        id: UdId::Single(1),
        text: "that".into(),
        lemma: "that".into(),
        upos: UdPunctable::Value(UniversalPos::Pron),
        xpos: None,
        feats: Some("PronType=Rel".into()),
        head: 0,
        deprel: "root".into(),
        deps: None,
        misc: None,
    };
    let mor = map_ud_word_to_mor(&ud, &ctx).unwrap();
    let mut out = String::new();
    mor.write_chat(&mut out).unwrap();
    // "that" and "who" get no NumberPerson string
    assert_eq!(out, "pron|that-Rel");
}

#[test]
fn test_det_default_definite() {
    let ctx = MappingContext {
        lang: talkbank_model::model::LanguageCode::new("en"),
    };
    let ud = UdWord {
        id: UdId::Single(1),
        text: "the".into(),
        lemma: "the".into(),
        upos: UdPunctable::Value(UniversalPos::Det),
        xpos: None,
        feats: None,
        head: 0,
        deprel: "det".into(),
        deps: None,
        misc: None,
    };
    let mor = map_ud_word_to_mor(&ud, &ctx).unwrap();
    let mut out = String::new();
    mor.write_chat(&mut out).unwrap();
    // Definite defaults to "Def"
    assert_eq!(out, "det|the-Def");
}

#[test]
fn test_det_with_article() {
    let ctx = MappingContext {
        lang: talkbank_model::model::LanguageCode::new("en"),
    };
    let ud = UdWord {
        id: UdId::Single(1),
        text: "the".into(),
        lemma: "the".into(),
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
    assert_eq!(out, "det|the-Def-Art");
}

#[test]
fn test_adj_default_degree() {
    let ctx = MappingContext {
        lang: talkbank_model::model::LanguageCode::new("en"),
    };
    let ud = UdWord {
        id: UdId::Single(1),
        text: "big".into(),
        lemma: "big".into(),
        upos: UdPunctable::Value(UniversalPos::Adj),
        xpos: None,
        feats: Some("Degree=Pos".into()),
        head: 0,
        deprel: "root".into(),
        deps: None,
        misc: None,
    };
    let mor = map_ud_word_to_mor(&ud, &ctx).unwrap();
    let mut out = String::new();
    mor.write_chat(&mut out).unwrap();
    // Degree "Pos" is cleared to empty
    assert_eq!(out, "adj|big-S1");
}

#[test]
fn test_adj_comparative() {
    let ctx = MappingContext {
        lang: talkbank_model::model::LanguageCode::new("en"),
    };
    let ud = UdWord {
        id: UdId::Single(1),
        text: "bigger".into(),
        lemma: "big".into(),
        upos: UdPunctable::Value(UniversalPos::Adj),
        xpos: None,
        feats: Some("Degree=Cmp".into()),
        head: 0,
        deprel: "root".into(),
        deps: None,
        misc: None,
    };
    let mor = map_ud_word_to_mor(&ud, &ctx).unwrap();
    let mut out = String::new();
    mor.write_chat(&mut out).unwrap();
    assert_eq!(out, "adj|big-Cmp-S1");
}

#[test]
fn test_noun_obj_accusative() {
    let ctx = MappingContext {
        lang: talkbank_model::model::LanguageCode::new("en"),
    };
    let ud = UdWord {
        id: UdId::Single(1),
        text: "dog".into(),
        lemma: "dog".into(),
        upos: UdPunctable::Value(UniversalPos::Noun),
        xpos: None,
        feats: None,
        head: 2,
        deprel: "obj".into(),
        deps: None,
        misc: None,
    };
    let mor = map_ud_word_to_mor(&ud, &ctx).unwrap();
    let mut out = String::new();
    mor.write_chat(&mut out).unwrap();
    // deprel "obj" without Case → "Acc"
    assert_eq!(out, "noun|dog-Acc");
}

#[test]
fn test_comma_lemma_early_return() {
    let ctx = MappingContext {
        lang: talkbank_model::model::LanguageCode::new("en"),
    };
    let ud = UdWord {
        id: UdId::Single(1),
        text: ",".into(),
        lemma: ",".into(),
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
    assert_eq!(out, "cm|cm");
}

#[test]
fn test_clean_lemma_strips_special_chars() {
    // Verify clean_lemma handles various problematic lemmas
    let (cleaned, unknown) = clean_lemma("$test.", "test");
    assert_eq!(cleaned, "test");
    assert!(!unknown);

    let (cleaned, unknown) = clean_lemma("0word", "0word");
    assert_eq!(cleaned, "word");
    assert!(unknown);
}

#[test]
fn test_english_irregular_verb_suffix() {
    // "wrote" is an irregular past of "write"
    assert!(is_irregular("write", "wrote"));
    assert!(is_irregular("go", "went"));
    assert!(!is_irregular("walk", "walked"));
}

#[test]
fn test_lang2_normalization() {
    assert_eq!(lang2("eng"), "en");
    assert_eq!(lang2("fra"), "fr");
    assert_eq!(lang2("jpn"), "ja");
    assert_eq!(lang2("en"), "en");
    assert_eq!(lang2("fr"), "fr");
    assert_eq!(lang2("ja"), "ja");
    assert_eq!(lang2("deu"), "de");
    assert_eq!(lang2("heb"), "he");
}

#[test]
fn test_irr_suffix_with_3letter_code() {
    // Ensure the -irr suffix works when lang is "eng" (3-letter, the real-world case)
    let ctx = MappingContext {
        lang: talkbank_model::model::LanguageCode::new("eng"),
    };
    let ud = UdWord {
        id: UdId::Single(1),
        text: "went".into(),
        lemma: "go".into(),
        upos: UdPunctable::Value(UniversalPos::Verb),
        xpos: None,
        feats: Some("Mood=Ind|Number=Sing|Person=3|Tense=Past|VerbForm=Fin".into()),
        head: 0,
        deprel: "root".into(),
        deps: None,
        misc: None,
    };
    let mor = map_ud_word_to_mor(&ud, &ctx).unwrap();
    let mut out = String::new();
    mor.write_chat(&mut out).unwrap();
    assert!(
        out.contains("-irr"),
        "3-letter 'eng' should trigger irr check: {}",
        out
    );
}

#[test]
fn test_multivalue_ud_features_preserve_commas() {
    // Croatian: PronType=Int,Rel should preserve the comma per UD conventions
    let ctx = MappingContext {
        lang: talkbank_model::model::LanguageCode::new("hr"),
    };
    let ud = UdWord {
        id: UdId::Single(1),
        text: "što".into(),
        lemma: "što".into(),
        upos: UdPunctable::Value(UniversalPos::Pron),
        xpos: None,
        feats: Some("Case=Acc|PronType=Int,Rel".to_string()),
        head: 0,
        deprel: "root".into(),
        deps: None,
        misc: None,
    };
    let mor = map_ud_word_to_mor(&ud, &ctx).unwrap();
    let mut out = String::new();
    mor.write_chat(&mut out).unwrap();
    // Must contain comma — we respect UD multi-value feature conventions
    assert!(
        out.contains("Int,Rel"),
        "Expected Int,Rel (UD convention), got: {out}"
    );
}
