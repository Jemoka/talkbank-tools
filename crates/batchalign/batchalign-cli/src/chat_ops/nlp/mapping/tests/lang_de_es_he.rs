//! Language-specific tests for German, Spanish, and Hebrew: MWT contractions, verb-form defaults, binyan/existential markers.

#![allow(unused_imports, dead_code)]

use super::*;

use crate::chat_ops::nlp::mapping::validate_generated_gra;
use crate::chat_ops::nlp::mapping::*;
use crate::chat_ops::nlp::{UdId, UdPunctable, UdSentence, UdWord, UniversalPos};
use crate::chat_ops::nlp::{clean_lemma, map_ud_word_to_mor};
use talkbank_model::model::GrammaticalRelation;
use talkbank_model::model::dependent_tier::mor::Mor;

#[test]
fn test_hebrew_verb_hebbinyan() {
    // ba2: Hebrew HebBinyan feature → lowercased suffix
    let ctx = MappingContext {
        lang: talkbank_model::model::LanguageCode::new("he"),
    };
    let ud = UdWord {
        id: UdId::Single(1),
        text: "כתב".into(),
        lemma: "כתב".into(),
        upos: UdPunctable::Value(UniversalPos::Verb),
        xpos: None,
        feats: Some("HebBinyan=PAAL|Number=Sing|Person=3|Tense=Past|VerbForm=Fin".into()),
        head: 0,
        deprel: "root".into(),
        deps: None,
        misc: None,
    };
    let mor = map_ud_word_to_mor(&ud, &ctx).unwrap();
    let mut out = String::new();
    mor.write_chat(&mut out).unwrap();
    // HebBinyan=PAAL → lowercased "paal" in suffix
    assert!(
        out.contains("-paal-"),
        "Hebrew HebBinyan must be lowercased in suffix, got: {out}"
    );
    // No -irr (Hebrew, not English)
    assert!(!out.contains("-irr"), "Hebrew must not get -irr: {out}");
}

#[test]
fn test_hebrew_verb_hebexistential() {
    // ba2: Hebrew HebExistential feature → lowercased suffix
    let ctx = MappingContext {
        lang: talkbank_model::model::LanguageCode::new("he"),
    };
    let ud = UdWord {
        id: UdId::Single(1),
        text: "יש".into(),
        lemma: "יש".into(),
        upos: UdPunctable::Value(UniversalPos::Verb),
        xpos: None,
        feats: Some("HebExistential=True|VerbForm=Fin".into()),
        head: 0,
        deprel: "root".into(),
        deps: None,
        misc: None,
    };
    let mor = map_ud_word_to_mor(&ud, &ctx).unwrap();
    let mut out = String::new();
    mor.write_chat(&mut out).unwrap();
    // HebExistential=True → lowercased "true"
    assert!(
        out.contains("-true-") || out.contains("-true"),
        "Hebrew HebExistential must appear in suffix, got: {out}"
    );
}

#[test]
fn test_german_mwt_contraction_im() {
    // ba2: German "im" → "in" + "dem" via MWT Range
    let ctx = MappingContext {
        lang: talkbank_model::model::LanguageCode::new("de"),
    };
    let sentence = UdSentence {
        words: vec![
            UdWord {
                id: UdId::Range(1, 2),
                text: "im".into(),
                lemma: "im".into(),
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
                text: "in".into(),
                lemma: "in".into(),
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
                text: "dem".into(),
                lemma: "der".into(),
                upos: UdPunctable::Value(UniversalPos::Det),
                xpos: None,
                feats: Some("Case=Dat|Definite=Def|Gender=Masc|Number=Sing|PronType=Art".into()),
                head: 3,
                deprel: "det".into(),
                deps: None,
                misc: None,
            },
            UdWord {
                id: UdId::Single(3),
                text: "Haus".into(),
                lemma: "Haus".into(),
                upos: UdPunctable::Value(UniversalPos::Noun),
                xpos: None,
                feats: Some("Case=Dat|Gender=Neut|Number=Sing".into()),
                head: 0,
                deprel: "root".into(),
                deps: None,
                misc: None,
            },
        ],
    };
    let mors = map_ud_sentence_to_mors(&sentence, &ctx);
    // "im" MWT → clitic group (in~der) + "Haus" = 2 items
    assert_eq!(mors.len(), 2, "Expected 2 MOR items for 'im Haus'");

    let mut out0 = String::new();
    mors[0].write_chat(&mut out0).unwrap();
    assert!(
        out0.contains("adp|in") && out0.contains("det|der"),
        "Expected adp|in~det|der clitic, got: {out0}"
    );
}

