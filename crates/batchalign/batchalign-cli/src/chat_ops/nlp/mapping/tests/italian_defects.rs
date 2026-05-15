//! Italian Stanza defect-8/9/10/12/13 mid-sentence and clitic-decomposition tests: dammelo/prendilo/digliela/posala/dagliela head rewriting and multi-chunk emission.

#![allow(unused_imports, dead_code)]

use super::*;

use crate::chat_ops::nlp::mapping::validate_generated_gra;
use crate::chat_ops::nlp::mapping::*;
use crate::chat_ops::nlp::{UdId, UdPunctable, UdSentence, UdWord, UniversalPos};
use crate::chat_ops::nlp::{clean_lemma, map_ud_word_to_mor};
use talkbank_model::model::GrammaticalRelation;
use talkbank_model::model::dependent_tier::mor::Mor;

#[test]
fn test_italian_defect8_dammela_mid_sentence_becomes_verb() {
    let sentence = UdSentence {
        words: vec![
            it_word(1, "per", "per", UniversalPos::Adp, 3, "case", None),
            it_word(2, "favore", "favore", UniversalPos::Noun, 3, "obl", None),
            it_word(
                3,
                "dammela",
                "dammelo",
                UniversalPos::Adj,
                0,
                "root",
                Some("Number=Sing"),
            ),
        ],
    };
    let (mors, _) = map_ud_sentence(&sentence, &it_ctx()).unwrap();
    let items = chat_strings(&mors);
    assert_eq!(items.len(), 3, "got {items:?}");
    // The third word (`dammela`) must now be tagged verb with
    // lemma `dare`, not adj with lemma `dammelo`.
    assert!(
        items[2].starts_with("v|dare") || items[2].starts_with("verb|dare"),
        "Defect 8: expected verb|dare for dammela mid-sentence, got {:?}",
        items[2]
    );
    assert!(
        !items[2].starts_with("adj|"),
        "Defect 8: dammela must not stay ADJ-tagged, got {:?}",
        items[2]
    );
}

#[test]
fn test_italian_defect8_dammelo_mid_sentence_becomes_verb() {
    let sentence = UdSentence {
        words: vec![
            it_word(1, "per", "per", UniversalPos::Adp, 3, "case", None),
            it_word(2, "favore", "favore", UniversalPos::Noun, 3, "obl", None),
            it_word(
                3,
                "dammelo",
                "dammelo",
                UniversalPos::Adj,
                0,
                "root",
                Some("Number=Sing"),
            ),
        ],
    };
    let (mors, _) = map_ud_sentence(&sentence, &it_ctx()).unwrap();
    let items = chat_strings(&mors);
    assert_eq!(items.len(), 3, "got {items:?}");
    assert!(
        items[2].starts_with("v|dare") || items[2].starts_with("verb|dare"),
        "Defect 8: expected verb|dare for dammelo, got {:?}",
        items[2]
    );
}

#[test]
fn test_italian_defect8_prendilo_mid_sentence_becomes_verb() {
    let sentence = UdSentence {
        words: vec![
            it_word(1, "per", "per", UniversalPos::Adp, 3, "case", None),
            it_word(2, "favore", "favore", UniversalPos::Noun, 3, "obl", None),
            it_word(
                3,
                "prendilo",
                "prendilo",
                UniversalPos::Adj,
                0,
                "root",
                Some("Number=Sing"),
            ),
        ],
    };
    let (mors, _) = map_ud_sentence(&sentence, &it_ctx()).unwrap();
    let items = chat_strings(&mors);
    assert_eq!(items.len(), 3);
    assert!(
        items[2].starts_with("v|prendere") || items[2].starts_with("verb|prendere"),
        "Defect 8: expected verb|prendere, got {:?}",
        items[2]
    );
}

