//! Replacement annotations for CHAT transcripts.
//!
//! Replacements indicate what should have been said instead of what was actually uttered.
//! They are marked with the `[: word]` syntax in CHAT format.
//!
//! # Format
//!
//! ```text
//! actual_word [: intended_word]
//! ```
//!
//! # Use Cases
//!
//! - **Mispronunciations**: `doggie [: dog]` - Child says "doggie" but means "dog"
//! - **Phonological errors**: `wed [: red]` - Substitution of /w/ for /r/
//! - **Semantic errors**: `cat [: dog]` - Wrong word chosen
//! - **Grammatical errors**: `goed [: went]` - Incorrect past tense
//!
//! # CHAT Manual Reference
//!
//! - [Replacement Scope](https://talkbank.org/0info/manuals/CHAT.html#Replacement_Scope)
//! - [Error Coding](https://talkbank.org/0info/manuals/CHAT.html#Error_Coding)
//!
//! # Examples
//!
//! ```text
//! *CHI: I want the doggie [: dog] .
//! *CHI: he goed [: went] home .
//! *CHI: the wed [: red] ball .
//! ```

use super::{ContentAnnotation, Word, WriteChat};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::ops::{Deref, DerefMut};
use talkbank_derive::{SemanticEq, SpanShift};

/// Ordered list of words in a replacement annotation (`[: word1 word2]`).
///
/// Wraps a `Vec<Word>` and provides collection-like access via `Deref`.
///
/// Reference:
/// - <https://talkbank.org/0info/manuals/CHAT.html#Replacement_Scope>
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema, SemanticEq, SpanShift)]
#[serde(transparent)]
#[schemars(transparent)]
pub struct ReplacementWords(pub Vec<Word>);

impl ReplacementWords {
    /// Wraps replacement tokens in transcript order.
    ///
    /// Construction is infallible; replacement-specific constraints are checked
    /// later by [`crate::validation::Validate`].
    pub fn new(words: Vec<Word>) -> Self {
        Self(words)
    }

    /// Returns `true` when no intended tokens were provided.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl Deref for ReplacementWords {
    type Target = Vec<Word>;

    /// Exposes the underlying words for read-only collection operations.
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for ReplacementWords {
    /// Exposes the underlying words for in-place mutation.
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl From<Vec<Word>> for ReplacementWords {
    /// Wraps a plain vector as `ReplacementWords`.
    fn from(words: Vec<Word>) -> Self {
        Self(words)
    }
}

impl<'a> IntoIterator for &'a ReplacementWords {
    type Item = &'a Word;
    type IntoIter = std::slice::Iter<'a, Word>;

    /// Iterates over borrowed replacement words.
    fn into_iter(self) -> Self::IntoIter {
        self.0.iter()
    }
}

impl<'a> IntoIterator for &'a mut ReplacementWords {
    type Item = &'a mut Word;
    type IntoIter = std::slice::IterMut<'a, Word>;

    /// Iterates over mutable replacement words.
    fn into_iter(self) -> Self::IntoIter {
        self.0.iter_mut()
    }
}

impl IntoIterator for ReplacementWords {
    type Item = Word;
    type IntoIter = std::vec::IntoIter<Word>;

    /// Consumes the wrapper and yields owned replacement words.
    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

impl crate::validation::Validate for ReplacementWords {
    /// Enforces replacement-specific word constraints (non-empty, no omissions/untranscribed).
    fn validate(
        &self,
        context: &crate::validation::ValidationContext,
        errors: &impl crate::ErrorSink,
    ) {
        let span = match context.field_span {
            Some(span) => span,
            None => crate::Span::DUMMY,
        };
        // DEFAULT: Absent field text is reported as empty for error context.
        let text = context.field_text.as_deref().unwrap_or_default();
        // DEFAULT: Missing label falls back to "replacement" for error messaging.
        let label = context.field_label.unwrap_or("replacement");

        if self.is_empty() {
            errors.report(
                crate::ParseError::new(
                    crate::ErrorCode::EmptyReplacement,
                    crate::Severity::Error,
                    crate::SourceLocation::new(span),
                    crate::ErrorContext::new(text, span, label),
                    "Replacement [: text] must contain at least one word",
                )
                .with_suggestion("Add replacement text after [: "),
            );
        }

        for replacement_word in &self.0 {
            replacement_word.validate(context, errors);

            if !crate::validation::word::structure::has_spoken_material(replacement_word) {
                errors.report(
                    crate::ParseError::new(
                        crate::ErrorCode::EmptySpokenContent,
                        crate::Severity::Error,
                        crate::SourceLocation::new(replacement_word.span),
                        crate::ErrorContext::new(
                            replacement_word.raw_text(),
                            replacement_word.span,
                            replacement_word.raw_text(),
                        ),
                        "Replacement word is empty",
                    )
                    .with_suggestion("Remove empty word from replacement or fix parsing issue"),
                );
            }

            if let Some(crate::model::WordCategory::Omission) = replacement_word.category {
                errors.report(
                    crate::ParseError::new(
                        crate::ErrorCode::ReplacementContainsOmission,
                        crate::Severity::Error,
                        crate::SourceLocation::new(replacement_word.span),
                        crate::ErrorContext::new(
                            replacement_word.raw_text(),
                            replacement_word.span,
                            replacement_word.raw_text(),
                        ),
                        "Replacement word cannot be an omission (0-prefix)",
                    )
                    .with_suggestion("Remove the 0 prefix from replacement words"),
                );
            }

            if replacement_word.untranscribed().is_some() {
                errors.report(
                    crate::ParseError::new(
                        crate::ErrorCode::ReplacementContainsUntranscribed,
                        crate::Severity::Error,
                        crate::SourceLocation::new(replacement_word.span),
                        crate::ErrorContext::new(
                            replacement_word.raw_text(),
                            replacement_word.span,
                            replacement_word.raw_text(),
                        ),
                        "Replacement word cannot be untranscribed (xxx, yyy, www)",
                    )
                    .with_suggestion(
                        "Replacement words must be intelligible words, not untranscribed markers",
                    ),
                );
            }
        }
    }
}

/// Scoped annotations attached to a replaced word (e.g., `[*]`, `[= text]`).
///
/// Wraps a `Vec<ContentAnnotation>` and provides collection-like access via `Deref`.
///
/// # Reference
///
/// - [Replacement scope](https://talkbank.org/0info/manuals/CHAT.html#Replacement_Scope)
#[derive(
    Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema, SemanticEq, SpanShift, Default,
)]
#[serde(transparent)]
#[schemars(transparent)]
pub struct ReplacedWordAnnotations(pub Vec<ContentAnnotation>);

impl ReplacedWordAnnotations {
    /// Wraps scoped annotations attached to a replaced word.
    pub fn new(annotations: Vec<ContentAnnotation>) -> Self {
        Self(annotations)
    }

