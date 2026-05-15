//! Single-word UD-to-CHAT MOR mapping.

use super::features::{adj_features, det_features, noun_features, pron_features, verb_features};
use super::{
    MappingContext, MappingError, UdPunctable, UdWord, UniversalPos, japanese_verbform, lang2,
    sanitize_mor_text,
};
use smallvec::SmallVec;
use std::collections::HashMap;
use talkbank_model::model::dependent_tier::mor::{Mor, MorFeature, MorStem, MorWord, PosCategory};

/// Map a single UD word into a CHAT `%mor` item.
pub fn map_ud_word_to_mor(ud: &UdWord, ctx: &MappingContext) -> Result<Mor, MappingError> {
    if matches!(ud.lemma.as_str(), "." | "!" | "?" | "," | "$,") {
        return Ok(map_actual_punct(ud));
    }

    let feats = parse_feats(ud.feats.as_deref());
    let (mut cleaned_lemma, _is_unknown) = clean_lemma(&ud.lemma, &ud.text);
    let mut effective_pos = upos_to_name(&ud.upos).to_string();
    if lang2(&ctx.lang) == "ja"
        && let Some(ovr) = japanese_verbform(&effective_pos, &cleaned_lemma, &ud.text)
    {
        effective_pos = ovr.pos.to_string();
        cleaned_lemma = ovr.lemma.to_string();
        cleaned_lemma = cleaned_lemma.replace(',', "cm");
    }

    if lang2(&ctx.lang) == "ja" {
        if matches!(ud.upos, UdPunctable::Value(UniversalPos::Punct)) {
            effective_pos = "cm".to_string();
        }
        if ud.lemma == "、" || ud.lemma == "," {
            effective_pos = "cm".to_string();
        }
    }

    let features = compute_features(&ud.upos, &feats, &effective_pos, ud, ctx);
    let sanitized_lemma = sanitize_mor_text(&cleaned_lemma);
    if sanitized_lemma.is_empty() {
        return Err(MappingError::EmptyStem {
            word: ud.text.clone(),
            lemma: ud.lemma.clone(),
            upos: format!("{:?}", &ud.upos),
        });
    }

    let mor_word = MorWord::new(
        PosCategory::new(&effective_pos),
        MorStem::new(sanitized_lemma),
    )
    .with_features(features);
    Ok(Mor::new(mor_word))
}

fn map_actual_punct(ud: &UdWord) -> Mor {
    let (pos_name, stem) = if ud.lemma == "," || ud.lemma == "$," {
        ("cm", "cm")
    } else {
        ("punct", ud.lemma.as_str())
    };

    let mor_word = MorWord::new(PosCategory::new(pos_name), MorStem::new(stem));
    Mor::new(mor_word)
}

pub(super) fn parse_feats(feats: Option<&str>) -> HashMap<String, String> {
    let mut map = HashMap::new();
    if let Some(f) = feats {
        for pair in f.split('|') {
            let mut parts = pair.split('=');
            if let (Some(key), Some(value)) = (parts.next(), parts.next()) {
                map.insert(key.to_string(), value.to_string());
            }
        }
    }
    map
}

