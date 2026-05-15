//! POS-specific feature handlers for UD-to-CHAT morphosyntax mapping.

use super::mor_word::{push_feat, push_feature};
use super::{MappingContext, UdWord, lang_en, lang_fr, lang2};
use smallvec::SmallVec;
use std::collections::HashMap;
use talkbank_model::model::dependent_tier::mor::MorFeature;

pub(super) fn verb_features(
    feats: &HashMap<String, String>,
    effective_pos: &str,
    ud: &UdWord,
    ctx: &MappingContext,
) -> SmallVec<[MorFeature; 4]> {
    if effective_pos.contains("sconj") {
        return SmallVec::new();
    }
    if ud.text == "\u{308D}" {
        return SmallVec::new();
    }
    if !effective_pos.contains("verb") && !effective_pos.contains("aux") {
        if ud.text == "\u{305F}\u{308A}" {
            let mut s = SmallVec::new();
            push_feature(&mut s, "Inf");
            push_feature(&mut s, "S");
            return s;
        }
        return SmallVec::new();
    }

    let mut suffixes = SmallVec::new();
    let verb_form = feats
        .get("VerbForm")
        .cloned()
        .unwrap_or_else(|| "Inf".to_string());
    push_feature(&mut suffixes, &verb_form);
    push_feat(&mut suffixes, feats, "Aspect");
    push_feat(&mut suffixes, feats, "Mood");
    push_feat(&mut suffixes, feats, "Tense");
    push_feat(&mut suffixes, feats, "Polarity");
    push_feat(&mut suffixes, feats, "Polite");

    if let Some(v) = feats.get("HebBinyan") {
        push_feature(&mut suffixes, &v.to_lowercase());
    }
    if let Some(v) = feats.get("HebExistential") {
        push_feature(&mut suffixes, &v.to_lowercase());
    }

    let person_raw = feats.get("Person").map(|s| s.as_str()).unwrap_or("");
    let number_raw = feats.get("Number").map(|s| s.as_str()).unwrap_or("Sing");
    let number_char = number_raw.chars().next().unwrap_or('S');
    let person_str = if person_raw == "0" { "4" } else { person_raw };
    let num_person = format!("{}{}", number_char, person_str);
    push_feature(&mut suffixes, &num_person);

    if lang2(&ctx.lang) == "en"
        && let Some(tense) = feats.get("Tense")
        && tense == "Past"
        && lang_en::is_irregular(&ud.lemma, &ud.text)
    {
        push_feature(&mut suffixes, "irr");
    }

    suffixes
}

pub(super) fn pron_features(
    feats: &HashMap<String, String>,
    ud: &UdWord,
    ctx: &MappingContext,
) -> SmallVec<[MorFeature; 4]> {
    let mut parts = Vec::new();
    let pron_type = feats.get("PronType").map(|s| s.as_str()).unwrap_or("Int");
    parts.push(pron_type.to_string());

    let case = if lang2(&ctx.lang) == "fr" {
        lang_fr::french_pronoun_case(&ud.text).to_string()
    } else {
        feats.get("Case").cloned().unwrap_or_default()
    };
    if !case.is_empty() {
        parts.push(case);
    }

    if let Some(reflex) = feats.get("Reflex")
        && reflex == "Yes"
    {
        parts.push("reflx".to_string());
    }

    if ud.text != "that" && ud.text != "who" {
        let person_raw = feats.get("Person").map(|s| s.as_str()).unwrap_or("1");
        let person_str = if person_raw == "0" { "4" } else { person_raw };
        let number = feats
            .get("Number")
            .map(|n| if n.starts_with('P') { "P" } else { "S" })
            .unwrap_or("S");
        parts.push(format!("{}{}", number, person_str));
    }

    let non_empty: Vec<&str> = parts
        .iter()
        .map(|s| s.as_str())
        .filter(|s| !s.is_empty())
        .collect();
    if non_empty.is_empty() {
        SmallVec::new()
    } else {
        let mut suffixes = SmallVec::new();
        for part in non_empty {
            push_feature(&mut suffixes, part);
        }
        suffixes
    }
}

pub(super) fn det_features(
    feats: &HashMap<String, String>,
    ctx: &MappingContext,
) -> SmallVec<[MorFeature; 4]> {
    let mut suffixes = SmallVec::new();
    let number = feats.get("Number").map(|s| s.as_str()).unwrap_or("");
    let gender_default = if lang2(&ctx.lang) == "fr" {
        if number == "Plur" { "" } else { "Masc" }
    } else {
        ""
    };
    let gender = feats
        .get("Gender")
        .cloned()
        .unwrap_or_else(|| gender_default.to_string());
    if !gender.is_empty() && gender != "Com,Neut" && gender != "Com" {
        push_feature(&mut suffixes, &gender);
    }

    let definite = feats.get("Definite").map(|s| s.as_str()).unwrap_or("Def");
    push_feature(&mut suffixes, definite);
    push_feat(&mut suffixes, feats, "PronType");
    push_feature(&mut suffixes, number);

    let np = feats
        .get("Number[psor]")
        .and_then(|s| s.chars().next())
        .map(|c| c.to_string())
        .unwrap_or_default();
    let pp = feats.get("Person[psor]").map(|s| s.as_str()).unwrap_or("");
    let psor = format!("{}{}", np, pp);
    push_feature(&mut suffixes, &psor);
    suffixes
}

pub(super) fn adj_features(feats: &HashMap<String, String>) -> SmallVec<[MorFeature; 4]> {
    let mut suffixes = SmallVec::new();
    let degree = feats.get("Degree").map(|s| s.as_str()).unwrap_or("Pos");
    if degree != "Pos" {
        push_feature(&mut suffixes, degree);
    }
    if let Some(case) = feats.get("Case") {
        push_feature(&mut suffixes, case);
    }
    let number = feats
        .get("Number")
        .and_then(|s| s.chars().next())
        .unwrap_or('S');
    let person_raw = feats.get("Person").map(|s| s.as_str()).unwrap_or("1");
    let person_str = if person_raw == "0" { "4" } else { person_raw };
    push_feature(&mut suffixes, &format!("{}{}", number, person_str));
    suffixes
}

pub(super) fn noun_features(
    feats: &HashMap<String, String>,
    ud: &UdWord,
    ctx: &MappingContext,
) -> SmallVec<[MorFeature; 4]> {
    let mut suffixes = SmallVec::new();
    let gender = feats
        .get("Gender")
        .cloned()
        .unwrap_or_else(|| "Com,Neut".to_string());
    if gender != "Com,Neut" && gender != "Com" {
        push_feature(&mut suffixes, &gender);
    }

    let number = feats.get("Number").map(|s| s.as_str()).unwrap_or("Sing");
    if number != "Sing" {
        push_feature(&mut suffixes, number);
    }

    let case = feats.get("Case").cloned().unwrap_or_else(|| {
        if ud.deprel == "obj" {
            "Acc".to_string()
        } else {
            String::new()
        }
    });
    push_feature(&mut suffixes, &case);
    push_feat(&mut suffixes, feats, "PronType");

    if lang2(&ctx.lang) == "en" && ud.text.ends_with("ing") {
        push_feature(&mut suffixes, "Ger");
    }
    if lang2(&ctx.lang) == "fr" && number == "Plur" && lang_fr::is_apm_noun(&ud.text) {
        push_feature(&mut suffixes, "Apm");
    }
    suffixes
}
