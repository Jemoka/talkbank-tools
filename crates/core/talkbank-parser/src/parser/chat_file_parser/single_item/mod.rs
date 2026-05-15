//! Single item parsing
//!
//! Convenience methods for parsing individual items: utterance, main tier, word, dependent tier.
//!
//! **Important:** the word/main-tier/utterance helpers here are synthetic
//! tree-sitter fragment paths used for legacy audit and compatibility work.
//! They should not be mistaken for honest isolated-fragment semantics.
//!
//! CHAT reference anchors:
//! - <https://talkbank.org/0info/manuals/CHAT.html#Main_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Words>

pub(crate) mod helpers;
mod parse_main_tier;
mod parse_tiers;
mod parse_utterance;
mod parse_word;
#[cfg(test)]
mod tests;

use super::TreeSitterParser;
use crate::error::ParseResult;
use crate::model::{MainTier, Utterance, Word};
use talkbank_model::model::DependentTier;

impl TreeSitterParser {
    /// Parse a single utterance (main tier plus any attached dependent tiers).
    ///
    /// The input may be either a bare utterance line (e.g., `*CHI:\thello .`) or a
    /// complete CHAT document. When the input does not look like a full file (no
    /// `@UTF8` header detected), it is wrapped in a minimal synthetic CHAT document
    /// before parsing.
    ///
    /// This makes the method explicitly synthetic on fragment input.
    ///
    /// # Parameters
    ///
    /// - `input`: A CHAT utterance string. May include dependent tiers following the
    ///   main tier line.
    ///
    /// # Returns
    ///
    /// An `Utterance` containing the parsed main tier. Dependent tiers attached to the
    /// utterance are included when present.
    ///
    /// # Errors
    ///
    /// Returns `ParseErrors` when:
    /// - Tree-sitter cannot produce a parse tree.
    /// - The main tier node contains unrecoverable CST errors.
    /// - No main tier line is found in the input.
    pub fn parse_utterance(&self, input: &str) -> ParseResult<Utterance> {
        parse_utterance::parse_utterance(self, input)
    }

    /// Parse a single main tier line (speaker code, content words, and terminator).
    ///
    /// The input is wrapped in a minimal synthetic CHAT document so tree-sitter can
    /// produce a valid CST. Spans in the returned `MainTier` are relative to the
    /// wrapper, not the original input.
    ///
    /// This is a legacy synthetic fragment path, not an honest isolated-fragment parser.
    ///
    /// # Parameters
    ///
    /// - `input`: A main tier line in CHAT format, e.g., `*CHI:\thello world .`.
    ///
    /// # Returns
    ///
    /// A `MainTier` containing the speaker, utterance content, and terminator.
    ///
    /// # Errors
    ///
    /// Returns `ParseErrors` when:
    /// - Tree-sitter fails to parse the wrapped input.
    /// - The CST contains error nodes inside the main tier.
    /// - No main tier node is found in the parse tree.
    pub fn parse_main_tier(&self, input: &str) -> ParseResult<MainTier> {
        parse_main_tier::parse_main_tier(self, input)
    }

    /// Parse a single CHAT word token, extracting it from a synthetic one-word utterance.
    ///
    /// The input word is embedded in a minimal `*CHI:\t<word> .` utterance, parsed by
    /// tree-sitter, and the first word-like content element is projected back. Inline
    /// annotations (e.g., `@b`, `@l`) attached to the word are preserved.
    ///
    /// This is a synthetic compatibility helper, not the semantic source of truth
    /// for isolated word parsing.
    ///
    /// # Parameters
    ///
    /// - `input`: A single CHAT word token, e.g., `hello`, `go&PAST`, `hello@b`.
    ///
    /// # Returns
    ///
    /// The parsed `Word` with its annotations and span information.
    ///
    /// # Errors
    ///
    /// Returns `ParseErrors` when:
    /// - Tree-sitter fails to parse the synthetic wrapper.
    /// - The resulting content is empty or the first element is not a word-like item.
    pub fn parse_word(&self, input: &str) -> ParseResult<Word> {
        parse_word::parse_word(self, input)
    }

    /// Parse a single dependent tier line (e.g., `%mor:\tdet|the n|cat .`)
    ///
    /// Uses the synthesis pattern: wraps the tier in a minimal CHAT file context,
    /// parses with tree-sitter, and extracts the dependent tier.
    ///
    /// # Format
    ///
    /// Dependent tiers follow the format: `%tiertype:\tcontent`
    ///
    /// # Examples
    ///
    /// ```
    /// use talkbank_parser::TreeSitterParser;
    ///
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let parser = TreeSitterParser::new()
    ///     .map_err(|err| std::io::Error::new(std::io::ErrorKind::Other, err))?;
    /// let result = parser.parse_tiers("%mor:\tpro|I v|want n|cookie-PL .");
    /// assert!(result.is_ok());
    /// # Ok(())
    /// # }
    /// ```
    pub fn parse_tiers(&self, input: &str) -> ParseResult<DependentTier> {
        parse_tiers::parse_tiers(self, input)
    }
}
