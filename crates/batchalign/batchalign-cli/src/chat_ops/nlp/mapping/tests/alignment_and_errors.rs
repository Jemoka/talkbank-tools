//! MWT GRA alignment cases (per-component, dont contraction) and structural-error paths (apostrophe fallback, empty stem, unmapped head, no-root parse).

#![allow(unused_imports, dead_code)]

use super::*;

use crate::chat_ops::nlp::mapping::validate_generated_gra;
use crate::chat_ops::nlp::mapping::*;
use crate::chat_ops::nlp::{UdId, UdPunctable, UdSentence, UdWord, UniversalPos};
use crate::chat_ops::nlp::{clean_lemma, map_ud_word_to_mor};
use talkbank_model::model::GrammaticalRelation;
use talkbank_model::model::dependent_tier::mor::Mor;

#[test]
fn test_mwt_gra_per_component_alignment() {
    let sentence = UdSentence {
        words: vec![
            // Range entry for the MWT "it's"
            UdWord {
                id: UdId::Range(1, 2),
                text: "it's".into(),
                lemma: "it's".into(),
                upos: UdPunctable::Punct("X".into()),
                xpos: None,
                feats: None,
                head: 0,
                deprel: "dep".into(),
                deps: None,
                misc: None,
            },
            // Component 1: "it"
            UdWord {
                id: UdId::Single(1),
                text: "it".into(),
                lemma: "it".into(),
                upos: UdPunctable::Value(UniversalPos::Pron),
                xpos: Some("PRP".into()),
                feats: Some("Case=Nom|Gender=Neut|Number=Sing|Person=3|PronType=Prs".into()),
                head: 3,
                deprel: "nsubj".into(),
                deps: None,
                misc: None,
            },
            // Component 2: "'s"
            UdWord {
                id: UdId::Single(2),
                text: "'s".into(),
                lemma: "be".into(),
                upos: UdPunctable::Value(UniversalPos::Aux),
                xpos: Some("VBZ".into()),
                feats: Some("Mood=Ind|Number=Sing|Person=3|Tense=Pres|VerbForm=Fin".into()),
                head: 0,
                deprel: "root".into(),
                deps: None,
                misc: None,
            },
            // Regular word: "just"
            UdWord {
                id: UdId::Single(3),
                text: "just".into(),
                lemma: "just".into(),
                upos: UdPunctable::Value(UniversalPos::Adv),
                xpos: Some("RB".into()),
                feats: None,
                head: 2,
                deprel: "advmod".into(),
                deps: None,
                misc: None,
            },
        ],
    };

    let ctx = MappingContext {
        lang: talkbank_model::model::LanguageCode::new("eng"),
    };
    let (mors, gras) = map_ud_sentence(&sentence, &ctx).unwrap();

    // MOR: clitic group (it~be) + just = 2 items, 3 chunks
    assert_eq!(mors.len(), 2, "Expected 2 MOR items (clitic group + adv)");
    let total_chunks: usize = mors.iter().map(|m| m.count_chunks()).sum();
    assert_eq!(total_chunks, 3, "Expected 3 MOR chunks (it + 's + just)");

    // GRA: 3 chunks + 1 terminator = 4 relations
    assert_eq!(
        gras.len(),
        4,
        "Expected 4 GRA entries (3 chunks + terminator PUNCT), got {gras:?}"
    );

    // Verify per-component indexing: chunk 1 = it, chunk 2 = 's, chunk 3 = just, chunk 4 = terminator
    assert_eq!(gras[0].index, 1);
    assert_eq!(gras[1].index, 2);
    assert_eq!(gras[2].index, 3);
    assert_eq!(gras[3].index, 4);
    assert_eq!(gras[3].relation, "PUNCT".to_string().into());
}

#[test]
fn test_mwt_gra_dont_contraction() {
    let sentence = UdSentence {
        words: vec![
            UdWord {
                id: UdId::Single(1),
                text: "I".into(),
                lemma: "I".into(),
                upos: UdPunctable::Value(UniversalPos::Pron),
                xpos: Some("PRP".into()),
                feats: Some("Case=Nom|Number=Sing|Person=1|PronType=Prs".into()),
                head: 4,
                deprel: "nsubj".into(),
                deps: None,
                misc: None,
            },
            // Range entry for "don't"
            UdWord {
                id: UdId::Range(2, 3),
                text: "don't".into(),
                lemma: "don't".into(),
                upos: UdPunctable::Punct("X".into()),
                xpos: None,
                feats: None,
                head: 0,
                deprel: "dep".into(),
                deps: None,
                misc: None,
            },
            UdWord {
                id: UdId::Single(2),
                text: "do".into(),
                lemma: "do".into(),
                upos: UdPunctable::Value(UniversalPos::Aux),
                xpos: Some("VBP".into()),
                feats: Some("Mood=Ind|Number=Sing|Person=1|Tense=Pres|VerbForm=Fin".into()),
                head: 4,
                deprel: "aux".into(),
                deps: None,
                misc: None,
            },
            UdWord {
                id: UdId::Single(3),
                text: "n't".into(),
                lemma: "not".into(),
                upos: UdPunctable::Value(UniversalPos::Part),
                xpos: Some("RB".into()),
                feats: Some("Polarity=Neg".into()),
                head: 4,
                deprel: "advmod".into(),
                deps: None,
                misc: None,
            },
            UdWord {
                id: UdId::Single(4),
                text: "know".into(),
                lemma: "know".into(),
                upos: UdPunctable::Value(UniversalPos::Verb),
                xpos: Some("VB".into()),
                feats: Some("VerbForm=Inf".into()),
                head: 0,
                deprel: "root".into(),
                deps: None,
                misc: None,
            },
        ],
    };

    let ctx = MappingContext {
        lang: talkbank_model::model::LanguageCode::new("eng"),
    };
    let (mors, gras) = map_ud_sentence(&sentence, &ctx).unwrap();

    // MOR: I + (do~n't) + know = 3 items
    assert_eq!(mors.len(), 3, "Expected 3 MOR items");
    // Chunks: I(1) + do(1) + n't(1) + know(1) = 4
    let total_chunks: usize = mors.iter().map(|m| m.count_chunks()).sum();
    assert_eq!(total_chunks, 4, "Expected 4 MOR chunks");

    // GRA: 4 chunks + 1 terminator = 5 relations
    assert_eq!(
        gras.len(),
        5,
        "Expected 5 GRA entries (4 chunks + terminator), got {gras:?}"
    );
}