#[test]
fn test_italian_defect8_dammela_emits_multi_chunk_mor() {
    let sentence = UdSentence {
        words: vec![
            it_word(1, "per", "per", UniversalPos::Adp, 3, "case", None),
            it_word(2, "favore", "favore", UniversalPos::Noun, 0, "root", None),
            it_word(
                3,
                "dammela",
                "dammelo",
                UniversalPos::Adj,
                2,
                "amod",
                Some("Gender=Masc|Number=Sing"),
            ),
        ],
    };
    let (mors, gras) = map_ud_sentence(&sentence, &it_ctx()).unwrap();
    let items = chat_strings(&mors);
    // `dammela` should emit a 3-chunk Mor: verb|dare + me + la.
    assert!(
        items[2].contains("~pron|me"),
        "Expected ~pron|me in dammela output, got {:?}",
        items[2]
    );
    assert!(
        items[2].contains("~pron|la"),
        "Expected ~pron|la in dammela output, got {:?}",
        items[2]
    );
    // Main word + 2 clitics = 3 chunks for dammela, plus
    // `per` (1) + `favore` (1) + terminator PUNCT (1) = 6 total.
    let total_chunks: usize = mors.iter().map(|m| m.count_chunks()).sum();
    assert_eq!(
        total_chunks, 5,
        "Expected 5 mor chunks (per + favore + dammela*3), got {total_chunks}"
    );
    assert_eq!(
        gras.len(),
        total_chunks + 1, // +1 for terminator PUNCT relation
        "Expected {} gra relations, got {}",
        total_chunks + 1,
        gras.len()
    );
}

#[test]
fn test_italian_defect12_aprilo_verb_becomes_multi_chunk() {
    let sentence = UdSentence {
        words: vec![
            it_word(1, "per", "per", UniversalPos::Adp, 3, "case", None),
            it_word(2, "favore", "favore", UniversalPos::Noun, 0, "root", None),
            it_word(
                3,
                "aprilo",
                "aprire",
                UniversalPos::Verb,
                2,
                "advcl",
                Some("Mood=Imp|Number=Sing|Person=2|VerbForm=Fin"),
            ),
        ],
    };
    let (mors, _) = map_ud_sentence(&sentence, &it_ctx()).unwrap();
    let items = chat_strings(&mors);
    assert!(
        items[2].starts_with("v|aprire") || items[2].starts_with("verb|aprire"),
        "Expected verb|aprire for aprilo, got {:?}",
        items[2]
    );
    assert!(
        items[2].contains("~pron|lo"),
        "Expected ~pron|lo post-clitic in aprilo output, got {:?}",
        items[2]
    );
}

#[test]
fn test_italian_defect13_leggila_fabricated_lemma_becomes_leggere() {
    let sentence = UdSentence {
        words: vec![
            it_word(1, "per", "per", UniversalPos::Adp, 3, "case", None),
            it_word(2, "favore", "favore", UniversalPos::Noun, 0, "root", None),
            it_word(
                3,
                "leggila",
                "leggilare",
                UniversalPos::Verb,
                2,
                "advcl",
                None,
            ),
        ],
    };
    let (mors, _) = map_ud_sentence(&sentence, &it_ctx()).unwrap();
    let items = chat_strings(&mors);
    assert!(
        items[2].starts_with("v|leggere") || items[2].starts_with("verb|leggere"),
        "Expected verb|leggere (not leggilare) for leggila, got {:?}",
        items[2]
    );
    assert!(
        items[2].contains("~pron|la"),
        "Expected ~pron|la post-clitic in leggila output, got {:?}",
        items[2]
    );
    assert!(
        !items[2].contains("leggilare"),
        "The fabricated Stanza lemma leggilare must be stripped, got {:?}",
        items[2]
    );
}

#[test]
fn test_italian_defect8_genuine_verb_stays_verb() {
    let sentence = UdSentence {
        words: vec![it_word(
            1,
            "guarda",
            "guardare",
            UniversalPos::Verb,
            0,
            "root",
            Some("Mood=Imp|Number=Sing|Person=2|VerbForm=Fin"),
        )],
    };
    let (mors, _) = map_ud_sentence(&sentence, &it_ctx()).unwrap();
    let items = chat_strings(&mors);
    assert!(
        items[0].starts_with("v|guardare") || items[0].starts_with("verb|guardare"),
        "guarda (genuine imperative verb) must stay verb|guardare, got {:?}",
        items[0]
    );
}

#[test]
fn test_italian_defect8_aprila_noun_becomes_verb() {
    let sentence = UdSentence {
        words: vec![
            it_word(1, "per", "per", UniversalPos::Adp, 3, "case", None),
            it_word(2, "favore", "favore", UniversalPos::Noun, 0, "root", None),
            it_word(3, "aprila", "aprila", UniversalPos::Noun, 2, "nmod", None),
        ],
    };
    let (mors, _) = map_ud_sentence(&sentence, &it_ctx()).unwrap();
    let items = chat_strings(&mors);
    assert!(
        items[2].starts_with("v|aprire") || items[2].starts_with("verb|aprire"),
        "Defect 8 NOUN variant: expected verb|aprire for aprila, got {:?}",
        items[2]
    );
}

