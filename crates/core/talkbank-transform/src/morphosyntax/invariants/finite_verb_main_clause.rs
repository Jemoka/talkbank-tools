//! Finite-verb-requirement rescue for English copula-progressive constructions.

use crate::morphosyntax::{
    DepRel, FINITE_COPULA_PRES_3SG, PRESENT_PARTICIPLE, UdId, UdPunctable, UdSentence, UdWord,
    UniversalPos,
};

/// English-specific rewrite. Returns a (possibly-rewritten) clone of the input.
pub fn rescue_english_copula_progressive(sentence: &UdSentence) -> UdSentence {
    let Some(plan) = detect_rescue(sentence) else {
        return sentence.clone();
    };

    let mut rewritten = sentence.clone();
    apply_rescue(&mut rewritten, &plan);
    rewritten
}

#[derive(Debug, Clone, Copy)]
struct RescuePlan {
    part_id: usize,
    possessor_id: usize,
    verb_id: usize,
    old_root_id: usize,
}

fn detect_rescue(sentence: &UdSentence) -> Option<RescuePlan> {
    if sentence.words.iter().any(UdWord::has_finite_verb_form) {
        return None;
    }

    let has_range = sentence
        .words
        .iter()
        .any(|w| matches!(w.id, UdId::Range(_, _)));
    if !has_range {
        return None;
    }

    let part = sentence.words.iter().find(|w| is_possessive_part(w))?;
    let possessor_id = part.head;
    if possessor_id == 0 {
        return None;
    }
    let possessor = find_word_by_single_id(sentence, possessor_id)?;
    let possessor_upos = match &possessor.upos {
        UdPunctable::Value(u) => *u,
        UdPunctable::Punct(_) => return None,
    };
    if !matches!(possessor_upos, UniversalPos::Noun | UniversalPos::Propn) {
        return None;
    }
    if possessor.dep_rel() != DepRel::NmodPoss {
        return None;
    }

    let mut ing_candidates = sentence.words.iter().filter(|w| is_ing_noun(w));
    let verb = ing_candidates.next()?;
    if ing_candidates.next().is_some() {
        return None;
    }
    let verb_id = match verb.id {
        UdId::Single(n) => n,
        _ => return None,
    };

    let root = sentence
        .words
        .iter()
        .find(|w| w.head == 0 && w.dep_rel() == DepRel::Root)?;
    let old_root_id = match root.id {
        UdId::Single(n) => n,
        _ => return None,
    };

    let part_id = match part.id {
        UdId::Single(n) => n,
        _ => return None,
    };

    Some(RescuePlan {
        part_id,
        possessor_id,
        verb_id,
        old_root_id,
    })
}

fn apply_rescue(sentence: &mut UdSentence, plan: &RescuePlan) {
    let RescuePlan {
        part_id,
        possessor_id,
        verb_id,
        old_root_id,
    } = *plan;

    for word in &mut sentence.words {
        let id_n = match word.id {
            UdId::Single(n) => n,
            _ => continue,
        };

        if id_n == part_id {
            word.upos = UdPunctable::Value(UniversalPos::Aux);
            word.lemma = "be".to_string();
            word.xpos = Some("VBZ".to_string());
            word.feats = Some(FINITE_COPULA_PRES_3SG.to_string());
            word.deprel = DepRel::Aux.as_str().to_string();
            word.head = verb_id;
        } else if id_n == possessor_id {
            word.deprel = DepRel::NSubj.as_str().to_string();
            word.head = verb_id;
        } else if id_n == verb_id {
            word.upos = UdPunctable::Value(UniversalPos::Verb);
            word.xpos = Some("VBG".to_string());
            word.feats = Some(PRESENT_PARTICIPLE.to_string());
            word.deprel = DepRel::Root.as_str().to_string();
            word.head = 0;
        } else if id_n == old_root_id && old_root_id != verb_id {
            word.deprel = DepRel::Obj.as_str().to_string();
            word.head = verb_id;
        } else if word.head == old_root_id
            && old_root_id != verb_id
            && matches!(
                word.dep_rel(),
                DepRel::Cc | DepRel::Punct | DepRel::Discourse | DepRel::Mark,
            )
        {
            word.head = verb_id;
        }
    }
}

fn is_possessive_part(word: &UdWord) -> bool {
    matches!(&word.upos, UdPunctable::Value(UniversalPos::Part))
        && word.lemma == "'s"
        && word.dep_rel() == DepRel::Case
}

