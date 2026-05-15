//! Italian early-MWT contraction, default verbform/number, 3-letter language-code dispatch, garbage-deprel rejection, and the four `is_terminator_punct`/`mid_utterance_comma`/`sentence_terminator`/`comma_kept` non-test_-named integration tests.

#![allow(unused_imports, dead_code)]

use super::*;

use crate::chat_ops::nlp::mapping::validate_generated_gra;
use crate::chat_ops::nlp::mapping::*;
use crate::chat_ops::nlp::{UdId, UdPunctable, UdSentence, UdWord, UniversalPos};
use crate::chat_ops::nlp::{clean_lemma, map_ud_word_to_mor};
use talkbank_model::model::GrammaticalRelation;
use talkbank_model::model::dependent_tier::mor::Mor;

#[test]
fn test_italian_mwt_contraction_della() {
    // ba2: Italian "della" → "di" + "la" via MWT Range
    let ctx = MappingContext {
        lang: talkbank_model::model::LanguageCode::new("it"),
    };
    let sentence = UdSentence {
        words: vec![
            UdWord {
                id: UdId::Range(1, 2),
                text: "della".into(),
                lemma: "della".into(),
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
                text: "di".into(),
                lemma: "di".into(),
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
                text: "la".into(),
                lemma: "il".into(),
                upos: UdPunctable::Value(UniversalPos::Det),
                xpos: None,
                feats: Some("Definite=Def|Gender=Fem|Number=Sing|PronType=Art".into()),
                head: 3,
                deprel: "det".into(),
                deps: None,
                misc: None,
            },
            UdWord {
                id: UdId::Single(3),
                text: "casa".into(),
                lemma: "casa".into(),
                upos: UdPunctable::Value(UniversalPos::Noun),
                xpos: None,
                feats: Some("Gender=Fem|Number=Sing".into()),
                head: 0,
                deprel: "root".into(),
                deps: None,
                misc: None,
            },
        ],
    };
    let mors = map_ud_sentence_to_mors(&sentence, &ctx);
    assert_eq!(mors.len(), 2, "Expected 2 MOR items for 'della casa'");

    let mut out0 = String::new();
    mors[0].write_chat(&mut out0).unwrap();
    assert!(
        out0.contains("adp|di") && out0.contains("det|il"),
        "Expected adp|di~det|il clitic for Italian, got: {out0}"
    );
}

#[test]
fn test_verb_default_verbform_inf() {
    // ba2: VerbForm defaults to "Inf" when not present (ALL languages)
    for lang in ["fr", "de", "es", "it", "pt", "ja", "ko", "he"] {
        let ctx = MappingContext {
            lang: talkbank_model::model::LanguageCode::new(lang),
        };
        let ud = UdWord {
            id: UdId::Single(1),
            text: "x".into(),
            lemma: "x".into(),
            upos: UdPunctable::Value(UniversalPos::Verb),
            xpos: None,
            feats: None, // No features → defaults
            head: 0,
            deprel: "root".into(),
            deps: None,
            misc: None,
        };
        let mor = map_ud_word_to_mor(&ud, &ctx).unwrap();
        let mut out = String::new();
        mor.write_chat(&mut out).unwrap();
        assert!(
            out.contains("-Inf-"),
            "VerbForm must default to Inf for lang={lang}, got: {out}"
        );
    }
}

#[test]
fn test_verb_default_number_sing() {
    // ba2: Number defaults to "Sing" (→ "S") for verbs (ALL languages)
    for lang in ["fr", "de", "es", "it"] {
        let ctx = MappingContext {
            lang: talkbank_model::model::LanguageCode::new(lang),
        };
        let ud = UdWord {
            id: UdId::Single(1),
            text: "x".into(),
            lemma: "x".into(),
            upos: UdPunctable::Value(UniversalPos::Verb),
            xpos: None,
            feats: None, // No Number → defaults to "Sing" → "S"
            head: 0,
            deprel: "root".into(),
            deps: None,
            misc: None,
        };
        let mor = map_ud_word_to_mor(&ud, &ctx).unwrap();
        let mut out = String::new();
        mor.write_chat(&mut out).unwrap();
        assert!(
            out.contains("-S"),
            "Number must default to S(ing) for lang={lang}, got: {out}"
        );
    }
}

