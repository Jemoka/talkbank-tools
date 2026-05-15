//! Shared trait for shifting byte-based spans after text edits.
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Format>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Main_Tier>

use crate::{ErrorContext, ErrorLabel, ParseError, Span};

/// Trait for shifting byte-offset spans after an insertion or deletion.
///
/// All spans at or after `offset` are shifted by `delta` bytes.
/// Implementations exist for AST nodes, errors, and primitive types (no-ops).
pub trait SpanShift {
    /// Shift all spans at or after `offset` by `delta` bytes.
    fn shift_spans_after(&mut self, offset: u32, delta: i32);
}

impl SpanShift for Span {
    fn shift_spans_after(&mut self, offset: u32, delta: i32) {
        if self.is_dummy() {
            return;
        }
        if self.start >= offset && self.end >= offset {
            let start = self.start as i64 + delta as i64;
            let end = self.end as i64 + delta as i64;
            self.start = start.max(0) as u32;
            self.end = end.max(0) as u32;
        }
    }
}

impl SpanShift for ErrorLabel {
    fn shift_spans_after(&mut self, offset: u32, delta: i32) {
        self.span.shift_spans_after(offset, delta);
    }
}

impl SpanShift for ErrorContext {
    fn shift_spans_after(&mut self, offset: u32, delta: i32) {
        self.span.shift_spans_after(offset, delta);
    }
}

impl SpanShift for ParseError {
    fn shift_spans_after(&mut self, offset: u32, delta: i32) {
        self.location.span.shift_spans_after(offset, delta);
        self.context.shift_spans_after(offset, delta);
        for label in &mut self.labels {
            label.shift_spans_after(offset, delta);
        }
    }
}

impl SpanShift for String {
    fn shift_spans_after(&mut self, _offset: u32, _delta: i32) {}
}

impl SpanShift for std::sync::Arc<str> {
    fn shift_spans_after(&mut self, _offset: u32, _delta: i32) {}
}

impl SpanShift for smol_str::SmolStr {
    fn shift_spans_after(&mut self, _offset: u32, _delta: i32) {}
}

impl SpanShift for bool {
    fn shift_spans_after(&mut self, _offset: u32, _delta: i32) {}
}

impl SpanShift for u8 {
    fn shift_spans_after(&mut self, _offset: u32, _delta: i32) {}
}

impl SpanShift for u32 {
    fn shift_spans_after(&mut self, _offset: u32, _delta: i32) {}
}

impl SpanShift for u64 {
    fn shift_spans_after(&mut self, _offset: u32, _delta: i32) {}
}

impl SpanShift for usize {
    fn shift_spans_after(&mut self, _offset: u32, _delta: i32) {}
}

impl SpanShift for i32 {
    fn shift_spans_after(&mut self, _offset: u32, _delta: i32) {}
}

impl SpanShift for i64 {
    fn shift_spans_after(&mut self, _offset: u32, _delta: i32) {}
}

impl<T: SpanShift> SpanShift for Option<T> {
    fn shift_spans_after(&mut self, offset: u32, delta: i32) {
        if let Some(value) = self {
            value.shift_spans_after(offset, delta);
        }
    }
}

impl<T: SpanShift> SpanShift for Vec<T> {
    fn shift_spans_after(&mut self, offset: u32, delta: i32) {
        for item in self {
            item.shift_spans_after(offset, delta);
        }
    }
}

impl<T: SpanShift, const N: usize> SpanShift for smallvec::SmallVec<[T; N]> {
    fn shift_spans_after(&mut self, offset: u32, delta: i32) {
        for item in self {
            item.shift_spans_after(offset, delta);
        }
    }
}

impl<T: SpanShift> SpanShift for Box<T> {
    fn shift_spans_after(&mut self, offset: u32, delta: i32) {
        self.as_mut().shift_spans_after(offset, delta);
    }
}

impl<K, V: SpanShift> SpanShift for indexmap::IndexMap<K, V> {
    fn shift_spans_after(&mut self, offset: u32, delta: i32) {
        for value in self.values_mut() {
            value.shift_spans_after(offset, delta);
        }
    }
}
