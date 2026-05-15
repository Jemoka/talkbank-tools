//! Syntax highlighting for CHAT format transcripts.
//!
//! `talkbank-highlight` turns CHAT text into a flat sequence of typed, byte-offset
//! tokens suitable for colorizing editors and terminal output. It sits on top of
//! [`tree-sitter-highlight`] and the [`tree-sitter-talkbank`] grammar, so every
//! highlight decision is driven by the same concrete syntax tree used for parsing.
//!
//! # Architecture
//!
//! ```text
//! ┌──────────────┐     ┌─────────────────────────┐     ┌──────────────┐
//! │ CHAT source  │────▶│ tree-sitter-highlight   │────▶│ Vec<Highlight│
//! │  (UTF-8)     │     │ + highlights.scm queries │     │    Token>    │
//! └──────────────┘     └─────────────────────────┘     └──────────────┘
//! ```
//!
//! [`HighlightConfig`] encapsulates the tree-sitter language, the
//! `highlights.scm` query file (bundled at compile time via `include_str!`),
//! and a reusable [`Highlighter`] instance.  Calling [`HighlightConfig::highlight`]
//! walks the capture stream and produces a `Vec<`[`HighlightToken`]`>` -- one entry
//! per contiguous source range that falls under a named capture.
//!
//! # Token types
//!
//! Every token is classified into one of the [`TokenType`] variants. The table
//! below shows the mapping from CHAT constructs to token types:
//!
//! | `TokenType`        | CHAT construct                                   | Example              |
//! |--------------------|--------------------------------------------------|----------------------|
//! | `Keyword`          | Structural headers                               | `@UTF8`, `@Begin`    |
//! | `KeywordDirective` | Metadata headers                                 | `@Participants`, `@ID` |
//! | `Variable`         | Speaker codes on main tiers                      | `*CHI:`, `*MOT:`     |
//! | `String`           | Words on the main tier                            | `hello`, `go`        |
//! | `StringSpecial`    | Replacement text `[: ...]` and alternatives       | `[: went]`           |
//! | `Comment`          | Comment tiers, pauses, explanations              | `%com:`, `(.)`, `[= ...]` |
//! | `Tag`              | Events and actions                                | `&=laughs`, `[* ...]` |
//! | `Function`         | Action/coding dependent tiers                    | `%act:`, `%cod:`     |
//! | `Type`             | Structured dependent tier names, morphology words | `%mor:`, `%gra:`     |
//! | `TypeBuiltin`      | Morphological POS tags and features               | `VERB`, `-PAST`      |
//! | `Operator`         | Scoped symbols and postcodes                     | `@s`, `@l`, `[!]`   |
//! | `Number`           | Numeric values and timing bullets                | `12345`, bullet marks |
//! | `Punctuation`      | Utterance terminators and delimiters             | `.`, `?`, `!`        |
//! | `Error`            | Error annotations and error tiers                | `[* s:r]`, `%err:`   |
//!
//! # LSP integration
//!
//! The primary consumer is the `chatter-lsp` crate, which wraps this library in
//! a [`SemanticTokensProvider`](https://docs.rs/tower-lsp) that converts
//! [`HighlightToken`] spans into delta-encoded LSP semantic tokens. The LSP
//! legend maps each [`TokenType`] to a standard (or custom) LSP
//! `SemanticTokenType`, so VS Code and other editors receive syntax colors that
//! are consistent with the CHAT manual's lexical categories.
//!
//! # Examples
//!
//! Highlight a minimal CHAT transcript and inspect the resulting tokens:
//!
//! ```
//! use chatter_lsp::highlight::{HighlightConfig, TokenType};
//!
//! let mut config = HighlightConfig::new().expect("grammar loaded");
//!
//! let chat = "@UTF8\n@Begin\n*CHI:\thello .\n@End\n";
//! let tokens = config.highlight(chat).expect("highlight succeeded");
//!
//! // The first token covers the @UTF8 header keyword.
//! assert_eq!(tokens[0].token_type, TokenType::Keyword);
//! assert!(chat[tokens[0].start..tokens[0].end].contains("@UTF8"));
//!
//! // Every token carries byte offsets into the original source.
//! for tok in &tokens {
//!     let text = &chat[tok.start..tok.end];
//!     println!("{:?}  {:>3}..{:<3}  {:?}", tok.token_type, tok.start, tok.end, text);
//! }
//! ```
//!
//! # Module map
//!
//! This crate is a single module (`lib.rs`) exporting three public items:
//!
//! - [`TokenType`] -- enum of highlight classifications
//! - [`HighlightToken`] -- a typed span `(start, end, token_type)`
//! - [`HighlightConfig`] -- grammar + query wrapper with [`highlight()`](HighlightConfig::highlight)
//!
//! The `queries/highlights.scm` file (included at compile time) contains the
//! tree-sitter capture patterns that drive classification. It covers headers,
//! main tier words, dependent tiers, morphology elements, grammar relations,
//! terminators, and overlap markers.
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Headers>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Main_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>

