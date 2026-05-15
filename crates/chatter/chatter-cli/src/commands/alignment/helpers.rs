//! Extraction helpers for alignment tier display text.
//!
//! Each helper maps a tier-item index to a human-readable string suitable for
//! the `show-alignment` tabular output. Words use `cleaned_text()` (stripping
//! CHAT markup), while non-word content and dependent-tier items serialise via
//! `WriteChat` / `to_chat_string()` so the display stays faithful to the source.
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#Main_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>

use talkbank_model::model::{
    GraTier, MainTier, MorTier, PhoTier, SinTier, UtteranceContent, WriteChat,
};

use crate::cli::AlignmentTier;

/// Formats a human-readable label for the provided alignment tier (`%mor`, `%gra`, `%pho`, `%sin`).
///
/// Keeping these labels aligned with the CHAT manual’s Dependent Tier nomenclature makes the CLI output easier to trace.
pub(super) fn format_tier_label(tier: AlignmentTier) -> &'static str {
    match tier {
        AlignmentTier::Mor => "%mor",
        AlignmentTier::Gra => "%gra",
        AlignmentTier::Pho => "%pho",
        AlignmentTier::Sin => "%sin",
    }
}

/// Extract display text for one main-tier content index.
/// Index `content.len()` addresses the terminator token.
///
/// Words emit their `cleaned_text` while other content serializes via `write_chat` so the rendered text mirrors the
/// Main Tier description in the manual.
pub(super) fn get_main_content_text(main: &MainTier, index: usize) -> Option<String> {
    // Check if this is the terminator (index == content length)
    if index == main.content.content.len() {
        return main.content.terminator.as_ref().map(|t| t.to_chat_string());
    }

    main.content.content.get(index).map(|content| {
        match content {
            // For words, use cleaned_text to show the actual word
            UtteranceContent::Word(w) => w.cleaned_text().to_string(),
            UtteranceContent::AnnotatedWord(aw) => aw.inner.cleaned_text().to_string(),
            UtteranceContent::ReplacedWord(rw) => rw.word.cleaned_text().to_string(),
            // For all other content, serialize to CHAT format
            _ => {
                let mut s = String::new();
                content.write_chat(&mut s).ok();
                s
            }
        }
    })
}

/// Extract serialized text for one `%mor` item.
///
/// Shows `%mor` item tokens in the exact format the manual lists (e.g., `word+tag`), keeping the alignment view faithful to the specification.
pub(super) fn get_mor_item_text(mor: &MorTier, index: usize) -> Option<String> {
    mor.items().get(index).map(|item| item.to_chat_string())
}

/// Extract serialized text for one `%gra` relation.
///
/// Grammatical relations appear as `head|dependent|REL` per the CHAT manual, and `to_chat_string` preserves that form.
pub(super) fn get_gra_relation_text(gra: &GraTier, index: usize) -> Option<String> {
    gra.relations().get(index).map(|rel| rel.to_chat_string())
}

/// Extract serialized text for one `%pho` item.
///
/// Keeps the phonetic transcription intact so the renderers match the manual’s presentation of `%pho`.
pub(super) fn get_pho_form_text(pho: &PhoTier, index: usize) -> Option<String> {
    pho.items.get(index).map(|item| item.to_chat_string())
}

/// Extract serialized text for one `%sin` item.
///
/// Uses the canonical `g:lexeme:dpoint` style described in the CHAT manual.
pub(super) fn get_sin_item_text(sin: &SinTier, index: usize) -> Option<String> {
    sin.items.get(index).map(|item| item.to_chat_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use talkbank_model::model::{
        GraTier, GrammaticalRelation, MainTier, MorTier, PhoItem, PhoTier, SinItem, SinTier,
        SinToken,
    };

    #[test]
    fn format_tier_label_maps_cli_tiers() {
        assert_eq!(format_tier_label(AlignmentTier::Mor), "%mor");
        assert_eq!(format_tier_label(AlignmentTier::Gra), "%gra");
        assert_eq!(format_tier_label(AlignmentTier::Pho), "%pho");
        assert_eq!(format_tier_label(AlignmentTier::Sin), "%sin");
    }

    #[test]
    fn extraction_helpers_return_none_for_out_of_bounds() {
        let main = MainTier::new("CHI", vec![], None);
        let mor = MorTier::new_mor(
            vec![],
            talkbank_model::Terminator::Period {
                span: talkbank_model::Span::DUMMY,
            },
        );
        let gra = GraTier::new_gra(vec![]);
        let pho = PhoTier::new_pho(vec![]);
        let sin = SinTier::new(vec![]);

        assert_eq!(get_main_content_text(&main, 0), None);
        assert_eq!(get_mor_item_text(&mor, 0), None);
        assert_eq!(get_gra_relation_text(&gra, 0), None);
        assert_eq!(get_pho_form_text(&pho, 0), None);
        assert_eq!(get_sin_item_text(&sin, 0), None);
    }

    #[test]
    fn extraction_helpers_return_serialized_values() {
        let gra = GraTier::new_gra(vec![GrammaticalRelation::new(1, 2, "SUBJ")]);
        let pho = PhoTier::new_pho(vec![PhoItem::Word("hɛˈloʊ".into())]);
        let sin = SinTier::new(vec![SinItem::Token(SinToken::new_unchecked(
            "g:ball:dpoint",
        ))]);

        assert_eq!(get_gra_relation_text(&gra, 0), Some("1|2|SUBJ".to_string()));
        assert_eq!(get_pho_form_text(&pho, 0), Some("hɛˈloʊ".to_string()));
        assert_eq!(
            get_sin_item_text(&sin, 0),
            Some("g:ball:dpoint".to_string())
        );
    }
}
