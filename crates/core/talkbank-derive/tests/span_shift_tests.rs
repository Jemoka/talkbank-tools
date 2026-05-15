// Integration tests for the SpanShift derive macro.
//
// The macro generates `impl talkbank_model::SpanShift`, which resolves
// because `talkbank-model` is a dev-dependency.

use talkbank_derive::SpanShift;
use talkbank_model::{Span, SpanShift as SpanShiftTrait};

// ---------------------------------------------------------------------------
// Test types
// ---------------------------------------------------------------------------

/// Simple struct with a single Span field.
#[derive(Debug, Clone, SpanShift)]
struct Located {
    span: Span,
    value: String,
}

/// Struct containing Option<Span>.
#[derive(Debug, Clone, SpanShift)]
struct MaybeLocated {
    span: Option<Span>,
}

/// Struct containing Vec of inner SpanShift types.
#[derive(Debug, Clone, SpanShift)]
struct Container {
    items: Vec<Located>,
}

/// Struct with a skipped field.
#[derive(Debug, Clone, SpanShift)]
struct WithSkipped {
    span: Span,
    #[span_shift(skip)]
    frozen_span: Span,
}

// ---------------------------------------------------------------------------
// Task 3: SpanShift tests (8 tests)
// ---------------------------------------------------------------------------

#[test]
fn span_at_offset_gets_shifted() {
    let mut loc = Located {
        span: Span::new(10, 20),
        value: "x".into(),
    };
    loc.shift_spans_after(10, 5);
    assert_eq!(loc.span.start, 15);
    assert_eq!(loc.span.end, 25);
}

#[test]
fn span_before_offset_untouched() {
    let mut loc = Located {
        span: Span::new(5, 10),
        value: "x".into(),
    };
    loc.shift_spans_after(15, 5);
    assert_eq!(loc.span.start, 5);
    assert_eq!(loc.span.end, 10);
}

#[test]
fn positive_delta_insertion() {
    // Insert 100 bytes at position 0 -- everything shifts.
    let mut loc = Located {
        span: Span::new(50, 80),
        value: "x".into(),
    };
    loc.shift_spans_after(0, 100);
    assert_eq!(loc.span.start, 150);
    assert_eq!(loc.span.end, 180);
}

#[test]
fn negative_delta_deletion() {
    // Delete 5 bytes at position 10 -- span at 20..30 shifts to 15..25.
    let mut loc = Located {
        span: Span::new(20, 30),
        value: "x".into(),
    };
    loc.shift_spans_after(10, -5);
    assert_eq!(loc.span.start, 15);
    assert_eq!(loc.span.end, 25);
}

#[test]
fn option_some_shifts() {
    let mut m = MaybeLocated {
        span: Some(Span::new(10, 20)),
    };
    m.shift_spans_after(5, 3);
    let span = m.span.expect("should still be Some");
    assert_eq!(span.start, 13);
    assert_eq!(span.end, 23);
}

#[test]
fn option_none_noop() {
    let mut m = MaybeLocated { span: None };
    m.shift_spans_after(0, 10);
    assert!(m.span.is_none());
}

#[test]
fn vec_recursion_all_elements_shifted() {
    let mut c = Container {
        items: vec![
            Located {
                span: Span::new(10, 20),
                value: "a".into(),
            },
            Located {
                span: Span::new(30, 40),
                value: "b".into(),
            },
            Located {
                span: Span::new(5, 8),
                value: "c".into(),
            },
        ],
    };
    c.shift_spans_after(10, 7);
    // Items at or after offset 10 get shifted.
    assert_eq!(c.items[0].span.start, 17);
    assert_eq!(c.items[0].span.end, 27);
    assert_eq!(c.items[1].span.start, 37);
    assert_eq!(c.items[1].span.end, 47);
    // Item before offset 10 is untouched.
    assert_eq!(c.items[2].span.start, 5);
    assert_eq!(c.items[2].span.end, 8);
}

#[test]
fn skip_field_not_shifted() {
    let mut w = WithSkipped {
        span: Span::new(10, 20),
        frozen_span: Span::new(10, 20),
    };
    w.shift_spans_after(0, 5);
    // Non-skipped span shifted.
    assert_eq!(w.span.start, 15);
    assert_eq!(w.span.end, 25);
    // Skipped span unchanged.
    assert_eq!(w.frozen_span.start, 10);
    assert_eq!(w.frozen_span.end, 20);
}