use tree_sitter_highlight::{HighlightConfiguration, HighlightEvent, Highlighter};

/// Token types that map to both LSP and GUI themes and align to CHAT constructs.
///
/// Each variant represents a high-level CHAT concept the manual highlights: headers (`Keyword`/`KeywordDirective`),
/// utterance words (`String`), actions/events, terminators, and errors. This label set feeds both the LSP semantic
/// legend and GUI theming so editors highlight the same structures described in the CHAT manual.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TokenType {
    /// Keywords: @UTF8, @Begin, @End
    Keyword,
    /// Header directives: @Participants, @ID, etc.
    KeywordDirective,
    /// Speaker codes
    Variable,
    /// Words in main tier
    String,
    /// Replacement text
    StringSpecial,
    /// Comments and pauses
    Comment,
    /// Events [* ...]
    Tag,
    /// Actions &=...
    Function,
    /// Tier names (%mor, %gra)
    Type,
    /// Mor categories (det, n, v)
    TypeBuiltin,
    /// Scoped symbols @s, @l
    Operator,
    /// Numbers
    Number,
    /// Terminators
    Punctuation,
    /// Error annotations
    Error,
}

/// Highlight token with start/end offsets and a typed classification.
///
/// LSP and GUI consumers use this struct to render CHAT tokens without reparsing the grammar, keeping
/// highlights and editor tooling aligned with the manual’s lexical categories.
#[derive(Debug, Clone)]
pub struct HighlightToken {
    /// Start byte offset
    pub start: usize,
    /// End byte offset
    pub end: usize,
    /// Token type
    pub token_type: TokenType,
}

/// Highlight configuration that loads tree-sitter queries and exposes semantic tokens.
///
/// This struct wraps the `HighlightConfiguration` for the CHAT grammar so the CLI consistently
/// produces the same capture names referenced in the manual’s syntax sections. It also holds the
/// `Highlighter` instance required by both the LSP and GUI.
pub struct HighlightConfig {
    config: HighlightConfiguration,
    highlighter: Highlighter,
}

impl HighlightConfig {
    /// Create a new highlight configuration for CHAT syntax highlighting.
    ///
    /// Loads the `tree-sitter-talkbank` language and the `queries/highlights.scm` captures so the
    /// resulting highlight set mirrors the categories discussed in the manual (headers, tiers, events).
    pub fn new() -> Result<Self, String> {
        // Load tree-sitter-talkbank language
        let language = tree_sitter_talkbank::LANGUAGE.into();

        // Load highlights.scm from the grammar repository
        let highlights_query = include_str!("../queries/highlights.scm");

        let mut config = HighlightConfiguration::new(
            language,
            "talkbank",
            highlights_query,
            "", // no injections
            "", // no locals
        )
        .map_err(|e| format!("Failed to load highlight config: {:?}", e))?;

        // Map capture names to indices (must match order in highlights.scm)
        config.configure(&[
            "keyword",
            "keyword.directive",
            "variable.builtin",
            "string",
            "string.special",
            "comment",
            "comment.block",
            "tag",
            "function",
            "function.method",
            "type",
            "type.builtin",
            "type.qualifier",
            "operator",
            "number",
            "punctuation.special",
            "punctuation.delimiter",
            "punctuation.bracket",
            "string.delimiter",
            "error",
            "constant",
            "constant.language",
            "attribute",
            "keyword.control",
        ]);

        Ok(Self {
            config,
            highlighter: Highlighter::new(),
        })
    }