#[test]
fn test_garbage_deprel_rejected() {
    // A deprel with garbage characters should be rejected, not silently fixed.
    let ctx = MappingContext {
        lang: talkbank_model::model::LanguageCode::new("en"),
    };
    let sentence = UdSentence {
        words: vec![
            UdWord {
                id: UdId::Single(1),
                text: "dog".to_string(),
                lemma: "dog".to_string(),
                upos: UdPunctable::Value(UniversalPos::Noun),
                xpos: None,
                feats: None,
                head: 0,
                deprel: "root".to_string(),
                deps: None,
                misc: None,
            },
            UdWord {
                id: UdId::Single(2),
                text: "big".to_string(),
                lemma: "big".to_string(),
                upos: UdPunctable::Value(UniversalPos::Adj),
                xpos: None,
                feats: None,
                head: 1,
                deprel: "<PAD>".to_string(), // garbage deprel
                deps: None,
                misc: None,
            },
        ],
    };
    let err = map_ud_sentence(&sentence, &ctx).unwrap_err();
    assert!(
        matches!(err, MappingError::InvalidDeprel { .. }),
        "Expected InvalidDeprel, got: {err}"
    );
}

#[test]
fn is_terminator_punct_matches_only_sentence_terminators() {
    let make = |text: &str, lemma: &str| UdWord {
        id: UdId::Single(1),
        text: text.to_string(),
        lemma: lemma.to_string(),
        upos: UdPunctable::Value(UniversalPos::Punct),
        xpos: None,
        feats: None,
        head: 0,
        deprel: "punct".to_string(),
        deps: None,
        misc: None,
    };

    // Every CHAT utterance terminator string accepted by
    // `Terminator::try_from_chat_str` must be classified as a
    // terminator by our filter. The list mirrors the match arms of
    // `try_from_chat_str` (`talkbank-model::model::content::terminator`).
    for t in [
        ".", "!", "?", "+...", "+/.", "+//.", "+/?", "+!?", "+\"/.", "+\".", "+//?", "+..?", "+.",
    ] {
        assert!(
            super::is_terminator_punct(&make(t, t)),
            "CHAT terminator {t:?} must be classified as a terminator"
        );
    }

    // Content punctuation MUST NOT be classified as a terminator —
    // these flow through to `map_ud_word_to_mor` to produce Mor items
    // (`cm|cm`, `end|end`, `beg|beg`, etc.). The CA-prosody arrows
    // (`⇗ ↗ → ↘ ⇘ ≋ ≈`) belong here too: per CHECK they are
    // SEPARATORS, not terminators (BUG-009, 2026-05-01).
    // `Terminator::CaRisingToHigh` and friends remain as model variants
    // pending the full reclassification pass, but `try_from_chat_str`
    // does NOT construct them, so `is_terminator_punct` returns false
    // for the codepoints — which is the correct behavior.
    for t in [
        ",",
        ";",
        ":",
        "—",
        "(",
        ")",
        "\"",
        "„",
        "‡",
        "\u{21D7}", // ⇗
        "\u{2197}", // ↗
        "\u{2192}", // →
        "\u{2198}", // ↘
        "\u{21D8}", // ⇘
        "\u{224B}", // ≋
        "+\u{224B}",
        "\u{2248}", // ≈
        "+\u{2248}",
    ] {
        assert!(
            !super::is_terminator_punct(&make(t, t)),
            "content punct {t:?} must NOT be treated as a terminator \
             (CA-prosody arrows: separators per CHECK, BUG-009)"
        );
    }

    // Non-PUNCT UPOS is never a terminator even with a '.' text.
    let non_punct = UdWord {
        id: UdId::Single(1),
        text: ".".to_string(),
        lemma: ".".to_string(),
        upos: UdPunctable::Value(UniversalPos::Noun),
        xpos: None,
        feats: None,
        head: 0,
        deprel: "root".to_string(),
        deps: None,
        misc: None,
    };
    assert!(!super::is_terminator_punct(&non_punct));
}