#[test]
fn test_german_verb_no_irr_suffix() {
    // German verbs must never get -irr (English-only feature)
    let ctx = MappingContext {
        lang: talkbank_model::model::LanguageCode::new("de"),
    };
    let ud = UdWord {
        id: UdId::Single(1),
        text: "ging".into(),
        lemma: "gehen".into(),
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
        !out.contains("-irr"),
        "German verbs must NOT get -irr suffix, got: {out}"
    );
    assert_eq!(out, "verb|gehen-Fin-Ind-Past-S3");
}

#[test]
fn test_spanish_mwt_contraction_del() {
    // ba2: Spanish "del" → "de" + "el" via MWT Range
    let ctx = MappingContext {
        lang: talkbank_model::model::LanguageCode::new("es"),
    };
    let sentence = UdSentence {
        words: vec![
            UdWord {
                id: UdId::Range(1, 2),
                text: "del".into(),
                lemma: "del".into(),
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
                text: "el".into(),
                lemma: "el".into(),
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
                text: "parque".into(),
                lemma: "parque".into(),
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
    assert_eq!(mors.len(), 2, "Expected 2 MOR items for 'del parque'");

    let mut out0 = String::new();
    mors[0].write_chat(&mut out0).unwrap();
    assert!(
        out0.contains("adp|de") && out0.contains("det|el"),
        "Expected adp|de~det|el clitic for Spanish, got: {out0}"
    );
}

#[test]
fn test_spanish_verb_person0_becomes_4() {
    // ba2: Person=0 → "4" in NumberPerson string (all languages)
    let ctx = MappingContext {
        lang: talkbank_model::model::LanguageCode::new("es"),
    };
    let ud = UdWord {
        id: UdId::Single(1),
        text: "llueve".into(),
        lemma: "llover".into(),
        upos: UdPunctable::Value(UniversalPos::Verb),
        xpos: None,
        feats: Some("Mood=Ind|Number=Sing|Person=0|Tense=Pres|VerbForm=Fin".into()),
        head: 0,
        deprel: "root".into(),
        deps: None,
        misc: None,
    };
    let mor = map_ud_word_to_mor(&ud, &ctx).unwrap();
    let mut out = String::new();
    mor.write_chat(&mut out).unwrap();
    // Person=0 → "4" (ba2 convention for impersonal verbs)
    assert!(
        out.contains("-S4"),
        "Person=0 must map to '4' in suffix, got: {out}"
    );
}

#[test]
fn test_hebrew_3letter_code_works() {
    // Real-world: "heb" not "he"
    let ctx = MappingContext {
        lang: talkbank_model::model::LanguageCode::new("heb"),
    };
    let ud = UdWord {
        id: UdId::Single(1),
        text: "כתב".into(),
        lemma: "כתב".into(),
        upos: UdPunctable::Value(UniversalPos::Verb),
        xpos: None,
        feats: Some("HebBinyan=PAAL|Tense=Past|VerbForm=Fin".into()),
        head: 0,
        deprel: "root".into(),
        deps: None,
        misc: None,
    };
    let mor = map_ud_word_to_mor(&ud, &ctx).unwrap();
    let mut out = String::new();
    mor.write_chat(&mut out).unwrap();
    // "heb" must still process HebBinyan
    assert!(
        out.contains("-paal-"),
        "3-letter 'heb' must process HebBinyan, got: {out}"
    );
}
