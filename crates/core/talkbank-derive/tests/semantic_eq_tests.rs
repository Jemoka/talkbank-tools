// Integration tests for the SemanticEq derive macro.
//
// The macro generates `impl crate::model::SemanticEq`, so we must bring
// `talkbank_model::model` into the crate root so that `crate::model::*`
// resolves correctly in the generated code.

use talkbank_derive::SemanticEq as DeriveSemanticEq;
use talkbank_model::Span;
use talkbank_model::model;
use talkbank_model::model::SemanticEq;

// ---------------------------------------------------------------------------
// Test types
// ---------------------------------------------------------------------------

/// Named struct with a mix of semantic and skip fields.
#[derive(Debug, Clone, DeriveSemanticEq)]
struct Word {
    text: String,
    category: u32,
    #[semantic_eq(skip)]
    span: Span,
}

/// Empty (unit) struct -- should always be semantically equal to itself.
#[derive(Debug, Clone, DeriveSemanticEq)]
struct Empty;

/// Struct where every field is skipped -- always equal.
#[derive(Debug, Clone, DeriveSemanticEq)]
struct AllSkipped {
    #[semantic_eq(skip)]
    span: Span,
    #[semantic_eq(skip)]
    cache_key: u64,
}

/// Enum with unit, unnamed, and named variants.
#[derive(Debug, Clone, DeriveSemanticEq)]
enum Resolution {
    Single(String),
    Multiple(String, String),
    Unknown,
}

/// Nested struct: outer contains an inner SemanticEq type.
#[derive(Debug, Clone, DeriveSemanticEq)]
struct Inner {
    value: String,
}

#[derive(Debug, Clone, DeriveSemanticEq)]
struct Outer {
    inner: Inner,
    label: String,
    #[semantic_eq(skip)]
    span: Span,
}

/// Tuple struct.
#[derive(Debug, Clone, DeriveSemanticEq)]
struct Pair(String, u32);

/// Enum with only unit variants.
#[derive(Debug, Clone, DeriveSemanticEq)]
enum Color {
    Red,
    Green,
    Blue,
}

// ---------------------------------------------------------------------------
// Task 2: Core SemanticEq tests (8 tests)
// ---------------------------------------------------------------------------

#[test]
fn simple_struct_equal() {
    let a = Word {
        text: "hello".into(),
        category: 1,
        span: Span::new(0, 5),
    };
    let b = Word {
        text: "hello".into(),
        category: 1,
        span: Span::new(100, 200),
    };
    assert!(a.semantic_eq(&b));
}

#[test]
fn simple_struct_not_equal_text() {
    let a = Word {
        text: "hello".into(),
        category: 1,
        span: Span::new(0, 5),
    };
    let b = Word {
        text: "world".into(),
        category: 1,
        span: Span::new(0, 5),
    };
    assert!(!a.semantic_eq(&b));
}

#[test]
fn skip_field_ignored() {
    // Same semantic fields, different skipped spans.
    let a = Word {
        text: "x".into(),
        category: 42,
        span: Span::new(0, 1),
    };
    let b = Word {
        text: "x".into(),
        category: 42,
        span: Span::new(999, 1000),
    };
    assert!(a.semantic_eq(&b));
}

#[test]
fn non_skip_field_checked() {
    // Same text but different category (non-skipped) should differ.
    let a = Word {
        text: "x".into(),
        category: 1,
        span: Span::new(0, 1),
    };
    let b = Word {
        text: "x".into(),
        category: 2,
        span: Span::new(0, 1),
    };
    assert!(!a.semantic_eq(&b));
}

#[test]
fn empty_struct_always_equal() {
    assert!(Empty.semantic_eq(&Empty));
}

#[test]
fn all_skipped_struct_always_equal() {
    let a = AllSkipped {
        span: Span::new(0, 10),
        cache_key: 111,
    };
    let b = AllSkipped {
        span: Span::new(50, 60),
        cache_key: 999,
    };
    assert!(a.semantic_eq(&b));
}

#[test]
fn enum_same_variant_equal() {
    let a = Resolution::Single("eng".into());
    let b = Resolution::Single("eng".into());
    assert!(a.semantic_eq(&b));
}

#[test]
fn enum_different_variant_not_equal() {
    let a = Resolution::Single("eng".into());
    let b = Resolution::Unknown;
    assert!(!a.semantic_eq(&b));
}

// ---------------------------------------------------------------------------
// Task 7: Nested/complex SemanticEq tests (5 bonus)
// ---------------------------------------------------------------------------

#[test]
fn nested_struct_equal() {
    let a = Outer {
        inner: Inner {
            value: "abc".into(),
        },
        label: "ok".into(),
        span: Span::new(0, 10),
    };
    let b = Outer {
        inner: Inner {
            value: "abc".into(),
        },
        label: "ok".into(),
        span: Span::new(99, 199),
    };
    assert!(a.semantic_eq(&b));
}

#[test]
fn nested_struct_inner_differs() {
    let a = Outer {
        inner: Inner {
            value: "abc".into(),
        },
        label: "ok".into(),
        span: Span::new(0, 10),
    };
    let b = Outer {
        inner: Inner {
            value: "xyz".into(),
        },
        label: "ok".into(),
        span: Span::new(0, 10),
    };
    assert!(!a.semantic_eq(&b));
}

#[test]
fn tuple_struct_equal() {
    let a = Pair("hello".into(), 5);
    let b = Pair("hello".into(), 5);
    assert!(a.semantic_eq(&b));
}

#[test]
fn tuple_struct_not_equal() {
    let a = Pair("hello".into(), 5);
    let b = Pair("hello".into(), 6);
    assert!(!a.semantic_eq(&b));
}

#[test]
fn enum_unit_variants_equal() {
    assert!(Color::Red.semantic_eq(&Color::Red));
    assert!(Color::Green.semantic_eq(&Color::Green));
    assert!(!Color::Red.semantic_eq(&Color::Blue));
}
