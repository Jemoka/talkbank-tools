//! Per-document validation-cache model used by incremental LSP revalidation.
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Format>
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Headers>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Main_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>

use talkbank_model::ParseError;
use talkbank_model::Span;
use talkbank_model::validation::ValidationContext;

/// Cached validation artifacts split by scope (headers, utterances, cross-line checks).
///
/// Each bucket stores the `ParseError`s emitted by the parser/validation runner.
/// The utterance-level vectors mirror the utterances in the cached `ChatFile`,
/// while the signature vectors hold 64-bit fingerprints used to decide when
/// incremental validation can be skipped.
#[derive(Clone)]
pub struct ValidationCache {
    /// Shared validation context (participants, languages, options).
    pub context: ValidationContext,
    /// Errors from header validation.
    pub header_errors: Vec<ParseError>,
    /// Errors from scoped (cross-tier alignment) validation.
    pub scoped_errors: Vec<ParseError>,
    /// Errors from timing-bullet validation.
    pub bullet_errors: Vec<ParseError>,
    /// Per-utterance errors, indexed in parallel with the cached `ChatFile`.
    pub utterance_errors: Vec<Vec<ParseError>>,
    /// Per-utterance scoped-marker fingerprints for skipping unchanged utterances.
    pub utterance_scoped_signature: Vec<u64>,
    /// Per-utterance bullet fingerprints for skipping unchanged utterances.
    pub utterance_bullet_signature: Vec<u64>,
}

impl ValidationCache {
    /// Flatten all cached error buckets into one diagnostics vector.
    pub fn all_errors(&self) -> Vec<ParseError> {
        let mut errors = Vec::new();
        errors.extend(self.header_errors.iter().cloned());
        errors.extend(self.scoped_errors.iter().cloned());
        errors.extend(self.bullet_errors.iter().cloned());
        for entry in &self.utterance_errors {
            errors.extend(entry.iter().cloned());
        }
        errors
    }

    /// Insert a placeholder entry at `idx` for a newly added utterance.
    pub fn insert_utterance_at(&mut self, idx: usize) {
        self.utterance_errors.insert(idx, Vec::new());
        self.utterance_scoped_signature.insert(idx, 0);
        self.utterance_bullet_signature.insert(idx, 0);
    }

    /// Remove the cached entry at `idx` for a deleted utterance.
    pub fn remove_utterance_at(&mut self, idx: usize) {
        if idx < self.utterance_errors.len() {
            self.utterance_errors.remove(idx);
        }
        if idx < self.utterance_scoped_signature.len() {
            self.utterance_scoped_signature.remove(idx);
        }
        if idx < self.utterance_bullet_signature.len() {
            self.utterance_bullet_signature.remove(idx);
        }
    }

    /// Shift cached spans after an edit offset by `delta` bytes.
    ///
    /// This keeps cached diagnostics consistent even when edits insert/delete text
    /// prior to stored spans; the logic clamps spans to the document bounds.
    pub fn shift_spans_after(&mut self, offset: u32, delta: i32, full_text_len: usize) {
        shift_errors_after(&mut self.header_errors, offset, delta, full_text_len);
        shift_errors_after(&mut self.scoped_errors, offset, delta, full_text_len);
        shift_errors_after(&mut self.bullet_errors, offset, delta, full_text_len);
        for errors in &mut self.utterance_errors {
            shift_errors_after(errors, offset, delta, full_text_len);
        }
    }
}

/// Shift each error span/label span after an edit.
fn shift_errors_after(errors: &mut Vec<ParseError>, offset: u32, delta: i32, full_text_len: usize) {
    for error in errors {
        shift_span_after(&mut error.location.span, offset, delta);

        if let Some(ctx) = &mut error.context {
            let use_global_span =
                ctx.source_text.is_empty() || ctx.source_text.len() == full_text_len;
            if use_global_span {
                shift_span_after(&mut ctx.span, offset, delta);
            }
        }

        for label in &mut error.labels {
            shift_span_after(&mut label.span, offset, delta);
        }
    }
}

/// Shift one span after an edit offset, clamping to non-negative offsets.
fn shift_span_after(span: &mut Span, offset: u32, delta: i32) {
    if span.is_dummy() {
        return;
    }
    if span.start >= offset && span.end >= offset {
        let start = span.start as i64 + delta as i64;
        let end = span.end as i64 + delta as i64;
        span.start = start.max(0) as u32;
        span.end = end.max(0) as u32;
    }
}
