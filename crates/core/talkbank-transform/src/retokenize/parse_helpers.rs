//! Small helpers for parsing NLP tokens into CHAT AST nodes.

use talkbank_model::NullErrorSink;
use talkbank_model::model::{BracketedItem, UtteranceContent, Word};

use crate::extract::ExtractedWord;

/// Try to parse a token into an `UtteranceContent` item.
pub fn try_parse_token_as_utterance_content(
    parser: &talkbank_parser::TreeSitterParser,
    text: &str,
    expected_terminator: Option<&str>,
    diagnostics: &mut Vec<String>,
) -> Option<UtteranceContent> {
    use talkbank_model::Span;
    use talkbank_model::model::Separator;

    match text {
        "," => Some(UtteranceContent::Separator(Separator::Comma {
            span: Span::DUMMY,
        })),
        "\u{201E}" => Some(UtteranceContent::Separator(Separator::Tag {
            span: Span::DUMMY,
        })),
        "\u{2021}" => Some(UtteranceContent::Separator(Separator::Vocative {
            span: Span::DUMMY,
        })),
        _ => {
            if handle_ending_punct_skip(text, expected_terminator, diagnostics) {
                None
            } else {
                try_parse_token_as_word(parser, text, diagnostics)
                    .map(|word| UtteranceContent::Word(Box::new(word)))
            }
        }
    }
}

/// Try to parse a token into a `BracketedItem`.
pub fn try_parse_token_as_bracketed_item(
    parser: &talkbank_parser::TreeSitterParser,
    text: &str,
    expected_terminator: Option<&str>,
    diagnostics: &mut Vec<String>,
) -> Option<BracketedItem> {
    use talkbank_model::Span;
    use talkbank_model::model::Separator;

    match text {
        "," => {
            return Some(BracketedItem::Separator(Separator::Comma {
                span: Span::DUMMY,
            }));
        }
        "\u{201E}" => {
            return Some(BracketedItem::Separator(Separator::Tag {
                span: Span::DUMMY,
            }));
        }
        "\u{2021}" => {
            return Some(BracketedItem::Separator(Separator::Vocative {
                span: Span::DUMMY,
            }));
        }
        _ => {}
    }

    if handle_ending_punct_skip(text, expected_terminator, diagnostics) {
        return None;
    }

    try_parse_token_as_word(parser, text, diagnostics)
        .map(|word| BracketedItem::Word(Box::new(word)))
}

/// Returns true if `text` is a tag-marker separator character.
pub fn is_tag_marker_text(text: &str) -> bool {
    matches!(text, "," | "\u{201E}" | "\u{2021}")
}

/// Returns true if `text` is a CHAT utterance terminator.
pub fn is_ending_punct(text: &str) -> bool {
    talkbank_model::model::content::Terminator::is_chat_terminator(text)
}

/// Return true when the token is an ending punctuation symbol that should be
/// skipped during retokenization.
pub fn handle_ending_punct_skip(
    text: &str,
    expected_terminator: Option<&str>,
    diagnostics: &mut Vec<String>,
) -> bool {
    if !is_ending_punct(text) {
        return false;
    }

    if let Some(expected) = expected_terminator
        && text != expected
    {
        diagnostics.push(format!(
            "skipped Stanza terminator {text:?} does not match existing terminator {expected:?}; keeping existing terminator"
        ));
    }
    true
}

/// Try to parse a token as a valid CHAT word.
pub fn try_parse_token_as_word(
    parser: &talkbank_parser::TreeSitterParser,
    text: &str,
    diagnostics: &mut Vec<String>,
) -> Option<Word> {
    let errors = NullErrorSink;
    match parser.parse_word_fragment(text, 0, &errors).into_option() {
        Some(word) => Some(word),
        None => {
            tracing::warn!(
                token = text,
                "Stanza token is not valid CHAT word syntax; keeping original word"
            );
            diagnostics.push(format!(
                "token {text:?} is not valid CHAT word syntax; keeping original word"
            ));
            None
        }
    }
}

/// Resolve the token text for a token, handling xbxxx restoration.
pub fn resolve_token_text(
    stanza_text: &str,
    orig_word_idx: usize,
    original_words: &[ExtractedWord],
) -> String {
    if stanza_text == "xbxxx"
        && let Some(word) = original_words.get(orig_word_idx)
        && (word.form_type.is_some() || word.lang.is_some())
    {
        return word.text.as_str().to_string();
    }
    stanza_text.to_string()
}
