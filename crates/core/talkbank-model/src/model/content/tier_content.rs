//! Shared tier content structure used by both main tiers and %wor tiers.
//!
//! This module defines the shared payload shape used by utterance-like tiers:
//! main tiers (`*SPEAKER:`) and `%wor` timing tiers.
//!
//! Both tier types share the same structure:
//! - Optional linkers (discourse markers)
//! - Optional language code
//! - Content (words, events, pauses, groups, etc.)
//! - Optional terminator
//! - Optional postcodes
//! - Optional bullet (media timestamp)
//!
//! CHAT reference anchors:
//! - [Words](https://talkbank.org/0info/manuals/CHAT.html#Words)
//! - [Dependent tiers](https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers)
//! - [Word tier](https://talkbank.org/0info/manuals/CHAT.html#Word_Tier)

use super::{Bullet, LanguageCode, Linker, Postcode, Terminator, UtteranceContent, WriteChat};
use crate::validation::{Validate, ValidationContext};
use crate::{ErrorCode, ErrorContext, ErrorSink, ParseError, Severity, SourceLocation, Span};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::ops::{Deref, DerefMut};
use talkbank_derive::{SemanticEq, SpanShift};

/// Shared utterance-like content structure.
///
/// Used by:
/// - MainTier (with speaker field)
/// - WorTier (without speaker field)
///
/// This structure contains all the components of an utterance except the speaker code.
///
/// # References
///
/// - [Words](https://talkbank.org/0info/manuals/CHAT.html#Words)
/// - [Word tier](https://talkbank.org/0info/manuals/CHAT.html#Word_Tier)
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema, SemanticEq, SpanShift)]
pub struct TierContent {
    /// Discourse linkers at the start of the tier (for example `++`, `+<`, `+^`).
    #[serde(skip_serializing_if = "TierLinkers::is_empty", default)]
    pub linkers: TierLinkers,

    /// Optional utterance-scoped language code (`[- code]`).
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub language_code: Option<LanguageCode>,

    /// Ordered tier content items (words, groups, events, pauses, etc.).
    pub content: TierContentItems,

    /// Optional utterance terminator (`.`, `?`, `!`, `+...`, etc.).
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub terminator: Option<Terminator>,

    /// Tier-level postcodes that appear after the terminator.
    #[serde(skip_serializing_if = "TierPostcodes::is_empty", default)]
    pub postcodes: TierPostcodes,

    /// Optional terminal media bullet (`start_end`) after postcodes.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub bullet: Option<Bullet>,

    /// Optional source span for content region (after colon).
    ///
    /// Used for precise diagnostics and skipped in JSON output.
    #[serde(skip)]
    #[schemars(skip)]
    #[semantic_eq(skip)]
    pub content_span: Option<Span>,
}

impl TierContent {
    /// Builds tier content with required body items and empty optional components.
    pub fn new(content: Vec<UtteranceContent>) -> Self {
        Self {
            linkers: TierLinkers::new(Vec::new()),
            language_code: None,
            content: content.into(),
            terminator: None,
            postcodes: TierPostcodes::new(Vec::new()),
            bullet: None,
            content_span: None,
        }
    }

    /// Builds tier content with all fields explicitly provided.
    pub fn with_all(
        linkers: Vec<Linker>,
        language_code: Option<LanguageCode>,
        content: Vec<UtteranceContent>,
        terminator: Option<Terminator>,
        postcodes: Vec<Postcode>,
        bullet: Option<Bullet>,
    ) -> Self {
        Self {
            linkers: linkers.into(),
            language_code,
            content: content.into(),
            terminator,
            postcodes: postcodes.into(),
            bullet,
            content_span: None,
        }
    }

    /// Replaces linker list.
    pub fn with_linkers(mut self, linkers: Vec<Linker>) -> Self {
        self.linkers = linkers.into();
        self
    }

    /// Sets utterance-scoped language code (`[- code]`).
    pub fn with_language_code(mut self, code: LanguageCode) -> Self {
        self.language_code = Some(code);
        self
    }

    /// Sets terminator token.
    pub fn with_terminator(mut self, terminator: Terminator) -> Self {
        self.terminator = Some(terminator);
        self
    }

