//! Generic wrapper for adding scoped annotations to content items.
//!
//! The `Annotated<T>` wrapper adds scoped annotations (bracketed markers) to any
//! content type that supports them. Scoped annotations provide linguistic, error,
//! and explanatory information about the preceding element.
//!
//! # Scoped Annotation Types
//!
//! - **Error codes** (`[* code]`) - Mark speech errors like `[* m]`, `[* s]`
//! - **Explanations** (`[= text]`) - Provide clarification or translation
//! - **Additions** (`[+ text]`) - Add transcriber comments
//! - **Retracing** (`[/]`, `[//]`, `[///]`) - Mark repetitions and corrections
//! - **Paralinguistic** (`[! text]`) - Note tone, emphasis, gestures
//! - **Replacements** (`[: text]`) - Show what was actually said
//!
//! # CHAT Format Examples
//!
//! ```text
//! I want [* m] cookie                      Word with error code
//! &=laughs [! loudly]                      Event with paralinguistic note
//! <I want> [/] I need cookie              Group with retracing
//! hola [= hello]                           Word with explanation
//! dog [: cat]                              Word with replacement
//! ```
//!
//! # References
//!
//! - [Scoped Symbols](https://talkbank.org/0info/manuals/CHAT.html#Scoped_Symbols)
//! - [Error Coding](https://talkbank.org/0info/manuals/CHAT.html#Error_Coding)

use super::{ContentAnnotation, WriteChat};
use crate::model::{
    SemanticDiff, SemanticDiffContext, SemanticDiffReport, SemanticEq, SemanticPath, normalize_span,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::ops::{Deref, DerefMut};
use talkbank_derive::{SemanticEq, SpanShift};

/// Scoped annotations attached to an `Annotated<T>` wrapper.
///
/// Wraps a `Vec<ContentAnnotation>` and provides collection-like access via `Deref`.
/// Must contain at least one annotation (validated during the validation phase).
///
/// References:
/// - <https://talkbank.org/0info/manuals/CHAT.html#Scoped_Symbols>
/// - <https://talkbank.org/0info/manuals/CHAT.html#Error_Coding>
#[derive(
    Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema, SemanticEq, SpanShift, Default,
)]
#[serde(transparent)]
#[schemars(transparent)]
pub struct AnnotatedContentAnnotations(pub Vec<ContentAnnotation>);

impl AnnotatedContentAnnotations {
    /// Wraps scoped annotations for an [`Annotated`] payload.
    ///
    /// Validation of "must not be empty" is intentionally deferred to
    /// [`crate::validation::Validate`], so parsers can stay lenient and report
    /// a typed error instead of failing construction.
    pub fn new(annotations: Vec<ContentAnnotation>) -> Self {
        Self(annotations)
    }

    /// Returns `true` when no scoped markers are attached.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl Deref for AnnotatedContentAnnotations {
    type Target = Vec<ContentAnnotation>;