/// Clean a UD lemma for use as a CHAT `%mor` stem.
pub fn clean_lemma(lemma: &str, text: &str) -> (String, bool) {
    let mut target = lemma.to_string();
    let mut unknown = false;

    if target.trim() == "\u{300D}" || target.trim() == "\u{300C}" {
        target = text.to_string();
    }
    if target == "\"" {
        target = text.to_string();
    }
    if target.is_empty() {
        target = text.to_string();
    }
    target = target.replace(['\u{300D}', '\u{300C}'], "");

    if target.starts_with('0') && target.len() > 1 {
        if text.len() > 1 {
            target = text[1..].to_string();
        }
        unknown = true;
    }

    if target.contains("<SOS>") {
        target = text.to_string();
    }
    target = target.replace(['$', '.'], "");
    if target.starts_with('-') && target.len() > 1 {
        target = target[1..].to_string();
    }
    if target.ends_with('-') && target.len() > 1 {
        target = target[..target.len() - 1].to_string();
    }
    target = target.replace("--", "-");
    target = target.replace("--", "-");
    target = target.replace("<unk>", "");
    target = target.replace("<SOS>", "");
    target = target.replace("/100", "");
    target = target.replace("/r", "");
    target = target.replace([',', '\'', '~', '(', ')'], "");

    if target.contains('|') {
        target = target.split('|').next().unwrap_or("").trim().to_string();
    }

    target = target.replace(['_', '+'], "");
    if target == "door zogen" {
        target = text.to_string();
    }
    target = target.replace('-', "\u{2013}");
    if target.contains('\u{201C}') {
        target = text.to_string();
    }

    let chars: Vec<char> = target.chars().collect();
    if chars.len() >= 2
        && chars[chars.len() - 2] == '@'
        && (chars[chars.len() - 1].is_alphanumeric() || chars[chars.len() - 1] == '_')
    {
        target = chars[..chars.len() - 2].iter().collect::<String>();
    }

    target = target.trim().to_string();
    if target.is_empty() && !text.is_empty() {
        target = text.to_string();
    }
    if target.is_empty() {
        target = "x".to_string();
    }

    (target, unknown)
}

fn upos_to_name(upos: &UdPunctable<UniversalPos>) -> &'static str {
    match upos {
        UdPunctable::Value(v) => v.to_chat_pos_name(),
        UdPunctable::Punct(_) => "punct",
    }
}

pub(super) fn push_feature(features: &mut SmallVec<[MorFeature; 4]>, value: &str) {
    if !value.is_empty() {
        features.push(MorFeature::flat(value));
    }
}

pub(super) fn push_feat(
    features: &mut SmallVec<[MorFeature; 4]>,
    feats: &HashMap<String, String>,
    key: &str,
) {
    if let Some(val) = feats.get(key) {
        push_feature(features, val);
    }
}

fn compute_features(
    original_upos: &UdPunctable<UniversalPos>,
    feats: &HashMap<String, String>,
    effective_pos: &str,
    ud: &UdWord,
    ctx: &MappingContext,
) -> SmallVec<[MorFeature; 4]> {
    match original_upos {
        UdPunctable::Value(UniversalPos::Verb | UniversalPos::Aux) => {
            verb_features(feats, effective_pos, ud, ctx)
        }
        UdPunctable::Value(UniversalPos::Pron) => pron_features(feats, ud, ctx),
        UdPunctable::Value(UniversalPos::Det) => det_features(feats, ctx),
        UdPunctable::Value(UniversalPos::Adj) => adj_features(feats),
        UdPunctable::Value(UniversalPos::Noun | UniversalPos::Propn) => {
            noun_features(feats, ud, ctx)
        }
        UdPunctable::Value(UniversalPos::Sym | UniversalPos::Punct) => SmallVec::new(),
        _ => SmallVec::new(),
    }
}

/// Return `true` if the token text represents a known clitic for the given
/// language.
pub fn is_clitic(text: &str, ctx: &MappingContext) -> bool {
    match lang2(&ctx.lang) {
        "en" => text == "n't" || text == "'s" || text == "'ve" || text == "'ll",
        "fr" => text.ends_with('\'') || text == "-ce" || text == "-être" || text == "-là",
        "it" => text.ends_with('\''),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_feats_preserves_multi_value_commas() {
        let feats = parse_feats(Some("PronType=Int,Rel|Person=3"));
        assert_eq!(feats.get("PronType"), Some(&"Int,Rel".to_string()));
        assert_eq!(feats.get("Person"), Some(&"3".to_string()));
    }

    #[test]
    fn clean_lemma_falls_back_from_empty_to_text() {
        let (lemma, unknown) = clean_lemma("'", "Claus'");
        assert_eq!(lemma, "Claus'");
        assert!(!unknown);
    }

    #[test]
    fn is_clitic_dispatches_by_language() {
        let en = MappingContext {
            lang: talkbank_model::model::LanguageCode::new("eng"),
        };
        let fr = MappingContext {
            lang: talkbank_model::model::LanguageCode::new("fra"),
        };
        assert!(is_clitic("n't", &en));
        assert!(is_clitic("l'", &fr));
        assert!(!is_clitic("hello", &en));
    }
}