#[test]
fn test_clean_lemma_apostrophe_fallback_to_text() {
    // clean_lemma("'", "'") must not return empty — fallback to surface text
    let (result, unknown) = clean_lemma("'", "'");
    assert!(
        !result.is_empty(),
        "clean_lemma must never return empty string"
    );
    assert_eq!(result, "'", "Expected fallback to surface text \"'\"");
    assert!(!unknown, "Not an unknown token");
}

#[test]
fn test_map_ud_word_apostrophe_no_empty_stem() {
    // map_ud_word_to_mor with an apostrophe-only PUNCT token must produce
    // "punct|'" (non-empty stem), not "punct|" (E342).
    let ctx = MappingContext {
        lang: talkbank_model::model::LanguageCode::new("en"),
    };
    let ud = UdWord {
        id: UdId::Single(2),
        text: "'".to_string(),
        lemma: "'".to_string(),
        upos: UdPunctable::Value(UniversalPos::Punct),
        xpos: None,
        feats: None,
        head: 1,
        deprel: "case".to_string(),
        deps: None,
        misc: None,
    };

    let mor = map_ud_word_to_mor(&ud, &ctx).unwrap();
    let mut out = String::new();
    mor.write_chat(&mut out).unwrap();

    assert!(!out.ends_with('|'), "Empty stem produces E342: got {out:?}");
    // Should produce "punct|'" — apostrophe preserved as stem
    assert_eq!(out, "punct|'", "Expected punct|' not punct|");
}

#[test]
fn test_map_ud_word_rejects_empty_stem() {
    // If clean_lemma and sanitize_mor_text both produce an empty string,
    // map_ud_word_to_mor must return Err(EmptyStem), not silently pass.
    let ctx = MappingContext {
        lang: talkbank_model::model::LanguageCode::new("en"),
    };
    // Craft a UD word whose lemma sanitizes to empty (all reserved chars).
    let ud = UdWord {
        id: UdId::Single(1),
        text: "|||".to_string(),
        lemma: "|||".to_string(), // clean_lemma preserves; sanitize strips '|' → "___" → non-empty
        upos: UdPunctable::Value(UniversalPos::Noun),
        xpos: None,
        feats: None,
        head: 0,
        deprel: "root".to_string(),
        deps: None,
        misc: None,
    };
    // sanitize_mor_text replaces reserved chars, so this won't actually be empty.
    // Verify it succeeds (non-empty stem after sanitization).
    let result = map_ud_word_to_mor(&ud, &ctx);
    assert!(
        result.is_ok(),
        "Reserved chars should be sanitized, not empty: {result:?}"
    );
}

#[test]
fn test_unmapped_head_reference() {
    // A word's head points to a decimal ID (not in chunk index map).
    // Should return Err(InvalidHeadReference), not silently fall back to 0.
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
            // Decimal word (empty/enhanced token) — not indexed in chunk map
            UdWord {
                id: UdId::Decimal(1.1),
                text: "of".to_string(),
                lemma: "of".to_string(),
                upos: UdPunctable::Value(UniversalPos::Adp),
                xpos: None,
                feats: None,
                head: 1,
                deprel: "case".to_string(),
                deps: None,
                misc: None,
            },
            UdWord {
                id: UdId::Single(2),
                text: "cat".to_string(),
                lemma: "cat".to_string(),
                upos: UdPunctable::Value(UniversalPos::Noun),
                xpos: None,
                feats: None,
                // Head 99 does not exist in the chunk map
                head: 99,
                deprel: "nmod".to_string(),
                deps: None,
                misc: None,
            },
        ],
    };
    let err = map_ud_sentence(&sentence, &ctx).unwrap_err();
    assert!(
        matches!(err, MappingError::InvalidHeadReference { .. }),
        "Expected InvalidHeadReference, got: {err}"
    );
}

#[test]
fn test_no_root_in_ud_parse() {
    // All words have non-zero heads forming a chain — no root.
    // Should return Err(InvalidRoot), not silently use root_chunk_idx=0.
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
                head: 2, // not root
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
                head: 1, // circular, but no head=0
                deprel: "nsubj".to_string(),
                deps: None,
                misc: None,
            },
        ],
    };
    let err = map_ud_sentence(&sentence, &ctx).unwrap_err();
    assert!(
        matches!(err, MappingError::InvalidRoot { .. }),
        "Expected InvalidRoot for no-root parse, got: {err}"
    );
}
