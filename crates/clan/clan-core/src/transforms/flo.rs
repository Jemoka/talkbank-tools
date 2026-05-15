//! FLO -- simplified fluent output.
//!
//! Reimplements CLAN's `flo` command, which generates a `%flo:` dependent tier
//! containing a simplified, "fluent" version of each utterance's main line.
//!
//! See the [CLAN manual](https://talkbank.org/0info/manuals/CLAN.html#_Toc220409312)
//! for the original command documentation.
//!
//! # Processing steps
//!
//! 1. Strips all header lines (no `@UTF8`, `@Begin`, `@End`, etc.)
//! 2. Adds a `%flo:` dependent tier to each utterance containing
//!    the simplified main line: just countable words + terminator
//! 3. Strips retrace targets (words/groups before `[/]`, `[//]`, `[///]`, `[/-]`)
//! 4. Strips non-countable words (`xxx`/`yyy`/`www`, `0word`, `&~frag`, `&-um`)
//! 5. Strips events (`&=thing`) and pauses
//! 6. For replaced words (`[: form]`), uses the replacement (corrected form)
//! 7. Keeps existing dependent tiers (`%mor`, `%gra`, etc.)
//!
//! # Differences from CLAN
//!
//! - Operates on the typed AST rather than raw text line scanning.
//! - Uses the framework transform pipeline (parse → transform → serialize).
//! - Builds `%flo:` content by walking AST nodes (retrace groups, replaced
//!   words, events, pauses) instead of regex-stripping annotation markers.
//! - Countable-word filtering uses the shared `is_countable_word()` utility
//!   rather than ad-hoc string prefix checks.

use smallvec::SmallVec;
use talkbank_model::{
    BracketedItem, ChatFile, DependentTier, Line, NonEmptyString, TextTier, UtteranceContent, Word,
    WriteChat,
};

use crate::framework::word_filter::is_countable_word;
use crate::framework::{TransformCommand, TransformError};

/// FLO transform: simplified fluent output with `%flo:` tier.
pub struct FloCommand;

impl TransformCommand for FloCommand {
    type Config = ();

    /// Strip headers and emit `%flo:` tiers with simplified lexical main-tier text.
    fn transform(&self, file: &mut ChatFile) -> Result<(), TransformError> {
        // Strip all header lines
        file.lines.retain(|line| matches!(line, Line::Utterance(_)));

        // Add %flo tier to each utterance
        for line in file.lines.iter_mut() {
            if let Line::Utterance(utterance) = line {
                // Strip preceding headers from utterances (e.g., @Comment)
                utterance.preceding_headers = SmallVec::new();

                let flo_text = build_flo_text(
                    &utterance.main.content.content,
                    utterance.main.content.terminator.as_ref(),
                );

                if let Some(content) = NonEmptyString::new(flo_text) {
                    let flo_tier = DependentTier::Flo(TextTier::new(content));

                    // Insert %flo right after the main tier (before other dependent tiers)
                    utterance.dependent_tiers.insert(0, flo_tier);
                }
            }
        }

        Ok(())
    }
}

/// Build the simplified %flo text from utterance content.
///
/// Extracts only countable words, skipping retrace targets, events,
/// pauses, and non-lexical content (unintelligible, fragments, zero words).
fn build_flo_text(
    content: &[UtteranceContent],
    terminator: Option<&talkbank_model::Terminator>,
) -> String {
    let mut words: Vec<String> = Vec::new();
    collect_flo_texts(content, &mut words);

    let mut result = words.join(" ");

    if let Some(term) = terminator {
        result.push(' ');
        let _ = term.write_chat(&mut result);
    }

    result
}

/// Collect word text strings from utterance content for FLO output.
///
/// Skips retrace targets, non-countable words, events, and pauses.
fn collect_flo_texts(content: &[UtteranceContent], out: &mut Vec<String>) {
    for item in content {
        match item {
            UtteranceContent::Word(word) if is_countable_word(word) => {
                out.push(word_display_text(word));
            }
            UtteranceContent::AnnotatedWord(annotated) if is_countable_word(&annotated.inner) => {
                out.push(word_display_text(&annotated.inner));
            }
            UtteranceContent::ReplacedWord(replaced) => {
                // Use the replacement (corrected form), not the original
                for rep in replaced.replacement.words.iter() {
                    if is_countable_word(rep) {
                        out.push(word_display_text(rep));
                    }
                }
            }
            UtteranceContent::Group(group) => {
                collect_flo_bracketed_texts(&group.content.content, out);
            }
            UtteranceContent::AnnotatedGroup(annotated) => {
                collect_flo_bracketed_texts(&annotated.inner.content.content, out);
            }
            // Retrace targets are skipped — they are false starts
            UtteranceContent::Retrace(_) => {}
            // Skip events, pauses, and all other non-word content
            _ => {}
        }
    }
}

/// Collect word texts from bracketed (group) content for FLO output.
fn collect_flo_bracketed_texts(items: &[BracketedItem], out: &mut Vec<String>) {
    for item in items {
        match item {
            BracketedItem::Word(word) if is_countable_word(word) => {
                out.push(word_display_text(word));
            }
            BracketedItem::AnnotatedWord(annotated) if is_countable_word(&annotated.inner) => {
                out.push(word_display_text(&annotated.inner));
            }
            BracketedItem::ReplacedWord(replaced) => {
                for rep in replaced.replacement.words.iter() {
                    if is_countable_word(rep) {
                        out.push(word_display_text(rep));
                    }
                }
            }
            BracketedItem::AnnotatedGroup(annotated) => {
                collect_flo_bracketed_texts(&annotated.inner.content.content, out);
            }
            // Retrace targets are skipped — they are false starts
            BracketedItem::Retrace(_) => {}
            _ => {}
        }
    }
}

/// Get the display text for a word, stripping overlap markers and CHAT annotation.
fn word_display_text(word: &Word) -> String {
    word.cleaned_text().to_owned()
}