    /// Returns `true` when no scoped annotations are present.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl Deref for ReplacedWordAnnotations {
    type Target = Vec<ContentAnnotation>;

    /// Exposes annotations as a read-only slice-like vector.
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for ReplacedWordAnnotations {
    /// Exposes annotations for in-place mutation.
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl From<Vec<ContentAnnotation>> for ReplacedWordAnnotations {
    /// Wraps raw annotation vectors into the semantic newtype.
    fn from(annotations: Vec<ContentAnnotation>) -> Self {
        Self(annotations)
    }
}

impl<'a> IntoIterator for &'a ReplacedWordAnnotations {
    type Item = &'a ContentAnnotation;
    type IntoIter = std::slice::Iter<'a, ContentAnnotation>;

    /// Iterates over borrowed scoped annotations.
    fn into_iter(self) -> Self::IntoIter {
        self.0.iter()
    }
}

impl<'a> IntoIterator for &'a mut ReplacedWordAnnotations {
    type Item = &'a mut ContentAnnotation;
    type IntoIter = std::slice::IterMut<'a, ContentAnnotation>;

    /// Iterates over mutable scoped annotations.
    fn into_iter(self) -> Self::IntoIter {
        self.0.iter_mut()
    }
}

impl IntoIterator for ReplacedWordAnnotations {
    type Item = ContentAnnotation;
    type IntoIter = std::vec::IntoIter<ContentAnnotation>;

    /// Consumes the wrapper and yields owned annotations.
    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

impl crate::validation::Validate for ReplacedWordAnnotations {
    /// Flags unknown scoped-annotation markers while preserving lenient parsing.
    fn validate(
        &self,
        context: &crate::validation::ValidationContext,
        errors: &impl crate::ErrorSink,
    ) {
        let span = match context.field_span {
            Some(span) => span,
            None => crate::Span::DUMMY,
        };
        // DEFAULT: Absent field text is reported as empty for error context.
        let text = context.field_text.as_deref().unwrap_or_default();
        // DEFAULT: Missing label falls back to "annotation" for error messaging.
        let label = context.field_label.unwrap_or("annotation");

        for annotation in &self.0 {
            if let ContentAnnotation::Unknown(unknown) = annotation {
                let marker = &unknown.marker;
                let message = format!(
                    "\"{}\" is not a known scoped annotation type: known types are *, =, +, <, >, //, ///",
                    marker
                );
                errors.report(
                    crate::ParseError::new(
                        crate::ErrorCode::UnknownAnnotation,
                        crate::Severity::Error,
                        crate::SourceLocation::new(span),
                        crate::ErrorContext::new(text, span, label),
                        message,
                    )
                    .with_suggestion(
                        "Use one of the known scoped annotation types: [*], [= text], [+ text], [<], [>], [//], [///]",
                    ),
                );
            }
        }
    }
}

/// Replacement specification indicating the intended word(s).
///
/// A replacement shows what should have been said instead of what was actually uttered.
/// In CHAT format, this is written as `[: intended_word]` after the actual word.
///
/// # Structure
///
/// - Can contain one or more words (though single word is most common)
/// - Must be non-empty (validated during parsing)
/// - Words in replacement follow normal word structure (can have form types, etc.)
///
/// # Common Usage
///
/// Replacements are typically used for:
/// - **Phonological errors**: Sound substitutions or mispronunciations
/// - **Morphological errors**: Incorrect inflections (e.g., "goed" → "went")
/// - **Lexical errors**: Wrong word selection
/// - **Target forms**: What the child was attempting to say
///
/// # CHAT Manual Reference
///
/// - [Replacement Scope](https://talkbank.org/0info/manuals/CHAT.html#Replacement_Scope)
///
/// # Examples
///
/// ```
/// use talkbank_model::model::{Replacement, Word};
///
/// // Single word replacement
/// let replacement = Replacement::from_word(
///     Word::new_unchecked("dog", "dog")
/// );
///
/// // Multi-word replacement
/// let replacement = Replacement::new(vec![
///     Word::new_unchecked("went", "went"),
///     Word::new_unchecked("home", "home"),
/// ]);
/// ```
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema, SemanticEq, SpanShift)]
pub struct Replacement {
    /// The intended word(s).
    ///
    /// What should have been said instead of the actual utterance.
    /// Must contain at least one word (validated during parsing).
    pub words: ReplacementWords,
}

impl Replacement {
    /// Builds replacement payload from one or more intended words.
    ///
    /// An empty list is allowed at construction time so parser pipelines can
    /// keep building an AST and surface a typed validation error afterward.
    pub fn new(words: Vec<Word>) -> Self {
        // Note: Nonempty constraint validated during validation phase, not here
        // This allows lenient parsing with validation errors reported later
        Self {
            words: words.into(),
        }
    }