#[test]
fn mid_utterance_comma_produces_cm_mor_item() {
    let ctx = MappingContext {
        lang: talkbank_model::model::LanguageCode::new("en"),
    };
    let sentence = UdSentence {
        words: vec![
            UdWord {
                id: UdId::Single(1),
                text: "hello".to_string(),
                lemma: "hello".to_string(),
                upos: UdPunctable::Value(UniversalPos::Intj),
                xpos: None,
                feats: None,
                head: 0,
                deprel: "root".to_string(),
                deps: None,
                misc: None,
            },
            UdWord {
                id: UdId::Single(2),
                text: ",".to_string(),
                lemma: ",".to_string(),
                upos: UdPunctable::Value(UniversalPos::Punct),
                xpos: None,
                feats: None,
                head: 1,
                deprel: "punct".to_string(),
                deps: None,
                misc: None,
            },
            UdWord {
                id: UdId::Single(3),
                text: "world".to_string(),
                lemma: "world".to_string(),
                upos: UdPunctable::Value(UniversalPos::Noun),
                xpos: None,
                feats: None,
                head: 1,
                deprel: "parataxis".to_string(),
                deps: None,
                misc: None,
            },
        ],
    };
    let (mors, _gras) = map_ud_sentence(&sentence, &ctx).unwrap();
    assert_eq!(
        mors.len(),
        3,
        "expected 3 Mor items (hello, cm, world); got: {:?}",
        mors.iter()
            .map(|m| {
                let mut s = String::new();
                m.write_chat(&mut s).unwrap();
                s
            })
            .collect::<Vec<_>>()
    );
    let mut comma_str = String::new();
    mors[1].write_chat(&mut comma_str).unwrap();
    assert_eq!(comma_str, "cm|cm");
}

#[test]
fn sentence_terminator_is_dropped_from_mor_output() {
    let ctx = MappingContext {
        lang: talkbank_model::model::LanguageCode::new("en"),
    };
    let sentence = UdSentence {
        words: vec![
            UdWord {
                id: UdId::Single(1),
                text: "hi".to_string(),
                lemma: "hi".to_string(),
                upos: UdPunctable::Value(UniversalPos::Intj),
                xpos: None,
                feats: None,
                head: 0,
                deprel: "root".to_string(),
                deps: None,
                misc: None,
            },
            UdWord {
                id: UdId::Single(2),
                text: ".".to_string(),
                lemma: ".".to_string(),
                upos: UdPunctable::Value(UniversalPos::Punct),
                xpos: None,
                feats: None,
                head: 1,
                deprel: "punct".to_string(),
                deps: None,
                misc: None,
            },
        ],
    };
    let (mors, _gras) = map_ud_sentence(&sentence, &ctx).unwrap();
    assert_eq!(mors.len(), 1, "terminator '.' must not produce a Mor item");
    let mut out = String::new();
    mors[0].write_chat(&mut out).unwrap();
    assert_eq!(out, "intj|hi");
}

#[test]
fn comma_kept_terminator_dropped_together() {
    let ctx = MappingContext {
        lang: talkbank_model::model::LanguageCode::new("en"),
    };
    let mk = |id: usize, text: &str, upos: UniversalPos, head: usize, deprel: &str| UdWord {
        id: UdId::Single(id),
        text: text.to_string(),
        lemma: text.to_string(),
        upos: UdPunctable::Value(upos),
        xpos: None,
        feats: None,
        head,
        deprel: deprel.to_string(),
        deps: None,
        misc: None,
    };
    let sentence = UdSentence {
        words: vec![
            mk(1, "yes", UniversalPos::Intj, 0, "root"),
            mk(2, ",", UniversalPos::Punct, 1, "punct"),
            mk(3, "ok", UniversalPos::Intj, 1, "parataxis"),
            mk(4, ".", UniversalPos::Punct, 1, "punct"),
        ],
    };
    let (mors, _gras) = map_ud_sentence(&sentence, &ctx).unwrap();
    let items: Vec<String> = mors
        .iter()
        .map(|m| {
            let mut s = String::new();
            m.write_chat(&mut s).unwrap();
            s
        })
        .collect();
    assert_eq!(items, vec!["intj|yes", "cm|cm", "intj|ok"]);
}