fn is_ing_noun(word: &UdWord) -> bool {
    matches!(&word.upos, UdPunctable::Value(UniversalPos::Noun)) && ends_with_ing(&word.text)
}

fn ends_with_ing(text: &str) -> bool {
    text.len() >= 4 && text.to_ascii_lowercase().ends_with("ing")
}

fn find_word_by_single_id(sentence: &UdSentence, id: usize) -> Option<&UdWord> {
    sentence
        .words
        .iter()
        .find(|w| matches!(w.id, UdId::Single(n) if n == id))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn word(
        id: UdId,
        text: &str,
        lemma: &str,
        upos: UniversalPos,
        feats: Option<&str>,
        head: usize,
        deprel: &str,
    ) -> UdWord {
        UdWord {
            id,
            text: text.to_string(),
            lemma: lemma.to_string(),
            upos: UdPunctable::Value(upos),
            xpos: None,
            feats: feats.map(|s| s.to_string()),
            head,
            deprel: deprel.to_string(),
            deps: None,
            misc: None,
        }
    }

    fn punct_word(id: UdId, text: &str, head: usize) -> UdWord {
        UdWord {
            id,
            text: text.to_string(),
            lemma: text.to_string(),
            upos: UdPunctable::Punct(text.to_string()),
            xpos: None,
            feats: None,
            head,
            deprel: "punct".to_string(),
            deps: None,
            misc: None,
        }
    }

    fn range_parent(start: usize, end: usize, text: &str) -> UdWord {
        UdWord {
            id: UdId::Range(start, end),
            text: text.to_string(),
            lemma: String::new(),
            upos: UdPunctable::Value(UniversalPos::X),
            xpos: None,
            feats: None,
            head: 0,
            deprel: String::new(),
            deps: None,
            misc: None,
        }
    }

    fn fixture_sink() -> UdSentence {
        UdSentence {
            words: vec![
                word(
                    UdId::Single(1),
                    "and",
                    "and",
                    UniversalPos::Cconj,
                    None,
                    5,
                    "cc",
                ),
                word(
                    UdId::Single(2),
                    "the",
                    "the",
                    UniversalPos::Det,
                    Some("Definite=Def|PronType=Art"),
                    3,
                    "det",
                ),
                range_parent(3, 4, "sink's"),
                word(
                    UdId::Single(3),
                    "sink",
                    "sink",
                    UniversalPos::Noun,
                    Some("Number=Sing"),
                    5,
                    "nmod:poss",
                ),
                word(
                    UdId::Single(4),
                    "'s",
                    "'s",
                    UniversalPos::Part,
                    None,
                    3,
                    "case",
                ),
                word(
                    UdId::Single(5),
                    "overflowing",
                    "overflow",
                    UniversalPos::Noun,
                    Some("Number=Sing"),
                    0,
                    "root",
                ),
                punct_word(UdId::Single(6), ".", 5),
            ],
        }
    }

    fn fixture_lady() -> UdSentence {
        UdSentence {
            words: vec![
                word(
                    UdId::Single(1),
                    "the",
                    "the",
                    UniversalPos::Det,
                    Some("Definite=Def|PronType=Art"),
                    2,
                    "det",
                ),
                range_parent(2, 3, "lady's"),
                word(
                    UdId::Single(2),
                    "lady",
                    "lady",
                    UniversalPos::Noun,
                    Some("Number=Sing"),
                    5,
                    "nmod:poss",
                ),
                word(
                    UdId::Single(3),
                    "'s",
                    "'s",
                    UniversalPos::Part,
                    None,
                    2,
                    "case",
                ),
                word(
                    UdId::Single(4),
                    "washing",
                    "washing",
                    UniversalPos::Noun,
                    Some("Number=Sing"),
                    5,
                    "compound",
                ),
                word(
                    UdId::Single(5),
                    "dishes",
                    "dish",
                    UniversalPos::Noun,
                    Some("Number=Plur"),
                    0,
                    "root",
                ),
                punct_word(UdId::Single(6), ".", 5),
            ],
        }
    }

    fn find_by_id(s: &UdSentence, id: usize) -> &UdWord {
        find_word_by_single_id(s, id).expect("expected Single id to exist")
    }

    fn assert_unchanged(sentence: UdSentence) {
        let out = rescue_english_copula_progressive(&sentence);
        assert_eq!(sentence, out, "expected rule to be a no-op");
    }

    #[test]
    fn sink_pattern_a_rewrite() {
        let out = rescue_english_copula_progressive(&fixture_sink());
        let s = find_by_id(&out, 4);
        assert!(matches!(s.upos, UdPunctable::Value(UniversalPos::Aux)));
        assert_eq!(s.lemma, "be");
        assert_eq!(s.deprel, "aux");
        assert_eq!(s.head, 5);
        assert!(s.feats.as_deref().unwrap().contains("VerbForm=Fin"));

        let v = find_by_id(&out, 5);
        assert!(matches!(v.upos, UdPunctable::Value(UniversalPos::Verb)));
        assert_eq!(v.deprel, "root");
        assert_eq!(v.head, 0);
        assert_eq!(v.feats.as_deref().unwrap(), "Tense=Pres|VerbForm=Part");

        let p = find_by_id(&out, 3);
        assert_eq!(p.deprel, "nsubj");
        assert_eq!(p.head, 5);
    }

    #[test]
    fn lady_pattern_b_rewrite() {
        let out = rescue_english_copula_progressive(&fixture_lady());
        let s = find_by_id(&out, 3);
        assert!(matches!(s.upos, UdPunctable::Value(UniversalPos::Aux)));
        assert_eq!(s.lemma, "be");
        assert_eq!(s.deprel, "aux");
        assert_eq!(s.head, 4);

        let v = find_by_id(&out, 4);
        assert!(matches!(v.upos, UdPunctable::Value(UniversalPos::Verb)));
        assert_eq!(v.deprel, "root");
        assert_eq!(v.head, 0);
        assert_eq!(v.feats.as_deref().unwrap(), "Tense=Pres|VerbForm=Part");

        let p = find_by_id(&out, 2);
        assert_eq!(p.deprel, "nsubj");
        assert_eq!(p.head, 4);

        let old = find_by_id(&out, 5);
        assert_eq!(old.deprel, "obj");
        assert_eq!(old.head, 4);

        let dot = find_by_id(&out, 6);
        assert_eq!(dot.head, 4);
    }

    #[test]
    fn negative_copula_adj_no_ing() {
        let s = UdSentence {
            words: vec![
                word(
                    UdId::Single(1),
                    "the",
                    "the",
                    UniversalPos::Det,
                    None,
                    2,
                    "det",
                ),
                range_parent(2, 3, "boy's"),
                word(
                    UdId::Single(2),
                    "boy",
                    "boy",
                    UniversalPos::Noun,
                    Some("Number=Sing"),
                    4,
                    "nmod:poss",
                ),
                word(
                    UdId::Single(3),
                    "'s",
                    "'s",
                    UniversalPos::Part,
                    None,
                    2,
                    "case",
                ),
                word(
                    UdId::Single(4),
                    "tall",
                    "tall",
                    UniversalPos::Adj,
                    Some("Degree=Pos"),
                    0,
                    "root",
                ),
                punct_word(UdId::Single(5), ".", 4),
            ],
        };
        assert_unchanged(s);
    }

    #[test]
    fn negative_existential_there_no_ing() {
        let s = UdSentence {
            words: vec![
                range_parent(1, 2, "there's"),
                word(
                    UdId::Single(1),
                    "there",
                    "there",
                    UniversalPos::Pron,
                    None,
                    2,
                    "expl",
                ),
                word(
                    UdId::Single(2),
                    "'s",
                    "be",
                    UniversalPos::Verb,
                    Some("Mood=Ind|Number=Sing|Person=3|Tense=Pres|VerbForm=Fin"),
                    0,
                    "root",
                ),
                word(UdId::Single(3), "a", "a", UniversalPos::Det, None, 4, "det"),
                word(
                    UdId::Single(4),
                    "cat",
                    "cat",
                    UniversalPos::Noun,
                    Some("Number=Sing"),
                    2,
                    "nsubj",
                ),
                punct_word(UdId::Single(5), ".", 2),
            ],
        };
        assert_unchanged(s);
    }

    #[test]
    fn ends_with_ing_predicate_is_conservative() {
        assert!(ends_with_ing("going"));
        assert!(ends_with_ing("WASHING"));
        assert!(!ends_with_ing("ing"));
        assert!(!ends_with_ing("dog"));
        assert!(!ends_with_ing("sinking things"));
    }
}