    /// Replaces postcode list.
    pub fn with_postcodes(mut self, postcodes: Vec<Postcode>) -> Self {
        self.postcodes = postcodes.into();
        self
    }

    /// Sets terminal bullet marker.
    pub fn with_bullet(mut self, bullet: Bullet) -> Self {
        self.bullet = Some(bullet);
        self
    }

    /// Extract the last `InternalBullet` from content into the `bullet` field.
    ///
    /// The grammar's greedy `contents` rule consumes all media bullets as
    /// content items, including the utterance-final one. This method moves
    /// the trailing `InternalBullet`(s) to the `bullet` serialization slot
    /// so that WriteChat emits the bullet after the terminator (correct
    /// CHAT format: `hello . \u0015100_200\u0015`).
    ///
    /// Called at the parse-to-model boundary by both TreeSitter and re2c
    /// parsers. Idempotent: does nothing if `bullet` is already set.
    pub fn extract_terminal_bullet(&mut self) {
        use super::utterance_content::UtteranceContent;
        if self.bullet.is_some() {
            return;
        }
        // Pop trailing InternalBullet(s). The last one becomes the terminal bullet.
        while let Some(UtteranceContent::InternalBullet(_)) = self.content.last() {
            if let Some(UtteranceContent::InternalBullet(b)) = self.content.pop() {
                self.bullet = Some(b);
            }
        }
    }

    /// Sets source span for content region.
    pub fn with_content_span(mut self, span: Span) -> Self {
        self.content_span = Some(span);
        self
    }

    /// Writes only the payload region (everything after tier prefix).
    ///
    /// This writes: linkers, language_code, content, terminator, postcodes, bullet
    /// The caller is responsible for writing the prefix (*SPEAKER:\t or %wor:\t)
    pub(crate) fn write_tier_content<W: std::fmt::Write>(&self, w: &mut W) -> std::fmt::Result {
        // Write linkers if present (after tab, before content)
        for (i, linker) in self.linkers.iter().enumerate() {
            if i > 0 {
                w.write_char(' ')?;
            }
            linker.write_chat(w)?;
        }

        // Write language code if present (after linkers, before content)
        if let Some(ref lang_code) = self.language_code {
            if !self.linkers.is_empty() {
                w.write_char(' ')?;
            }
            w.write_str("[- ")?;
            lang_code.write_chat(w)?;
            w.write_char(']')?;
        }

        // Write content
        for (i, item) in self.content.iter().enumerate() {
            // Add space before item, EXCEPT:
            // - First item (when i==0 and no linkers/language_code)
            //
            // NOTE: Overlap markers at content level are ALWAYS standalone and serialized
            // with canonical spacing: space AFTER opening, space BEFORE closing.
            // Word-internal overlaps are handled inside Word serialization (no extra spaces).
            // The model is spacing-agnostic, so we normalize to canonical form here.
            let needs_space = i > 0 || !self.linkers.is_empty() || self.language_code.is_some();

            if needs_space {
                w.write_char(' ')?;
            }
            item.write_chat(w)?;
        }

        // Write terminator if present
        if let Some(ref term) = self.terminator {
            // Add space before terminator if there's content, linkers, or language code
            if !self.content.is_empty() || !self.linkers.is_empty() || self.language_code.is_some()
            {
                w.write_char(' ')?;
            }
            term.write_chat(w)?;
        }

        // Write postcodes after terminator
        for postcode in &self.postcodes {
            w.write_char(' ')?;
            postcode.write_chat(w)?;
        }

        // Write bullet if present (terminal timing marker)
        if let Some(ref bullet) = self.bullet {
            w.write_char(' ')?;
            bullet.write_chat(w)?;
        }

        Ok(())
    }

    /// Render the tier content portion (everything after the tier prefix) to a String.
    ///
    /// Preconditions:
    /// - `self` satisfies the `TierContent` invariants (well-formed linkers, content, and markers).
    ///
    /// Postconditions:
    /// - Returns the canonical CHAT serialization of this tier content without a prefix.
    ///
    /// Invariants:
    /// - Does not mutate `self`.
    ///
    /// Complexity:
    /// - Time: O(n) in the number of content items and annotations.
    /// - Space: O(n) for the output string.
    pub fn to_content_string(&self) -> String {
        let mut output = String::new();
        let _ = self.write_tier_content(&mut output);
        output
    }

