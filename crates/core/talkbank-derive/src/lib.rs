#![warn(missing_docs)]
//! Procedural derive macros for the TalkBank CHAT data model.
//!
//! This crate centralizes derive-time code generation so model crates can keep
//! runtime implementations small and consistent. It eliminates boilerplate that
//! would otherwise need manual maintenance across the dozens of AST types in
//! [`talkbank_model`].
//!
//! # Provided macros
//!
//! | Macro | Trait generated | Purpose |
//! |-------|----------------|---------|
//! | `#[derive(SemanticEq)]` | `SemanticEq` + `SemanticDiff` | Compare AST nodes ignoring spans and metadata |
//! | `#[derive(SpanShift)]` | `SpanShift` | Shift byte-offset spans after edits |
//! | `#[derive(ValidationTagged)]` | `ValidationTagged` | Map enum variants to `ValidationTag::{Clean,Warning,Error}` |
//! | `#[error_code_enum]` | (attribute) | Generate `as_str()`, `new()`, `Display`, and serde glue for error code enums |
//!
//! # Contract with consuming crates
//!
//! Generated impls target paths under `crate::model` and `talkbank_model`.
//! This is intentional for the TalkBank workspace, where the derives are used
//! by `talkbank-model` and sibling crates with that shared layout.
//!
//! # Examples
//!
//! ## `SemanticEq` -- ignore span fields in equality checks
//!
//! The tree-sitter and direct parsers produce different byte spans for the same
//! input. `SemanticEq` lets equivalence tests compare only the linguistic content:
//!
//! ```ignore
//! use talkbank_derive::SemanticEq;
//!
//! #[derive(SemanticEq)]
//! struct Word {
//!     pub text: String,
//!     pub category: Option<WordCategory>,
//!     #[semantic_eq(skip)]
//!     pub span: Span,  // ignored during comparison
//! }
//!
//! // Also generates `SemanticDiff` for structured diff output:
//! // word_a.semantic_diff(&word_b) -> Vec<Difference>
//! ```
//!
//! ## `SpanShift` -- adjust spans after source edits
//!
//! When an edit inserts or removes bytes from a CHAT file, all spans after
//! the edit point must be shifted. `SpanShift` recurses into nested fields:
//!
//! ```ignore
//! use talkbank_derive::SpanShift;
//!
//! #[derive(SpanShift)]
//! struct Utterance {
//!     pub span: Span,
//!     pub words: Vec<Word>,        // recursively shifted
//!     pub dependent_tiers: Vec<DependentTier>,
//!     #[span_shift(skip)]
//!     pub cached_hash: u64,        // not shifted
//! }
//! ```
//!
//! ## `ValidationTagged` -- classify enum variants for validation
//!
//! Validation code often needs to know whether a resolved enum state is
//! clean, a warning, or an error. The derive uses naming conventions with
//! explicit overrides:
//!
//! ```ignore
//! use talkbank_derive::ValidationTagged;
//!
//! #[derive(ValidationTagged)]
//! enum LanguageResolution {
//!     Single(LanguageCode),           // -> Clean (default)
//!     Multiple(Vec<LanguageCode>),    // -> Clean (default)
//!     Ambiguous(Vec<LanguageCode>),   // -> Clean (default)
//!     #[validation_tag(error)]
//!     Unresolved,                     // -> Error (explicit override)
//! }
//!
//! // Convention-based detection also works:
//! #[derive(ValidationTagged)]
//! enum ParseState {
//!     Clean,              // -> Clean (default)
//!     ParseError,         // -> Error (suffix "Error")
//!     DeferredWarning,    // -> Warning (suffix "Warning")
//! }
//! ```
//!
//! ## `error_code_enum` -- error code string/serde/display glue
//!
//! Annotate each variant with `#[code("E###")]` to get serde rename
//! attributes, `as_str()`, `new()`, `Display`, and `documentation_url()`
//! from a single source of truth:
//!
//! ```ignore
//! use talkbank_derive::error_code_enum;
//!
//! #[error_code_enum]
//! pub enum ErrorCode {
//!     #[code("E001")]
//!     InternalError,
//!     #[code("E101")]
//!     InvalidLineFormat,
//!     #[code("E201")]
//!     MissingBeginHeader,
//! }
//!
//! // Generated methods:
//! // ErrorCode::InternalError.as_str()           -> "E001"
//! // ErrorCode::new("E101")                      -> Some(ErrorCode::InvalidLineFormat)
//! // format!("{}", ErrorCode::MissingBeginHeader) -> "E201"
//! // ErrorCode::InternalError.documentation_url() -> "https://..."
//! ```
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Format>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Main_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>