    /// Convenience constructor for the common single-target form `[: word]`.
    pub fn from_word(word: Word) -> Self {
        Self {
            words: vec![word].into(),
        }
    }

    /// Serializes this replacement as CHAT `[: ... ]` syntax.
    pub fn write_chat<W: std::fmt::Write>(&self, w: &mut W) -> std::fmt::Result {
        w.write_str("[: ")?;
        for (i, word) in self.words.iter().enumerate() {
            if i > 0 {
                w.write_char(' ')?;
            }
            word.write_chat(w)?;
        }
        w.write_char(']')
    }
}

impl crate::validation::Validate for Replacement {
    /// Validates each replacement word and enforces non-empty replacement content.
    fn validate(
        &self,
        context: &crate::validation::ValidationContext,
        errors: &impl crate::ErrorSink,
    ) {
        self.words.validate(context, errors);
    }
}

/// A spoken word paired with CHAT replacement metadata.
///
/// `ReplacedWord` models the local structure:
/// `spoken [: intended] [scoped-annotations...]`.
///
/// # Format in CHAT
///
/// ```text
/// actual_word [: replacement] [* error_code] [= explanation]
/// ```
///
/// # Structure
///
/// - **word**: What was actually said
/// - **replacement**: What should have been said (one or more words)
/// - **scoped_annotations**: Optional error codes, explanations, etc.
///
/// # CHAT Manual Reference
///
/// - [Replacement Scope](https://talkbank.org/0info/manuals/CHAT.html#Replacement_Scope)
/// - [Error Coding with Replacements](https://talkbank.org/0info/manuals/CHAT.html#Error_Coding)
///
/// # Examples
///
/// ```
/// use talkbank_model::model::{ReplacedWord, Replacement, ContentAnnotation, ScopedError, Word};
///
/// // Simple replacement
/// let replaced = ReplacedWord::new(
///     Word::new_unchecked("doggie", "doggie"),
///     Replacement::from_word(Word::new_unchecked("dog", "dog"))
/// );
///
/// // With error annotation
/// let replaced = ReplacedWord::new(
///     Word::new_unchecked("goed", "goed"),
///     Replacement::from_word(Word::new_unchecked("went", "went"))
/// ).with_scoped_annotations(vec![
///     ContentAnnotation::Error(ScopedError { code: Some("grammar".into()) })
/// ]);
/// ```
///
/// **CHAT format:**
/// ```text
/// *CHI: I doggie [: dog] .
/// *CHI: he goed [: went] [* grammar] home .
/// ```
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema, SemanticEq, SpanShift)]
pub struct ReplacedWord {
    /// The word that was actually said.
    ///
    /// This is what appears in the transcript as spoken.
    pub word: Word,