    /// Writes payload region while omitting all bullet markers.
    ///
    /// Skips the terminal bullet and any `InternalBullet` content items.
    /// Everything else (linkers, language code, words, terminator, postcodes)
    /// is written normally. Useful for producing clean display text (e.g.
    /// TextGrid interval labels) from the AST without cloning.
    pub(crate) fn write_tier_content_no_bullets<W: std::fmt::Write>(
        &self,
        w: &mut W,
    ) -> std::fmt::Result {
        use super::UtteranceContent;

        // Linkers
        for (i, linker) in self.linkers.iter().enumerate() {
            if i > 0 {
                w.write_char(' ')?;
            }
            linker.write_chat(w)?;
        }

        // Language code
        if let Some(ref lang_code) = self.language_code {
            if !self.linkers.is_empty() {
                w.write_char(' ')?;
            }
            w.write_str("[- ")?;
            lang_code.write_chat(w)?;
            w.write_char(']')?;
        }

        // Content items — skip InternalBullet
        let mut item_count = 0;
        for item in self.content.iter() {
            if matches!(item, UtteranceContent::InternalBullet(_)) {
                continue;
            }
            let needs_space =
                item_count > 0 || !self.linkers.is_empty() || self.language_code.is_some();
            if needs_space {
                w.write_char(' ')?;
            }
            item.write_chat(w)?;
            item_count += 1;
        }

        // Terminator
        if let Some(ref term) = self.terminator {
            if item_count > 0 || !self.linkers.is_empty() || self.language_code.is_some() {
                w.write_char(' ')?;
            }
            term.write_chat(w)?;
        }

        // Postcodes
        for postcode in &self.postcodes {
            w.write_char(' ')?;
            postcode.write_chat(w)?;
        }

        // Terminal bullet intentionally omitted

        Ok(())
    }

    /// Render tier content while omitting all bullet markers.
    pub fn to_content_string_no_bullets(&self) -> String {
        let mut output = String::new();
        let _ = self.write_tier_content_no_bullets(&mut output);
        output
    }
}

impl Default for TierContent {
    /// Builds an empty tier payload used by constructors and serde defaults.
    fn default() -> Self {
        Self {
            linkers: TierLinkers::new(Vec::new()),
            language_code: None,
            content: TierContentItems::new(Vec::new()),
            terminator: None,
            postcodes: TierPostcodes::new(Vec::new()),
            bullet: None,
            content_span: None,
        }
    }
}

#[derive(
    Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema, SemanticEq, SpanShift, Default,
)]
#[serde(transparent)]
#[schemars(transparent)]
/// Collection of linkers at the start of a tier.
///
/// # Reference
///
/// - [Utterance linkers](https://talkbank.org/0info/manuals/CHAT.html#Utterance_Linkers)
pub struct TierLinkers(pub Vec<Linker>);

impl TierLinkers {
    /// Wraps linker values while preserving caller-provided order.
    pub fn new(linkers: Vec<Linker>) -> Self {
        Self(linkers)
    }

    /// Returns `true` if this contains no items.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl Deref for TierLinkers {
    type Target = Vec<Linker>;