#[test]
fn test_italian_defect8_aprili_homograph_becomes_verb() {
    let sentence = UdSentence {
        words: vec![
            it_word(1, "per", "per", UniversalPos::Adp, 3, "case", None),
            it_word(2, "favore", "favore", UniversalPos::Noun, 0, "root", None),
            it_word(3, "aprili", "aprile", UniversalPos::Noun, 2, "nmod", None),
        ],
    };
    let (mors, _) = map_ud_sentence(&sentence, &it_ctx()).unwrap();
    let items = chat_strings(&mors);
    assert!(
        items[2].starts_with("v|aprire") || items[2].starts_with("verb|aprire"),
        "Defect 8 homograph variant: expected verb|aprire for aprili, got {:?}",
        items[2]
    );
}

#[test]
fn test_italian_defect8_finila_mid_sentence_becomes_verb() {
    let sentence = UdSentence {
        words: vec![
            it_word(1, "per", "per", UniversalPos::Adp, 3, "case", None),
            it_word(2, "favore", "favore", UniversalPos::Noun, 0, "root", None),
            it_word(3, "finila", "finile", UniversalPos::Adj, 2, "amod", None),
        ],
    };
    let (mors, _) = map_ud_sentence(&sentence, &it_ctx()).unwrap();
    let items = chat_strings(&mors);
    assert!(
        items[2].starts_with("v|finire") || items[2].starts_with("verb|finire"),
        "Defect 8: expected verb|finire for finila, got {:?}",
        items[2]
    );
}

#[test]
fn test_italian_defect8_genuine_noun_stays_noun() {
    let sentence = UdSentence {
        words: vec![
            it_word(1, "il", "il", UniversalPos::Det, 2, "det", None),
            it_word(
                2,
                "cavallo",
                "cavallo",
                UniversalPos::Noun,
                0,
                "root",
                Some("Gender=Masc|Number=Sing"),
            ),
        ],
    };
    let (mors, _) = map_ud_sentence(&sentence, &it_ctx()).unwrap();
    let items = chat_strings(&mors);
    assert!(
        items[1].starts_with("n|cavallo") || items[1].starts_with("noun|cavallo"),
        "cavallo (genuine noun) must stay noun, got {:?}",
        items[1]
    );
}

#[test]
fn test_italian_defect8_genuine_adj_stays_adj() {
    let sentence = UdSentence {
        words: vec![
            it_word(1, "il", "il", UniversalPos::Det, 2, "det", None),
            it_word(2, "gatto", "gatto", UniversalPos::Noun, 0, "root", None),
            it_word(
                3,
                "piccolo",
                "piccolo",
                UniversalPos::Adj,
                2,
                "amod",
                Some("Gender=Masc|Number=Sing"),
            ),
        ],
    };
    let (mors, _) = map_ud_sentence(&sentence, &it_ctx()).unwrap();
    let items = chat_strings(&mors);
    // `piccolo` is NOT in the compound-imperative allowlist
    // (it's in the Defect 6 allowlist, but only when arriving
    // via a Range). As a Single UdWord it should pass through
    // the normal adj mapping.
    assert!(
        items[2].starts_with("adj|piccolo"),
        "piccolo as Single UdWord should stay adj|piccolo, got {:?}",
        items[2]
    );
}

