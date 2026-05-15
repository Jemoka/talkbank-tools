//! `%mor` hover formatting — POS tag, lemma, and morphological features.
//!
//! Produces a human-readable breakdown of a `%mor` item: the POS tag is
//! expanded to a readable label via [`get_pos_description`](super::pos::get_pos_description),
//! the lemma is shown as-is, and fusional/derivational suffixes are listed.
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>

use super::pos::get_pos_description;
use talkbank_model::model::{Mor, MorWord};

/// Format one `%mor` item with human-readable breakdown.
///
/// Extracts POS tag, lemma, features, and clitics with proper formatting.
///
/// # Examples
///
/// - `noun|I` → "**POS**: noun (noun)\n**Lemma**: I"
/// - `verb|go-Prog` → "**POS**: verb (verb)\n**Lemma**: go\n**Features**: Prog"
/// - `pron|it~verb|be-Pres` → "**Main**: ...\n**Post-clitic**: pron|it"
pub fn format_mor_item(mor: &Mor) -> String {
    let mut parts = Vec::new();

    // Main word (always present)
    let main_str = format_mor_word_detailed(&mor.main);
    parts.push(main_str);

    // Post-clitics
    if !mor.post_clitics.is_empty() {
        let clitics: Vec<String> = mor
            .post_clitics
            .iter()
            .map(format_mor_word_simple)
            .collect();
        parts.push(format!(
            "**Post-clitic{}**: {}",
            if clitics.len() > 1 { "s" } else { "" },
            clitics.join(", ")
        ));
    }

    parts.join("\n")
}

/// Format one `MorWord` with POS, lemma, and feature details.
fn format_mor_word_detailed(word: &MorWord) -> String {
    let mut parts = Vec::new();

    // POS with description
    let pos_str = word.pos.as_ref();
    let pos_desc = get_pos_description(pos_str);
    parts.push(format!("**POS**: {} ({})", pos_str, pos_desc));

    // Lemma
    parts.push(format!("**Lemma**: {}", word.lemma));

    // Features
    if !word.features.is_empty() {
        let feature_strs: Vec<String> = word.features.iter().map(|f| f.to_string()).collect();
        parts.push(format!(
            "**Feature{}**: {}",
            if word.features.len() > 1 { "s" } else { "" },
            feature_strs.join(", ")
        ));
    }

    parts.join("\n")
}

/// Format a `MorWord` in canonical CHAT `%mor` surface form.
fn format_mor_word_simple(word: &MorWord) -> String {
    let mut result = String::new();
    let _ = word.write_chat(&mut result);
    result
}
