//! Traversal context for semantic-diff operations.
//!
//! The context tracks the best current source span while recursive diff logic
//! walks model nodes. This keeps emitted diffs location-aware without forcing
//! every call site to thread span plumbing manually.
//!
//! Reference: <https://talkbank.org/0info/manuals/CHAT.html#File_Format>

use crate::Span;

/// Tracks the current source span during semantic-diff traversal.
///
/// The value behaves like a lightweight stack frame pointer for span metadata:
/// nested nodes may temporarily override it, then restore the previous value.
///
/// Reference: <https://talkbank.org/0info/manuals/CHAT.html#File_Format>
#[derive(Debug, Clone)]
pub struct SemanticDiffContext {
    current_span: Option<Span>,
}

impl Default for SemanticDiffContext {
    /// Creates an empty context with no active span.
    ///
    /// This keeps default traversal behavior deterministic when no source spans
    /// have been pushed yet.
    fn default() -> Self {
        Self::new()
    }
}

impl SemanticDiffContext {
    /// Creates a new [`SemanticDiffContext`] with no active span.
    ///
    /// Callers usually allocate one context per semantic diff traversal.
    pub fn new() -> Self {
        Self { current_span: None }
    }

    /// Returns the current source span, if any.
    ///
    /// This is read by report helpers to attach location metadata to diffs.
    pub fn current_span(&self) -> Option<Span> {
        self.current_span
    }

    /// Set a new current span and return the previous one for restoration.
    ///
    /// The push/pop pattern allows nested model nodes to temporarily override
    /// span context without losing outer-node location information.
    pub fn push_span(&mut self, span: Option<Span>) -> Option<Span> {
        let prev = self.current_span;
        if span.is_some() {
            self.current_span = span;
        }
        prev
    }

    /// Restore a previous span returned by [`push_span`](Self::push_span).
    ///
    /// Callers should always restore in reverse traversal order to preserve
    /// context correctness.
    pub fn pop_span(&mut self, prev: Option<Span>) {
        self.current_span = prev;
    }
}

/// Converts a [`Span`] to `None` if it is a dummy span, or wraps it in `Some`.
///
/// This keeps diagnostic output free from meaningless placeholder offsets.
pub fn normalize_span(span: Span) -> Option<Span> {
    if span.is_dummy() { None } else { Some(span) }
}

/// Filters out dummy spans from an `Option<Span>`.
///
/// Use this helper when working with optional span fields that may carry
/// placeholder values from partially constructed AST nodes.
pub fn normalize_span_option(span: Option<Span>) -> Option<Span> {
    span.filter(|s| !s.is_dummy())
}
