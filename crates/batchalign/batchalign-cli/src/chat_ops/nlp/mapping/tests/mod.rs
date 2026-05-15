//! Unit tests for the UD-to-CHAT mapping layer, partitioned by feature
//! so each child file fits the workspace's ≤800 LOC hard limit. Most of
//! the implementation tested here lives in
//! `talkbank-transform::morphosyntax`; this layer wires Batchalign's
//! orchestration around it.
//!
//! Shared helpers (`map_ud_sentence_to_mors`, the Italian fixture
//! constructors `it_word` / `it_range` / `it_ctx` / `chat_strings`,
//! the `ExpectedPos` predicate enum, and
//! `assert_defect6_2component_collapses`) live here as `pub(super)`
//! items so each child file imports them via `use super::*;` without
//! redundant copies.

#![allow(unused_imports, dead_code)]

use crate::chat_ops::nlp::map_ud_word_to_mor;
use crate::chat_ops::nlp::mapping::*;
use crate::chat_ops::nlp::{UdId, UdPunctable, UdSentence, UdWord, UniversalPos};
use talkbank_model::WriteChat;
use talkbank_model::model::dependent_tier::mor::Mor;

mod alignment_and_errors;
mod core_mapping;
mod italian_collapse;
mod italian_defects;
mod lang_de_es_he;
mod lang_french_japanese;
mod lang_misc_and_defaults;
mod pos_variants;

pub(super) fn map_ud_sentence_to_mors(sentence: &UdSentence, ctx: &MappingContext) -> Vec<Mor> {
    let (mors, _) = map_ud_sentence(sentence, ctx).unwrap_or_default();
    mors
}

pub(super) fn it_range(start: usize, end: usize, text: &str) -> UdWord {
    UdWord {
        id: UdId::Range(start, end),
        text: text.into(),
        lemma: "".into(),
        upos: UdPunctable::Value(UniversalPos::X),
        xpos: None,
        feats: None,
        head: 0,
        deprel: "dep".into(),
        deps: None,
        misc: None,
    }
}

pub(super) fn it_word(
    id: usize,
    text: &str,
    lemma: &str,
    upos: UniversalPos,
    head: usize,
    deprel: &str,
    feats: Option<&str>,
) -> UdWord {
    UdWord {
        id: UdId::Single(id),
        text: text.into(),
        lemma: lemma.into(),
        upos: UdPunctable::Value(upos),
        xpos: None,
        feats: feats.map(|f| f.to_string()),
        head,
        deprel: deprel.into(),
        deps: None,
        misc: None,
    }
}

pub(super) fn it_ctx() -> MappingContext {
    MappingContext {
        lang: talkbank_model::model::LanguageCode::new("ita"),
    }
}

pub(super) fn chat_strings(mors: &[Mor]) -> Vec<String> {
    mors.iter()
        .map(|m| {
            let mut s = String::new();
            m.write_chat(&mut s).unwrap();
            s
        })
        .collect()
}

#[derive(Clone, Copy)]
pub(super) enum ExpectedPos {
    Noun,
    Adj,
}

impl ExpectedPos {
    fn matches_prefix(self, item: &str, lemma: &str) -> bool {
        match self {
            ExpectedPos::Noun => {
                item.starts_with(&format!("n|{}", lemma))
                    || item.starts_with(&format!("noun|{}", lemma))
            }
            ExpectedPos::Adj => item.starts_with(&format!("adj|{}", lemma)),
        }
    }
}

pub(super) fn assert_defect6_2component_collapses(
    surface: &str,
    comp1: (&str, &str),
    comp2: (&str, &str),
    expected_pos: ExpectedPos,
    expected_lemma: &str,
) {
    let sentence = UdSentence {
        words: vec![
            it_range(1, 2, surface),
            it_word(1, comp1.0, comp1.1, UniversalPos::Verb, 0, "root", None),
            it_word(2, comp2.0, comp2.1, UniversalPos::Pron, 1, "obj", None),
        ],
    };
    let (mors, _) = map_ud_sentence(&sentence, &it_ctx()).unwrap();
    let items = chat_strings(&mors);
    assert_eq!(
        items.len(),
        1,
        "surface {surface:?}: expected 1 mor, got {items:?}"
    );
    assert!(
        expected_pos.matches_prefix(&items[0], expected_lemma),
        "surface {surface:?}: expected prefix for lemma {expected_lemma:?}, got {:?}",
        items[0]
    );
    assert!(
        !items[0].contains("~"),
        "surface {surface:?}: collapsed Mor must not carry clitic suffix, got {:?}",
        items[0]
    );
}