    /// Borrows the underlying annotation vector.
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for AnnotatedContentAnnotations {
    /// Mutably borrows the underlying annotation vector.
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl From<Vec<ContentAnnotation>> for AnnotatedContentAnnotations {
    /// Wraps a raw scoped-annotation vector without copying.
    fn from(annotations: Vec<ContentAnnotation>) -> Self {
        Self(annotations)
    }
}

impl<'a> IntoIterator for &'a AnnotatedContentAnnotations {
    type Item = &'a ContentAnnotation;
    type IntoIter = std::slice::Iter<'a, ContentAnnotation>;

    /// Iterates immutably over scoped annotations.
    fn into_iter(self) -> Self::IntoIter {
        self.0.iter()
    }
}

impl<'a> IntoIterator for &'a mut AnnotatedContentAnnotations {
    type Item = &'a mut ContentAnnotation;
    type IntoIter = std::slice::IterMut<'a, ContentAnnotation>;

    /// Iterates mutably over scoped annotations.
    fn into_iter(self) -> Self::IntoIter {
        self.0.iter_mut()
    }
}

impl IntoIterator for AnnotatedContentAnnotations {
    type Item = ContentAnnotation;
    type IntoIter = std::vec::IntoIter<ContentAnnotation>;

    /// Consumes the wrapper and yields owned scoped annotations.
    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

impl crate::validation::Validate for AnnotatedContentAnnotations {
    /// Enforces non-empty annotations and reports unknown scoped markers.
    fn validate(
        &self,
        context: &crate::validation::ValidationContext,
        errors: &impl crate::ErrorSink,
    ) {
        // DEFAULT: Missing span indicates unknown location; use dummy span for diagnostics.
        let span = context.field_span.unwrap_or(crate::Span::DUMMY);
        // DEFAULT: Absent source text is reported as empty for error context.
        let text = context.field_text.as_deref().unwrap_or_default();
        // DEFAULT: Missing label falls back to generic "annotation".
        let label = context.field_label.unwrap_or("annotation");

        if self.is_empty() {
            errors.report(crate::ParseError::new(
                crate::ErrorCode::EmptyAnnotatedContentAnnotations,
                crate::Severity::Error,
                crate::SourceLocation::new(span),
                crate::ErrorContext::new(text, span, label),
                "Annotated content must include at least one scoped annotation",
            ));
        }

        for annotation in &self.0 {
            if let ContentAnnotation::Unknown(unknown) = annotation {
                let marker = &unknown.marker;
                errors.report(
                    crate::ParseError::new(
                        crate::ErrorCode::UnknownAnnotation,
                        crate::Severity::Error,
                        crate::SourceLocation::new(span),
                        crate::ErrorContext::new(marker.as_str(), 0..marker.len(), "annotation"),
                        format!("\"{}\" is not a known scoped annotation type", marker),
                    )
                    .with_suggestion("Check CHAT manual for valid annotation types"),
                );
            }
        }
    }
}

/// Generic wrapper that adds scoped annotations to a content item.
///
/// This wrapper is used throughout CHAT to attach bracketed annotations to
/// words, events, groups, and actions. The annotations appear immediately
/// after the annotated element in CHAT format.
///
/// # CHAT Format Examples
///
/// ```text
/// want [* m]                               Word error (missing word)
/// going [* s]                              Word error (started utterance)
/// dog [= explanation]                      Explanation annotation
/// &=laughs [! loudly]                      Paralinguistic note
/// <I want> [/] I need                      Repetition retracing
/// <the dog> [//] the cat                   Correction retracing
/// perro [= dog]                            Translation
/// ```
///
/// # Type Parameter
///
/// - `T` - The inner content type being annotated (Word, Event, Group, or Action)
///
/// # Common Uses
///
/// - `Annotated<Word>` - Word with error codes, explanations, or comments
/// - `Annotated<Event>` - Event with paralinguistic notes
/// - `Annotated<Group>` - Group with retracing markers
/// - `Annotated<Action>` - Action with explanatory notes
///
/// # References
///
/// - [Scoped Symbols](https://talkbank.org/0info/manuals/CHAT.html#Scoped_Symbols)
/// - [Error Coding](https://talkbank.org/0info/manuals/CHAT.html#Error_Coding)
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema, SpanShift)]
pub struct Annotated<T> {
    /// The payload that receives scoped annotations.
    #[serde(flatten)]
    pub inner: T,

    /// Scoped annotations emitted immediately after [`Self::inner`].
    ///
    /// Examples: `[*]`, `[= text]`, `[+ text]`, `[//]`.
    #[serde(skip_serializing_if = "AnnotatedContentAnnotations::is_empty", default)]
    pub scoped_annotations: AnnotatedContentAnnotations,