#[test]
fn test_italian_defect9_dagliela_head_rewritten_to_verb_dare() {
    let sentence = UdSentence {
        words: vec![
            it_range(1, 3, "dagliela"),
            it_word(1, "da", "da", UniversalPos::Adp, 0, "root", None),
            it_word(
                2,
                "glie",
                "gli",
                UniversalPos::Pron,
                1,
                "iobj",
                Some("Gender=Masc|Number=Sing|Person=3|PronType=Prs"),
            ),
            it_word(
                3,
                "la",
                "la",
                UniversalPos::Pron,
                1,
                "obj",
                Some("Gender=Fem|Number=Sing|Person=3|PronType=Prs"),
            ),
        ],
    };
    let (mors, _gras) = map_ud_sentence(&sentence, &it_ctx()).unwrap();
    let items = chat_strings(&mors);
    assert_eq!(
        items.len(),
        1,
        "Expected 1 mor for 3-piece MWT, got {items:?}"
    );
    assert!(
        items[0].starts_with("v|dare") || items[0].starts_with("verb|dare"),
        "Defect 9: expected verb|dare head for dagliela, got {:?}",
        items[0]
    );
    assert!(
        !items[0].starts_with("adp|da") && !items[0].starts_with("prep|da"),
        "Defect 9: head must not stay ADP, got {:?}",
        items[0]
    );
    // The clitic decomposition must be preserved — this is what
    // distinguishes Defect 9 from Defect 6 (which collapses to a
    // single chunk).
    assert!(
        items[0].contains("~"),
        "Defect 9: 3-piece MWT decomposition must be preserved \
         (expected clitic-suffixed Mor), got {:?}",
        items[0]
    );
}

#[test]
fn test_italian_defect10_posala_head_lemma_rewritten_to_posare() {
    let sentence = UdSentence {
        words: vec![
            it_range(1, 2, "posala"),
            it_word(
                1,
                "posa",
                "posa",
                UniversalPos::Verb,
                0,
                "root",
                Some("Mood=Imp|Number=Sing|Person=2|VerbForm=Fin"),
            ),
            it_word(
                2,
                "la",
                "la",
                UniversalPos::Pron,
                1,
                "obj",
                Some("Gender=Fem|Number=Sing|Person=3|PronType=Prs"),
            ),
        ],
    };
    let (mors, _gras) = map_ud_sentence(&sentence, &it_ctx()).unwrap();
    let items = chat_strings(&mors);
    assert_eq!(items.len(), 1);
    assert!(
        items[0].starts_with("v|posare") || items[0].starts_with("verb|posare"),
        "Defect 10: expected verb|posare head for posala, got {:?}",
        items[0]
    );
    assert!(
        items[0].contains("~"),
        "Defect 10: 2-piece MWT decomposition must be preserved, got {:?}",
        items[0]
    );
}

#[test]
fn test_italian_defect10_posalo_head_lemma_rewritten_to_posare() {
    let sentence = UdSentence {
        words: vec![
            it_range(1, 2, "posalo"),
            it_word(
                1,
                "posa",
                "posa",
                UniversalPos::Verb,
                0,
                "root",
                Some("Mood=Imp|Number=Sing|Person=2|VerbForm=Fin"),
            ),
            it_word(
                2,
                "lo",
                "lo",
                UniversalPos::Pron,
                1,
                "obj",
                Some("Gender=Masc|Number=Sing|Person=3|PronType=Prs"),
            ),
        ],
    };
    let (mors, _gras) = map_ud_sentence(&sentence, &it_ctx()).unwrap();
    let items = chat_strings(&mors);
    assert_eq!(items.len(), 1);
    assert!(
        items[0].starts_with("v|posare") || items[0].starts_with("verb|posare"),
        "Defect 10: expected verb|posare head for posalo, got {:?}",
        items[0]
    );
    assert!(items[0].contains("~"));
}

#[test]
fn test_italian_digliela_stays_correctly_merged() {
    let sentence = UdSentence {
        words: vec![
            it_range(1, 3, "digliela"),
            it_word(
                1,
                "di",
                "dire",
                UniversalPos::Verb,
                0,
                "root",
                Some("Mood=Imp|Number=Sing|Person=2|VerbForm=Fin"),
            ),
            it_word(
                2,
                "glie",
                "gli",
                UniversalPos::Pron,
                1,
                "iobj",
                Some("Gender=Masc|Number=Sing|Person=3|PronType=Prs"),
            ),
            it_word(
                3,
                "la",
                "la",
                UniversalPos::Pron,
                1,
                "obj",
                Some("Gender=Fem|Number=Sing|Person=3|PronType=Prs"),
            ),
        ],
    };
    let (mors, _gras) = map_ud_sentence(&sentence, &it_ctx()).unwrap();
    let items = chat_strings(&mors);
    assert_eq!(items.len(), 1);
    assert!(
        items[0].starts_with("v|dire") || items[0].starts_with("verb|dire"),
        "digliela: expected verb|dire head (Stanza-correct), got {:?}",
        items[0]
    );
    assert!(
        items[0].contains("~"),
        "digliela: clitic decomposition must be preserved, got {:?}",
        items[0]
    );
}
