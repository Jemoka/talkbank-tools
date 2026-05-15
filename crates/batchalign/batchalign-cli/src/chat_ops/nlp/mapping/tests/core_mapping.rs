//! Core UD-to-CHAT mapping tests: simple POS, MWT assembly, GRA index shifting, feature mapping, generated-GRA validation, talkbank GRA conventions.

#![allow(unused_imports, dead_code)]

use super::*;

use crate::chat_ops::nlp::mapping::validate_generated_gra;
use crate::chat_ops::nlp::mapping::*;
use crate::chat_ops::nlp::{UdId, UdPunctable, UdSentence, UdWord, UniversalPos};
use crate::chat_ops::nlp::{clean_lemma, map_ud_word_to_mor};
use talkbank_model::model::GrammaticalRelation;
use talkbank_model::model::dependent_tier::mor::Mor;

#[test]
fn test_simple_noun_mapping() {
    let ctx = MappingContext {
        lang: talkbank_model::model::LanguageCode::new("en"),
    };
    let ud = UdWord {
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
    };

    let mor = map_ud_word_to_mor(&ud, &ctx).unwrap();
    let mut out = String::new();
    mor.write_chat(&mut out).unwrap();
    // Python: noun|dog (UPOS lowercased)
    assert_eq!(out, "noun|dog");
}

