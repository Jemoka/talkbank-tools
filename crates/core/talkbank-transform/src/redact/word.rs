//! Word-level sanitization: replace `WordContent::Text` and `Shortening`
//! segments with deterministic placeholders, preserve all other
//! structural elements verbatim.
//!
//! `WriteChat for Word` ignores `Word.raw_text` and serializes from
//! `Word.content`, so mutation must happen on the structured content,
//! not on `raw_text`. `raw_text` is updated alongside for downstream
//! JSON consumers (the Serialize impl emits raw_text directly).

use smol_str::SmolStr;
use talkbank_model::{Word, WordContent, WordShortening, WordText, WriteChat};

use super::placeholder::{PlaceholderState, PlaceholderToken};

/// Sanitizes a single `Word` in place.
///
/// Untranscribed markers (`xxx`/`yyy`/`www`) are passed through
/// unchanged — replacing them changes their semantic meaning.
pub(crate) fn sanitize_word(word: &mut Word, state: &mut PlaceholderState) {
    if word.untranscribed().is_some() {
        return;
    }

    let placeholder = PlaceholderToken::word(state.next());
    let mut modified = false;
    for i in 0..word.content.len() {
        match &word.content[i] {
            WordContent::Text(_) => {
                let new_text = WordText::new_unchecked(placeholder.as_str());
                word.content.replace_at(i, WordContent::Text(new_text));
                modified = true;
            }
            WordContent::Shortening(_) => {
                let new_short = WordShortening::new_unchecked("x");
                word.content
                    .replace_at(i, WordContent::Shortening(new_short));
                modified = true;
            }
            // Structural / prosodic markers — preserved verbatim. Listed
            // explicitly (not `_ => {}`) so a new WordContent variant fails
            // to compile here, forcing an explicit redact-vs-preserve
            // decision for any future leaf type.
            WordContent::OverlapPoint(_)
            | WordContent::CAElement(_)
            | WordContent::CADelimiter(_)
            | WordContent::StressMarker(_)
            | WordContent::Lengthening(_)
            | WordContent::SyllablePause(_)
            | WordContent::UnderlineBegin(_)
            | WordContent::UnderlineEnd(_)
            | WordContent::CompoundMarker(_)
            | WordContent::CliticBoundary(_) => {}
        }
    }

    if !modified {
        return;
    }

    let mut buffer = String::new();
    for item in word.content.iter() {
        let _ = item.write_chat(&mut buffer);
    }
    word.set_raw_text(SmolStr::new(&buffer));
}