use proc_macro::TokenStream;
use quote::quote;
use syn::{DeriveInput, parse_macro_input};

mod error_code_enum;
mod helpers;
mod semantic_diff;
mod semantic_eq;
mod span_shift;
mod validation_tagged;

/// Derive macro for the `SemanticEq` trait (defined in `talkbank_model::model`).
///
/// Generates a field-by-field semantic equality check. By default all fields are
/// compared. Mark fields with `#[semantic_eq(skip)]` to exclude them -- typically
/// `span`, `parse_health`, or other runtime metadata that differs between parsers.
///
/// Also emits the companion `SemanticDiff` implementation so both equality and
/// structured diffing stay in lock-step. `SemanticDiff` produces a `Vec<Difference>`
/// describing which fields diverged, useful for diagnosing parser equivalence failures.
///
/// Works on structs and enums. For enums, variants are compared by discriminant
/// first, then by payload fields.
///
/// # Attributes
///
/// - `#[semantic_eq(skip)]` -- exclude a field from comparison (and from diff output).
///
/// # Example
///
/// ```ignore
/// use talkbank_derive::SemanticEq;
///
/// #[derive(SemanticEq)]
/// struct Word {
///     pub text: String,
///     pub category: Option<WordCategory>,
///     #[semantic_eq(skip)]
///     pub span: Span,              // ignored: differs between parsers
///     #[semantic_eq(skip)]
///     pub parse_health: ParseHealth, // ignored: runtime metadata
/// }
///
/// let a = Word { text: "hello".into(), category: None, span: Span::new(0, 5), .. };
/// let b = Word { text: "hello".into(), category: None, span: Span::new(10, 15), .. };
/// assert!(a.semantic_eq(&b)); // spans differ but semantic content matches
/// ```
#[proc_macro_derive(SemanticEq, attributes(semantic_eq))]
pub fn derive_semantic_eq(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);

    let semantic_eq_impl = semantic_eq::impl_semantic_eq(&input);
    let semantic_diff_impl = semantic_diff::impl_semantic_diff(&input);

    let expanded = quote! {
        #semantic_eq_impl
        #semantic_diff_impl
    };

    TokenStream::from(expanded)
}

/// Derive macro for the `SpanShift` trait (defined in `talkbank_model`).
///
/// Generates `shift_spans_after(&mut self, offset: u32, delta: i32)` which
/// recursively walks all fields and shifts any `Span` values whose start is at
/// or after `offset` by `delta` bytes. This is essential for incremental editing
/// in the LSP: when the user types into a CHAT file, all spans after the edit
/// point must be adjusted.
///
/// Use `#[span_shift(skip)]` to exclude fields that should not be shifted
/// (e.g., cached hashes, indices into external data).
///
/// Works on structs and enums. For container types (`Vec<T>`, `Option<T>`),
/// the generated code recurses into each element.
///
/// # Attributes
///
/// - `#[span_shift(skip)]` -- exclude a field from span shifting.
///
/// # Example
///
/// ```ignore
/// use talkbank_derive::SpanShift;
///
/// #[derive(SpanShift)]
/// struct Utterance {
///     pub span: Span,
///     pub words: Vec<Word>,          // recursively shifted
///     pub dependent_tiers: Vec<DependentTier>,
///     #[span_shift(skip)]
///     pub cached_hash: u64,          // not shifted
/// }
///
/// // After inserting 10 bytes at position 50:
/// utterance.shift_spans_after(50, 10);
/// ```
#[proc_macro_derive(SpanShift, attributes(span_shift))]
pub fn derive_span_shift(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let expanded = span_shift::impl_span_shift(&input);
    TokenStream::from(expanded)
}