    /// Borrows the underlying linker vector.
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for TierLinkers {
    /// Mutably borrows the underlying linker vector.
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl From<Vec<Linker>> for TierLinkers {
    /// Wraps a linker vector without additional allocation.
    fn from(linkers: Vec<Linker>) -> Self {
        Self(linkers)
    }
}

impl<'a> IntoIterator for &'a TierLinkers {
    type Item = &'a Linker;
    type IntoIter = std::slice::Iter<'a, Linker>;

    /// Iterates immutably over linker entries.
    fn into_iter(self) -> Self::IntoIter {
        self.0.iter()
    }
}

impl<'a> IntoIterator for &'a mut TierLinkers {
    type Item = &'a mut Linker;
    type IntoIter = std::slice::IterMut<'a, Linker>;

    /// Iterates mutably over linker entries.
    fn into_iter(self) -> Self::IntoIter {
        self.0.iter_mut()
    }
}

impl IntoIterator for TierLinkers {
    type Item = Linker;
    type IntoIter = std::vec::IntoIter<Linker>;

    /// Consumes the wrapper and yields owned linker entries.
    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

impl crate::validation::Validate for TierLinkers {
    /// Linker-level constraints are enforced by higher-level tier validation.
    fn validate(
        &self,
        _context: &crate::validation::ValidationContext,
        _errors: &impl crate::ErrorSink,
    ) {
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema, SemanticEq, SpanShift)]
#[serde(transparent)]
#[schemars(transparent)]
/// Collection of utterance content items within a tier.
///
/// # References
///
/// - [Words](https://talkbank.org/0info/manuals/CHAT.html#Words)
/// - [Annotations](https://talkbank.org/0info/manuals/CHAT.html#Annotations)
pub struct TierContentItems(pub Vec<UtteranceContent>);

impl TierContentItems {
    /// Wraps utterance content items in their on-tier order.
    pub fn new(content: Vec<UtteranceContent>) -> Self {
        Self(content)
    }

    /// Returns `true` if this contains no items.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl Deref for TierContentItems {
    type Target = Vec<UtteranceContent>;

    /// Borrows the underlying utterance-content vector.
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for TierContentItems {
    /// Mutably borrows the underlying utterance-content vector.
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl From<Vec<UtteranceContent>> for TierContentItems {
    /// Wraps content items without copying.
    fn from(content: Vec<UtteranceContent>) -> Self {
        Self(content)
    }
}

impl<'a> IntoIterator for &'a TierContentItems {
    type Item = &'a UtteranceContent;
    type IntoIter = std::slice::Iter<'a, UtteranceContent>;

    /// Iterates immutably over content items.
    fn into_iter(self) -> Self::IntoIter {
        self.0.iter()
    }
}

impl<'a> IntoIterator for &'a mut TierContentItems {
    type Item = &'a mut UtteranceContent;
    type IntoIter = std::slice::IterMut<'a, UtteranceContent>;

    /// Iterates mutably over content items.
    fn into_iter(self) -> Self::IntoIter {
        self.0.iter_mut()
    }
}

impl IntoIterator for TierContentItems {
    type Item = UtteranceContent;
    type IntoIter = std::vec::IntoIter<UtteranceContent>;

    /// Consumes the wrapper and yields owned content items.
    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

impl Validate for TierContentItems {
    /// Enforces minimum-content invariants before deeper item-level validation.
    fn validate(&self, context: &ValidationContext, errors: &impl ErrorSink) {
        let span = match context.field_span {
            Some(span) => span,
            None => Span::from_usize(0, 0),
        };
        // DEFAULT: Missing field text is reported as empty to match the offending value.
        let field_text = context.field_text.clone().unwrap_or_default();
        // DEFAULT: Missing label falls back to "content" for error messaging.
        let _field_label = context.field_label.unwrap_or("content");
        let error_context = ErrorContext::new(field_text.clone(), span, field_text.clone());

        if self.0.is_empty() {
            // E306: no content after the speaker. Same code as the
            // "only separators" branch below.
            errors.report(
                ParseError::new(
                    ErrorCode::EmptyUtterance,
                    Severity::Error,
                    SourceLocation::new(span),
                    error_context,
                    "Utterance is empty (no content after speaker)",
                )
                .with_suggestion("Add at least one word or content element to the utterance"),
            );
            return;
        }

        let has_meaningful_content = self
            .0
            .iter()
            .any(|item| !matches!(item, UtteranceContent::Separator(_)));

        if !has_meaningful_content {
            errors.report(
                ParseError::new(
                    ErrorCode::EmptyUtterance,
                    Severity::Error,
                    SourceLocation::new(span),
                    error_context.clone(),
                    "Utterance has no meaningful content (only separators)",
                )
                .with_suggestion("Add at least one word or content element to the utterance"),
            );

            errors.report(
                ParseError::new(
                    ErrorCode::EmptyWordContent,
                    Severity::Error,
                    SourceLocation::new(span),
                    error_context,
                    "Utterance contains no word content",
                )
                .with_suggestion("Add at least one lexical word item to the utterance"),
            );
        }

        // Note: Separators CAN appear at the end of utterances (before terminators).
        // Per CHAT specification: "Separators can appear anywhere, including at start and before terminator."
        // See docs/index.md for the current content syntax references.
    }
}

#[derive(
    Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema, SemanticEq, SpanShift, Default,
)]
#[serde(transparent)]
#[schemars(transparent)]
/// Collection of postcodes attached to a tier.
///
/// # Reference
///
/// - [Postcodes](https://talkbank.org/0info/manuals/CHAT.html#Postcodes)
pub struct TierPostcodes(pub Vec<Postcode>);

impl TierPostcodes {
    /// Wraps postcode annotations while preserving transcript order.
    pub fn new(postcodes: Vec<Postcode>) -> Self {
        Self(postcodes)
    }

    /// Returns `true` if this contains no items.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl Deref for TierPostcodes {
    type Target = Vec<Postcode>;

    /// Borrows the underlying postcode vector.
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for TierPostcodes {
    /// Mutably borrows the underlying postcode vector.
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl From<Vec<Postcode>> for TierPostcodes {
    /// Wraps a postcode vector without copying.
    fn from(postcodes: Vec<Postcode>) -> Self {
        Self(postcodes)
    }
}

impl<'a> IntoIterator for &'a TierPostcodes {
    type Item = &'a Postcode;
    type IntoIter = std::slice::Iter<'a, Postcode>;

    /// Iterates immutably over postcode entries.
    fn into_iter(self) -> Self::IntoIter {
        self.0.iter()
    }
}

impl<'a> IntoIterator for &'a mut TierPostcodes {
    type Item = &'a mut Postcode;
    type IntoIter = std::slice::IterMut<'a, Postcode>;

    /// Iterates mutably over postcode entries.
    fn into_iter(self) -> Self::IntoIter {
        self.0.iter_mut()
    }
}

impl IntoIterator for TierPostcodes {
    type Item = Postcode;
    type IntoIter = std::vec::IntoIter<Postcode>;

    /// Consumes the wrapper and yields owned postcode entries.
    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

impl crate::validation::Validate for TierPostcodes {
    /// Postcode-level checks run in utterance/tier validators where context is available.
    fn validate(
        &self,
        _context: &crate::validation::ValidationContext,
        _errors: &impl crate::ErrorSink,
    ) {
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{OverlapPoint, OverlapPointKind, Separator, Word};
    use crate::validation::ValidationContext;
    use crate::{ErrorCollector, ParseError, Span};

    /// Validates a `TierContentItems` payload and returns collected parse errors.
    fn run_validation(items: TierContentItems) -> Vec<ParseError> {
        let mut context = ValidationContext::new();
        context.field_span = Some(Span::from_usize(0, 0));
        context.field_text = Some("test".to_string());
        let errors = ErrorCollector::new();
        items.validate(&context, &errors);
        errors.into_vec()
    }

    /// Regression: trailing separators remain valid in CHAT before terminators.
    #[test]
    fn trailing_separator_is_valid() {
        // Per CHAT specification, separators can appear anywhere, including before terminators
        let items = TierContentItems::new(vec![
            UtteranceContent::Word(Box::new(Word::new_unchecked("hi", "hi"))),
            UtteranceContent::Separator(Separator::Comma { span: Span::DUMMY }),
        ]);

        let errors = run_validation(items);
        assert!(
            errors.is_empty(),
            "Separators are allowed at the end of utterances (before terminators). No error should be reported."
        );
    }

    /// Regression: overlap markers after separators still count as meaningful content.
    #[test]
    fn separator_followed_by_overlap_is_valid() {
        let items = TierContentItems::new(vec![
            UtteranceContent::Word(Box::new(Word::new_unchecked("hello", "hello"))),
            UtteranceContent::Separator(Separator::Comma { span: Span::DUMMY }),
            UtteranceContent::OverlapPoint(OverlapPoint::new(
                OverlapPointKind::TopOverlapBegin,
                None,
            )),
        ]);

        let errors = run_validation(items);
        assert!(
            errors.is_empty(),
            "Overlap marker after separator should count as content"
        );
    }
}