    /// Source span for error reporting (not serialized to JSON)
    #[serde(skip)]
    #[schemars(skip)]
    pub span: crate::Span,
}

impl<T: SemanticEq> SemanticEq for Annotated<T> {
    /// Semantic equality ignores wrapper span and compares payload + annotations.
    fn semantic_eq(&self, other: &Self) -> bool {
        self.inner.semantic_eq(&other.inner)
            && self
                .scoped_annotations
                .semantic_eq(&other.scoped_annotations)
    }
}

impl<T: SemanticDiff> SemanticDiff for Annotated<T> {
    /// Computes nested semantic diff while preserving wrapper span in context.
    fn semantic_diff_into(
        &self,
        other: &Self,
        path: &mut SemanticPath,
        report: &mut SemanticDiffReport,
        ctx: &mut SemanticDiffContext,
    ) {
        let prev_span = ctx.push_span(normalize_span(self.span));

        path.push_field("inner");
        self.inner
            .semantic_diff_into(&other.inner, path, report, ctx);
        path.pop();

        if !report.is_truncated() {
            path.push_field("scoped_annotations");
            self.scoped_annotations.semantic_diff_into(
                &other.scoped_annotations,
                path,
                report,
                ctx,
            );
            path.pop();
        }

        ctx.pop_span(prev_span);
    }
}

impl<T> Annotated<T> {
    /// Creates an annotated wrapper with an empty scoped-annotation list.
    ///
    /// This is convenient for builder-style assembly where annotations are added
    /// afterward via [`Self::with_scoped_annotation`] or
    /// [`Self::with_scoped_annotations`].
    pub fn new(inner: T) -> Self {
        Self {
            inner,
            scoped_annotations: AnnotatedContentAnnotations::new(Vec::new()),
            span: crate::Span::DUMMY,
        }
    }

    /// Sets source span metadata used in diagnostics.
    pub fn with_span(mut self, span: crate::Span) -> Self {
        self.span = span;
        self
    }

    /// Replaces the scoped-annotation list.
    pub fn with_scoped_annotations(mut self, scoped: Vec<ContentAnnotation>) -> Self {
        self.scoped_annotations = scoped.into();
        self
    }

    /// Appends one scoped annotation to the existing list.
    pub fn with_scoped_annotation(mut self, annotation: ContentAnnotation) -> Self {
        self.scoped_annotations.0.push(annotation);
        self
    }
}

impl<T: WriteChat> WriteChat for Annotated<T> {
    /// Serializes `inner` followed by each scoped annotation separated by spaces.
    fn write_chat<W: std::fmt::Write>(&self, w: &mut W) -> std::fmt::Result {
        self.inner.write_chat(w)?;
        for ann in &self.scoped_annotations {
            w.write_char(' ')?;
            ann.write_chat(w)?;
        }
        Ok(())
    }
}

impl<T: crate::validation::Validate> crate::validation::Validate for Annotated<T> {
    /// Validates inner payload first, then validates scoped-annotation constraints.
    fn validate(
        &self,
        context: &crate::validation::ValidationContext,
        errors: &impl crate::ErrorSink,
    ) {
        // Validate the inner item
        self.inner.validate(context, errors);
        let scoped_context = context
            .clone()
            .with_field_span(self.span)
            .with_field_label("annotation");
        self.scoped_annotations.validate(&scoped_context, errors);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ErrorCollector;
    use crate::model::{ContentAnnotation, Word};
    use crate::validation::{Validate, ValidationContext};

    /// Reports E214 when an `Annotated<T>` has no scoped annotations.
    #[test]
    fn empty_scoped_annotations_report_error() {
        let annotated = Annotated::new(Word::new_unchecked("hi", "hi"));
        let errors = ErrorCollector::new();
        let ctx = ValidationContext::default();
        annotated.validate(&ctx, &errors);

        let error_vec = errors.into_vec();
        assert!(
            error_vec.iter().any(|e| e.code.as_str() == "E214"),
            "Expected E214 for empty scoped annotations, got: {:#?}",
            error_vec
        );
    }

    /// Does not report E214 when at least one scoped annotation is present.
    #[test]
    fn nonempty_scoped_annotations_pass() {
        let annotated = Annotated::new(Word::new_unchecked("hi", "hi"))
            .with_scoped_annotation(ContentAnnotation::Stressing);
        let errors = ErrorCollector::new();
        let ctx = ValidationContext::default();
        annotated.validate(&ctx, &errors);

        let error_vec = errors.into_vec();
        assert!(
            !error_vec.iter().any(|e| e.code.as_str() == "E214"),
            "Non-empty scoped annotations should not trigger E214"
        );
    }
}