/// Derive macro for the `ValidationTagged` trait (defined in `talkbank_model::model`).
///
/// Generates `fn validation_tag(&self) -> ValidationTag` for enums, mapping each
/// variant to one of `ValidationTag::Clean`, `ValidationTag::Warning`, or
/// `ValidationTag::Error`. Validation code uses this to decide whether a resolved
/// state needs an error/warning diagnostic.
///
/// # Resolution order
///
/// 1. **Explicit annotation** -- `#[validation_tag(error)]`, `#[validation_tag(warning)]`,
///    or `#[validation_tag(clean)]` on the variant.
/// 2. **Naming convention** -- variants whose name ends in `Error` map to
///    `ValidationTag::Error`; those ending in `Warning` map to `ValidationTag::Warning`.
/// 3. **Default** -- `ValidationTag::Clean`.
///
/// # Example
///
/// ```ignore
/// use talkbank_derive::ValidationTagged;
///
/// #[derive(ValidationTagged)]
/// enum LanguageResolution {
///     Single(LanguageCode),           // -> Clean (default)
///     Multiple(Vec<LanguageCode>),    // -> Clean (default)
///     #[validation_tag(error)]
///     Unresolved,                     // -> Error (explicit)
/// }
///
/// #[derive(ValidationTagged)]
/// enum ParseState {
///     Clean,              // -> Clean (default)
///     ParseError,         // -> Error (suffix "Error")
///     DeferredWarning,    // -> Warning (suffix "Warning")
///     #[validation_tag(error)]
///     ExplicitProblem,    // -> Error (explicit override)
/// }
///
/// assert!(ParseState::ParseError.is_validation_error());
/// assert!(ParseState::DeferredWarning.is_validation_warning());
/// assert!(!ParseState::Clean.is_validation_error());
/// ```
#[proc_macro_derive(ValidationTagged, attributes(validation_tag))]
pub fn derive_validation_tagged(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let expanded = validation_tagged::impl_validation_tagged(&input);
    TokenStream::from(expanded)
}

/// Attribute macro for error code enum generation.
///
/// Annotate an enum with `#[error_code_enum]` and mark each variant with
/// `#[code("E###")]`. The macro generates:
///
/// - **`#[serde(rename = "E###")]`** on each variant for JSON/TOML serialization.
/// - **`fn as_str(&self) -> &'static str`** -- returns the code string (e.g., `"E001"`).
/// - **`fn new(code: &str) -> Option<Self>`** -- parses a code string back to a variant.
/// - **`impl Display`** -- formats as the code string.
/// - **`fn documentation_url(&self) -> String`** -- returns the canonical docs URL.
///
/// This eliminates the fragile double-maintenance pattern where code strings,
/// serde renames, and display impls must be kept in sync manually.
///
/// # Example
///
/// ```ignore
/// use talkbank_derive::error_code_enum;
///
/// #[error_code_enum]
/// pub enum ErrorCode {
///     /// Internal error (unexpected condition).
///     #[code("E001")]
///     InternalError,
///     /// Missing @Begin header.
///     #[code("E201")]
///     MissingBeginHeader,
///     /// Invalid line format.
///     #[code("E101")]
///     InvalidLineFormat,
/// }
///
/// // Generated API:
/// assert_eq!(ErrorCode::InternalError.as_str(), "E001");
/// assert_eq!(ErrorCode::new("E201"), Some(ErrorCode::MissingBeginHeader));
/// assert_eq!(format!("{}", ErrorCode::InvalidLineFormat), "E101");
/// ```
#[proc_macro_attribute]
pub fn error_code_enum(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let expanded = error_code_enum::impl_error_code_enum(item.into());
    TokenStream::from(expanded)
}