#[test]
fn test_sanitization_prevents_corruption() {
    let ctx = MappingContext {
        lang: talkbank_model::model::LanguageCode::new("en"),
    };
    let ud = UdWord {
        id: UdId::Single(1),
        text: "bad|word".to_string(),
        lemma: "bad|word".to_string(), // Lemma contains a reserved CHAT character!
        upos: UdPunctable::Value(UniversalPos::Noun),
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

    // clean_lemma takes everything before '|' → "bad"
    assert_eq!(out, "noun|bad");
    assert!(
        out.matches('|').count() == 1,
        "Sanitization failed to remove illegal reserved character '|' from stem"
    );
}

#[test]
fn test_mwt_assembly_english_dont() {
    let ctx = MappingContext {
        lang: talkbank_model::model::LanguageCode::new("en"),
    };
    let sentence = UdSentence {
        words: vec![
            UdWord {
                id: UdId::Range(1, 2),
                text: "don't".to_string(),
                lemma: "do not".to_string(),
                upos: UdPunctable::Value(UniversalPos::Verb),
                xpos: None,
                feats: None,
                head: 0,
                deprel: "root".to_string(),
                deps: None,
                misc: None,
            },
            UdWord {
                id: UdId::Single(1),
                text: "do".to_string(),
                lemma: "do".to_string(),
                upos: UdPunctable::Value(UniversalPos::Aux),
                xpos: None,
                feats: None,
                head: 0,
                deprel: "root".to_string(),
                deps: None,
                misc: None,
            },
            UdWord {
                id: UdId::Single(2),
                text: "n't".to_string(),
                lemma: "not".to_string(),
                upos: UdPunctable::Value(UniversalPos::Part),
                xpos: None,
                feats: None,
                head: 1,
                deprel: "advmod".to_string(),
                deps: None,
                misc: None,
            },
        ],
    };

    let mors = map_ud_sentence_to_mors(&sentence, &ctx);
    assert_eq!(mors.len(), 1);
    let mut out = String::new();
    mors[0].write_chat(&mut out).unwrap();

    // AUX "do" gets verb suffixes (VerbForm=Inf default, Number=S)
    // PART "not" gets no suffixes
    assert_eq!(out, "aux|do-Inf-S~part|not");
}

#[test]
fn test_gra_index_shifting_with_mwt() {
    let ctx = MappingContext {
        lang: talkbank_model::model::LanguageCode::new("en"),
    };
    let sentence = UdSentence {
        words: vec![
            UdWord {
                id: UdId::Range(1, 2),
                text: "don't".to_string(),
                lemma: "do not".to_string(),
                upos: UdPunctable::Value(UniversalPos::Verb),
                xpos: None,
                feats: None,
                head: 0,
                deprel: "root".to_string(),
                deps: None,
                misc: None,
            },
            UdWord {
                id: UdId::Single(1),
                text: "do".to_string(),
                lemma: "do".to_string(),
                upos: UdPunctable::Value(UniversalPos::Aux),
                xpos: None,
                feats: None,
                head: 0,
                deprel: "root".to_string(),
                deps: None,
                misc: None,
            },
            UdWord {
                id: UdId::Single(2),
                text: "n't".to_string(),
                lemma: "not".to_string(),
                upos: UdPunctable::Value(UniversalPos::Part),
                xpos: None,
                feats: None,
                head: 1,
                deprel: "advmod".to_string(),
                deps: None,
                misc: None,
            },
            UdWord {
                id: UdId::Single(3),
                text: "go".to_string(),
                lemma: "go".to_string(),
                upos: UdPunctable::Value(UniversalPos::Verb),
                xpos: None,
                feats: None,
                head: 1,
                deprel: "conj".to_string(),
                deps: None,
                misc: None,
            },
        ],
    };

    let (_mors, gras) = map_ud_sentence(&sentence, &ctx).unwrap();

    // MWT "don't" produces 2 chunks (do + n't), "go" is 1 chunk, + terminator = 4
    // Chunk indices: do=1, n't=2, go=3, .=4
    assert_eq!(gras.len(), 4);

    // "do" component (root — head=0)
    assert_eq!(gras[0].index, 1);
    assert_eq!(gras[0].head, 0);
    assert_eq!(gras[0].relation, "ROOT".into());

    // "n't" component (advmod of "do", chunk 1)
    assert_eq!(gras[1].index, 2);
    assert_eq!(gras[1].head, 1);
    assert_eq!(gras[1].relation, "ADVMOD".into());

    // "go" (conj of "do", chunk 1)
    assert_eq!(gras[2].index, 3);
    assert_eq!(gras[2].head, 1);
    assert_eq!(gras[2].relation, "CONJ".into());

    // Terminator
    assert_eq!(gras[3].index, 4);
    assert_eq!(gras[3].head, 1);
    assert_eq!(gras[3].relation, "PUNCT".into());
}

#[test]
fn test_feature_mapping_plural() {
    let ctx = MappingContext {
        lang: talkbank_model::model::LanguageCode::new("en"),
    };
    let ud = UdWord {
        id: UdId::Single(1),
        text: "dogs".to_string(),
        lemma: "dog".to_string(),
        upos: UdPunctable::Value(UniversalPos::Noun),
        xpos: None,
        feats: Some("Number=Plur".to_string()),
        head: 0,
        deprel: "root".to_string(),
        deps: None,
        misc: None,
    };

    let mor = map_ud_word_to_mor(&ud, &ctx).unwrap();
    let mut out = String::new();
    mor.write_chat(&mut out).unwrap();
    // Python: noun|dog-Plur (NOUN suffix: Number kept as-is)
    assert_eq!(out, "noun|dog-Plur");
}

#[test]
fn test_feature_mapping_past_tense() {
    let ctx = MappingContext {
        lang: talkbank_model::model::LanguageCode::new("en"),
    };
    let ud = UdWord {
        id: UdId::Single(1),
        text: "walked".to_string(),
        lemma: "walk".to_string(),
        upos: UdPunctable::Value(UniversalPos::Verb),
        xpos: None,
        feats: Some("Tense=Past".to_string()),
        head: 0,
        deprel: "root".to_string(),
        deps: None,
        misc: None,
    };

    let mor = map_ud_word_to_mor(&ud, &ctx).unwrap();
    let mut out = String::new();
    mor.write_chat(&mut out).unwrap();
    // Python: verb|walk-Inf-Past-S (VerbForm default "Inf", Tense "Past", Number default "S")
    assert_eq!(out, "verb|walk-Inf-Past-S");
}

#[test]
fn test_english_gerund_fix() {
    let ctx = MappingContext {
        lang: talkbank_model::model::LanguageCode::new("en"),
    };
    let ud = UdWord {
        id: UdId::Single(1),
        text: "walking".to_string(),
        lemma: "walk".to_string(),
        upos: UdPunctable::Value(UniversalPos::Noun),
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
    // Python: noun|walk-Ger (NOUN suffix for English -ing words)
    assert_eq!(out, "noun|walk-Ger");
}

#[test]
fn test_validate_generated_gra_accepts_valid() {
    // Valid structure: single root, no cycles
    let gras = vec![
        GrammaticalRelation {
            index: 1,
            head: 2,
            relation: "DET".into(),
        },
        GrammaticalRelation {
            index: 2,
            head: 2,
            relation: "ROOT".into(),
        },
        GrammaticalRelation {
            index: 3,
            head: 2,
            relation: "OBJ".into(),
        },
        GrammaticalRelation {
            index: 4,
            head: 2,
            relation: "PUNCT".into(),
        },
    ];
    validate_generated_gra(&gras).unwrap();
}

#[test]
fn test_validate_generated_gra_rejects_no_root() {
    let gras = vec![
        GrammaticalRelation {
            index: 1,
            head: 2,
            relation: "SUBJ".into(),
        },
        GrammaticalRelation {
            index: 2,
            head: 3,
            relation: "ROOT".into(),
        },
        GrammaticalRelation {
            index: 3,
            head: 1,
            relation: "OBJ".into(),
        },
        GrammaticalRelation {
            index: 4,
            head: 1,
            relation: "PUNCT".into(),
        },
    ];
    let err = validate_generated_gra(&gras).unwrap_err();
    assert!(
        matches!(err, MappingError::InvalidRoot { .. }),
        "Expected InvalidRoot, got: {err}"
    );
}

#[test]
fn test_validate_generated_gra_rejects_multiple_roots() {
    let gras = vec![
        GrammaticalRelation {
            index: 1,
            head: 1,
            relation: "ROOT".into(),
        },
        GrammaticalRelation {
            index: 2,
            head: 2,
            relation: "ROOT".into(),
        },
        GrammaticalRelation {
            index: 3,
            head: 1,
            relation: "PUNCT".into(),
        },
    ];
    let err = validate_generated_gra(&gras).unwrap_err();
    assert!(
        matches!(err, MappingError::InvalidRoot { .. }),
        "Expected InvalidRoot, got: {err}"
    );
}

#[test]
fn test_validate_generated_gra_rejects_cycle() {
    let gras = vec![
        GrammaticalRelation {
            index: 1,
            head: 2,
            relation: "FLAT".into(),
        },
        GrammaticalRelation {
            index: 2,
            head: 1,
            relation: "APPOS".into(),
        },
        GrammaticalRelation {
            index: 3,
            head: 3,
            relation: "ROOT".into(),
        },
        GrammaticalRelation {
            index: 4,
            head: 3,
            relation: "PUNCT".into(),
        },
    ];
    let err = validate_generated_gra(&gras).unwrap_err();
    assert!(
        matches!(err, MappingError::CircularDependency { .. }),
        "Expected CircularDependency, got: {err}"
    );
}

#[test]
fn test_validate_generated_gra_rejects_invalid_head() {
    let gras = vec![
        GrammaticalRelation {
            index: 1,
            head: 99,
            relation: "SUBJ".into(),
        },
        GrammaticalRelation {
            index: 2,
            head: 2,
            relation: "ROOT".into(),
        },
        GrammaticalRelation {
            index: 3,
            head: 2,
            relation: "PUNCT".into(),
        },
    ];
    let err = validate_generated_gra(&gras).unwrap_err();
    assert!(
        matches!(err, MappingError::InvalidHeadReference { .. }),
        "Expected InvalidHeadReference, got: {err}"
    );
}

#[test]
fn test_validate_generated_gra_accepts_head_zero() {
    let gras = vec![
        GrammaticalRelation {
            index: 1,
            head: 2,
            relation: "DET".into(),
        },
        GrammaticalRelation {
            index: 2,
            head: 0,
            relation: "ROOT".into(),
        },
        GrammaticalRelation {
            index: 3,
            head: 2,
            relation: "PUNCT".into(),
        },
    ];
    validate_generated_gra(&gras).unwrap();
}

#[test]
fn test_gra_talkbank_conventions() {
    let ctx = MappingContext {
        lang: talkbank_model::model::LanguageCode::new("en"),
    };
    let sentence = UdSentence {
        words: vec![
            UdWord {
                id: UdId::Single(1),
                text: "the".to_string(),
                lemma: "the".to_string(),
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
                id: UdId::Single(3),
                text: "that".to_string(),
                lemma: "that".to_string(),
                upos: UdPunctable::Value(UniversalPos::Pron),
                xpos: None,
                feats: None,
                head: 4,
                deprel: "nsubj".to_string(),
                deps: None,
                misc: None,
            },
            UdWord {
                id: UdId::Single(4),
                text: "barks".to_string(),
                lemma: "bark".to_string(),
                upos: UdPunctable::Value(UniversalPos::Verb),
                xpos: None,
                feats: None,
                head: 2,
                // UD uses colon for subtypes: "acl:relcl"
                deprel: "acl:relcl".to_string(),
                deps: None,
                misc: None,
            },
        ],
    };

    let (_mors, gras) = map_ud_sentence(&sentence, &ctx).unwrap();

    // 4 words + 1 terminator
    assert_eq!(gras.len(), 5);

    // Convention 1: ROOT head=0 (virtual root node)
    assert_eq!(gras[1].index, 2);
    assert_eq!(gras[1].head, 0, "ROOT head must be 0 (virtual root)");
    assert_eq!(gras[1].relation, "ROOT".into());

    // Convention 2: UD colon subtypes become TalkBank dashes
    assert_eq!(gras[3].index, 4);
    assert_eq!(gras[3].head, 2);
    assert_eq!(
        gras[3].relation,
        "ACL-RELCL".into(),
        "TalkBank uses dashes for subtypes, not colons"
    );

    // Convention 3: All labels uppercase
    assert_eq!(gras[0].relation, "DET".into());
    assert_eq!(gras[2].relation, "NSUBJ".into());

    // Convention 4: Terminator PUNCT head points to ROOT word
    assert_eq!(gras[4].index, 5);
    assert_eq!(
        gras[4].head, 2,
        "Terminator PUNCT head must point to ROOT word"
    );
    assert_eq!(gras[4].relation, "PUNCT".into());
}