    /// The intended or correct word(s).
    ///
    /// What should have been said instead of the actual word.
    pub replacement: Replacement,

    /// Optional scoped annotations attached after the replacement block.
    ///
    /// Common annotations include:
    /// - `[*]` or `[* code]` - Error type markers
    /// - `[= text]` - Explanations
    /// - Other scoped annotations as needed
    #[serde(skip_serializing_if = "ReplacedWordAnnotations::is_empty", default)]
    pub scoped_annotations: ReplacedWordAnnotations,

    /// Source span for diagnostics (not serialized to JSON).
    #[serde(skip)]
    #[schemars(skip)]
    pub span: crate::Span,
}

impl ReplacedWord {
    /// Creates a replaced word with no scoped annotations.
    pub fn new(word: Word, replacement: Replacement) -> Self {
        Self {
            word,
            replacement,
            scoped_annotations: ReplacedWordAnnotations::new(Vec::new()),
            span: crate::Span::DUMMY,
        }
    }

    /// Sets source span metadata used in diagnostics.
    pub fn with_span(mut self, span: crate::Span) -> Self {
        self.span = span;
        self
    }

    /// Replaces scoped annotations attached to this replaced word.
    pub fn with_scoped_annotations(mut self, scoped: Vec<ContentAnnotation>) -> Self {
        self.scoped_annotations = scoped.into();
        self
    }

    /// Serializes `word [: replacement]` and trailing scoped annotations.
    pub fn write_chat<W: std::fmt::Write>(&self, w: &mut W) -> std::fmt::Result {
        self.word.write_chat(w)?;
        w.write_str(" [: ")?;
        // Write replacement words
        for (i, word) in self.replacement.words.iter().enumerate() {
            if i > 0 {
                w.write_char(' ')?;
            }
            word.write_chat(w)?;
        }
        w.write_char(']')?;
        for ann in &self.scoped_annotations {
            w.write_char(' ')?;
            ann.write_chat(w)?;
        }
        Ok(())
    }
}

impl crate::validation::Validate for ReplacedWord {
    /// Validates replacement legality for word category and validates nested fields.
    fn validate(
        &self,
        context: &crate::validation::ValidationContext,
        errors: &impl crate::ErrorSink,
    ) {
        self.word.validate(context, errors);

        // E387: Check if replacement is used with phonological fragment
        if let Some(crate::model::WordCategory::PhonologicalFragment) = self.word.category {
            errors.report(
                crate::ParseError::new(
                    crate::ErrorCode::ReplacementOnFragment,
                    crate::Severity::Error,
                    crate::SourceLocation::new(self.word.span),
                    crate::ErrorContext::new(
                        self.word.cleaned_text(),
                        self.word.span,
                        self.word.cleaned_text(),
                    ),
                    "Replacement [: text] is not allowed for fragments",
                )
                .with_suggestion(
                    "Remove the [: replacement] annotation from fragment words (those starting with &+)",
                ),
            );
        }

        // E388: Check if replacement is used with nonword
        if let Some(crate::model::WordCategory::Nonword) = self.word.category {
            errors.report(
                crate::ParseError::new(
                    crate::ErrorCode::ReplacementOnNonword,
                    crate::Severity::Error,
                    crate::SourceLocation::new(self.word.span),
                    crate::ErrorContext::new(
                        self.word.cleaned_text(),
                        self.word.span,
                        self.word.cleaned_text(),
                    ),
                    "Replacement [: text] is not allowed for nonwords",
                )
                .with_suggestion(
                    "Remove the [: replacement] annotation from nonword markers (those starting with &~)",
                ),
            );
        }

        // E389: Check if replacement is used with filler
        if let Some(crate::model::WordCategory::Filler) = self.word.category {
            errors.report(
                crate::ParseError::new(
                    crate::ErrorCode::ReplacementOnFiller,
                    crate::Severity::Error,
                    crate::SourceLocation::new(self.word.span),
                    crate::ErrorContext::new(
                        self.word.cleaned_text(),
                        self.word.span,
                        self.word.cleaned_text(),
                    ),
                    "Replacement [: text] is not allowed for fillers",
                )
                .with_suggestion(
                    "Remove the [: replacement] annotation from filler markers (those starting with &-)",
                ),
            );
        }

        let replacement_context = context
            .clone()
            .with_field_span(self.word.span)
            .with_field_text(self.word.cleaned_text().to_string())
            .with_field_label("replacement");
        self.replacement.validate(&replacement_context, errors);

        let annotation_context = context
            .clone()
            .with_field_span(self.word.span)
            .with_field_text(self.word.cleaned_text().to_string())
            .with_field_label("annotation");
        self.scoped_annotations
            .validate(&annotation_context, errors);
    }
}

impl std::fmt::Display for ReplacedWord {
    /// Renders the replacement using CHAT serialization format.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.write_chat(f)
    }
}