    /// Highlight CHAT text and return the resulting tokens.
    ///
    /// Walks the capture stream produced by tree-sitter-highlight, maps capture indices to `TokenType`,
    /// and tracks the active capture stack so the result mirrors the concrete syntax the CHAT manual
    /// describes (keywords, headers, events, overlapped tokens, etc.).
    ///
    /// # Arguments
    /// * `text` - The CHAT format text to highlight
    ///
    /// # Returns
    /// Vector of highlight tokens with positions and types
    pub fn highlight(&mut self, text: &str) -> Result<Vec<HighlightToken>, String> {
        let highlights = self
            .highlighter
            .highlight(&self.config, text.as_bytes(), None, |_| None)
            .map_err(|e| format!("Highlight failed: {:?}", e))?;

        let mut tokens = Vec::new();
        let mut highlight_stack: Vec<TokenType> = Vec::new();

        for event in highlights {
            match event.map_err(|e| format!("Event error: {:?}", e))? {
                HighlightEvent::Source { start, end } => {
                    // If we're inside a highlight, emit a token for this source range
                    if let Some(&token_type) = highlight_stack.last() {
                        tokens.push(HighlightToken {
                            start,
                            end,
                            token_type,
                        });
                    }
                }

                HighlightEvent::HighlightStart(capture) => {
                    let token_type = Self::capture_to_token_type(capture.0);
                    highlight_stack.push(token_type);
                }

                HighlightEvent::HighlightEnd => {
                    highlight_stack.pop();
                }
            }
        }

        Ok(tokens)
    }

    /// Map a tree-sitter capture index to the `TokenType` that tracks CHAT syntax categories.
    ///
    /// The capture order matches the categories described in `queries/highlights.scm` (keywords, directives,
    /// words, punctuation, errors), so this translation maps them to the enum used by both GUI and LSP.
    fn capture_to_token_type(capture: usize) -> TokenType {
        match capture {
            0 => TokenType::Keyword,
            1 => TokenType::KeywordDirective,
            2 => TokenType::Variable,
            3 => TokenType::String,
            4 => TokenType::StringSpecial,
            5 | 6 => TokenType::Comment, // comment, comment.block
            7 => TokenType::Tag,
            8 | 9 => TokenType::Function, // function, function.method
            10 => TokenType::Type,
            11 | 12 => TokenType::TypeBuiltin, // type.builtin, type.qualifier
            13 => TokenType::Operator,
            14 => TokenType::Number,
            15..=18 => TokenType::Punctuation, // punctuation variants
            19 => TokenType::Error,
            20 | 21 => TokenType::String, // constant variants
            22 => TokenType::Operator,    // attribute
            23 => TokenType::Keyword,     // keyword.control
            _ => TokenType::String,       // fallback
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Tests highlight config creation.
    #[test]
    fn test_highlight_config_creation() {
        let result = HighlightConfig::new();
        assert!(
            result.is_ok(),
            "Failed to create HighlightConfig: {:?}",
            result.err()
        );
    }

    /// Tests basic highlighting.
    #[test]
    fn test_basic_highlighting() -> Result<(), String> {
        let mut config = HighlightConfig::new()
            .map_err(|err| format!("Failed to create HighlightConfig: {err}"))?;

        let text = "@UTF8\n@Begin\n*CHI:\thello .\n@End\n";
        let tokens = config
            .highlight(text)
            .map_err(|err| format!("Failed to highlight text: {err}"))?;

        // Should have some tokens
        assert!(!tokens.is_empty(), "No tokens generated");

        // Check that we have different token types
        let mut token_types = std::collections::HashSet::new();
        for token in &tokens {
            token_types.insert(token.token_type);
        }

        assert!(token_types.len() > 1, "Should have multiple token types");
        Ok(())
    }
}
