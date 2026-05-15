# talkbank-derive

**Status:** Current
**Last updated:** 2026-04-29 07:53 EDT

Derive macros for talkbank-model.

## Overview

This crate provides procedural macros for automatic trait implementations
used throughout the TalkBank ecosystem. These macros reduce boilerplate
for common patterns in the CHAT data model:

- **`#[derive(SemanticEq)]`** — Semantic equality comparison that ignores
  positional information (spans). Fields can be excluded with
  `#[semantic_eq(skip)]`.
- **`#[derive(SpanShift)]`** — Recursively shifts source spans by an offset,
  used when splicing or rewriting CHAT content.
- **`#[derive(ValidationTagged)]`** — Maps enum variants to validation
  significance tags (error, warning, or clean).
- **`#[error_code_enum]`** — Attribute macro for error code enums that
  generates serde rename rules, `as_str()`, `new()`, and `Display` impls.

## Usage

```rust
use talkbank_derive::{SemanticEq, SpanShift};

#[derive(SemanticEq, SpanShift)]
struct MyNode {
    content: String,
    #[semantic_eq(skip)]
    #[span_shift(skip)]
    debug_info: Option<String>,
}
```

## License

BSD-3-Clause. See [LICENSE](../../LICENSE) for details.
