//! Typed error surface for LSP backend request handling.
//!
//! Every `Result` boundary inside the backend returns [`LspBackendError`]
//! rather than a stringly `Result<T, String>`. The enum variants carry
//! enough structure for callers to classify failures without parsing
//! free-form text; `Display` still renders the same user-facing
//! messages the client sees on the wire.
//!
//! `Clone` / `Eq` / `PartialEq` are intentionally not derived:
//! [`LspBackendError::JsonSerializeFailed`] wraps a [`serde_json::Error`]
//! which does not implement them. Callers who need a snapshot should
//! stringify via `Display` and store the message.

use std::fmt::{self, Display};

use thiserror::Error;

/// The closed set of dependent tiers the LSP can reference in
/// alignment-missing / tier-missing diagnostics.
///
/// Narrower than [`talkbank_model::model::DependentTier`]'s 29
/// variants on purpose — this is the set of tiers that participate in
/// the alignment machinery the LSP surfaces to users. The `Display`
/// impl renders the tier with its `%` prefix (`"%mor"`, `"%gra"`, …)
/// so error messages read naturally.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TierName {
    /// `%mor` — morphological analysis tier.
    Mor,
    /// `%gra` — grammatical relation tier.
    Gra,
    /// `%pho` — actual phonological transcription tier.
    Pho,
    /// `%mod` — model (target) phonological transcription tier.
    Mod,
    /// `%sin` — sign / gesture tier.
    Sin,
}

impl Display for TierName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            TierName::Mor => "%mor",
            TierName::Gra => "%gra",
            TierName::Pho => "%pho",
            TierName::Mod => "%mod",
            TierName::Sin => "%sin",
        };
        f.write_str(s)
    }
}

/// Classified failures inside the LSP backend.
///
/// New variants are cheap to add — prefer a new variant over reusing
/// a catch-all when the failure is something a caller might want to
/// `match` on.
#[derive(Debug, Error)]
pub enum LspBackendError {
    /// A required dependent tier is absent from the utterance.
    #[error("No {tier} tier found")]
    MissingTier {
        /// Which tier is missing. `Display` renders with the `%`
        /// prefix so the error message reads naturally.
        tier: TierName,
    },

    /// The utterance has no computed alignment metadata at all.
    /// Callers should check `Utterance::alignments.is_some()` before
    /// asking for tier-specific alignment.
    #[error("No alignment metadata available")]
    AlignmentMetadataMissing,

    /// A specific per-tier alignment has not been computed on the
    /// utterance.
    #[error("No {tier} alignment computed")]
    TierAlignmentMissing {
        /// Which tier's alignment is absent.
        tier: TierName,
    },

    /// A graph-subsystem failure (edge construction, chunk lookup,
    /// `%gra` head resolution). Wraps
    /// [`GraphEdgeError`](crate::graph::GraphEdgeError) so callers
    /// that propagate with `?` do not need to pattern-match the
    /// specific graph variant.
    #[error(transparent)]
    Graph(#[from] crate::graph::GraphEdgeError),

    /// Wraps a failure from the tree-sitter-driven
    /// [`HighlightConfig`](crate::highlight::HighlightConfig) (grammar
    /// load, query compilation, or token extraction). The inner type is
    /// still a `String` because the upstream `tree-sitter-highlight`
    /// surface is stringly; the wrap lifts it into the typed error at
    /// the LSP boundary so handler callers match on the variant rather
    /// than on free-form substrings.
    #[error("Semantic highlighting failed: {reason}")]
    HighlightFailed {
        /// Free-form message propagated up from tree-sitter.
        reason: String,
    },

    /// The tree-sitter parser failed to produce a `ChatFile` from
    /// document text. `count` gives the total diagnostics; when `count
    /// > 0`, `first_message` carries the first diagnostic for the
    /// summary. When `count == 0`, the parser returned no `ChatFile`
    /// > *and* no diagnostics — a distinct failure mode callers may want
    /// > to flag separately (it usually indicates a parser bug rather
    /// > than malformed input).
    ///
    /// Handlers that want to show more than one diagnostic should
    /// consult the backend's validation cache instead — this variant
    /// is for contexts that fail fast (formatting, dependency graph).
    #[error("{}", format_parse_failure(*count, first_message.as_deref()))]
    ParseFailure {
        /// Number of diagnostics the parser emitted.
        count: usize,
        /// First diagnostic message for the summary shown to the user.
        /// `None` when `count == 0`.
        first_message: Option<String>,
    },

    /// A backing language-services subsystem failed to initialize on
    /// this thread (tree-sitter parser or semantic-tokens provider).
    /// Preserves the full structured
    /// [`BackendInitError`](super::state::BackendInitError) — which
    /// subsystem failed, and why — rather than stringifying at the
    /// boundary.
    #[error("Language services unavailable: {0}")]
    LanguageServicesUnavailable(#[from] super::state::BackendInitError),

    /// The document text for a URI is not in the backend's cache.
    /// Typically because the client never opened it or closed it before
    /// the request arrived; handlers should degrade gracefully rather
    /// than retry.
    #[error("Document not found")]
    DocumentNotFound,

    /// A regular-expression input failed to compile. Raised by the
    /// scoped-find handler when the user supplies an invalid pattern.
    #[error("Invalid regex: {reason}")]
    InvalidRegex {
        /// The regex compiler's error message.
        reason: String,
    },

    /// Serializing a response payload to JSON failed.
    ///
    /// **Deserialization of user input must NOT propagate through this
    /// variant.** `#[from]` is provided for ergonomic `?` on response
    /// serialization (`serde_json::to_value(...)?`), but the same
    /// `serde_json::Error` type is produced by
    /// `serde_json::from_value(...)` on inbound arguments — for those,
    /// wrap explicitly into [`LspBackendError::ArgumentInvalid`] so the
    /// failure is classified as user-facing rather than internal.
    #[error("Serialization error: {source}")]
    JsonSerializeFailed {
        /// Originating serde_json error.
        #[from]
        source: serde_json::Error,
    },

    /// A URI string was malformed. `label` identifies which URI
    /// argument was rejected (`"file URI"`, `"second file URI"`, …);
    /// `reason` carries the underlying parse error. Use
    /// [`LspBackendError::invalid_uri_parse`] to construct.
    #[error("Invalid {label}: {reason}")]
    InvalidUriParse {
        /// Which argument was rejected.
        label: &'static str,
        /// Parse-error message.
        reason: String,
    },

    /// A URI was well-formed but did not map to a local file path.
    /// Typically a non-`file://` scheme or a malformed host component.
    #[error("Invalid {label}: URI is not a file path")]
    UriNotFilePath {
        /// Which argument was rejected.
        label: &'static str,
    },

    /// An external service (CLAN analysis runner, database scanner,
    /// etc.) failed with a non-typed error. `service` identifies the
    /// call site; `reason` is the external crate's `Display` output.
    /// Kept as a catch-all so we aren't forced to add a new variant
    /// every time an upstream crate grows a new error enum.
    #[error("{service}: {reason}")]
    ExternalServiceFailed {
        /// Human-readable service label for the error prefix.
        service: &'static str,
        /// Originating error's `Display` output.
        reason: String,
    },

    /// An `executeCommand` request used an unrecognised command name.
    #[error("Unknown command: {name}")]
    UnknownCommand {
        /// The raw command string supplied by the client.
        name: String,
    },

    /// A required `executeCommand` argument was absent.
    #[error("Missing {label} argument")]
    ArgumentMissing {
        /// Argument name for the user-facing message.
        label: &'static str,
    },

    /// An `executeCommand` argument was present but failed type or
    /// value validation (wrong shape, malformed JSON, etc.).
    #[error("Invalid {label} argument: {reason}")]
    ArgumentInvalid {
        /// Argument name for the user-facing message.
        label: &'static str,
        /// Specific failure description (parser error, type mismatch).
        reason: String,
    },
}

impl LspBackendError {
    /// Build an [`InvalidUriParse`](Self::InvalidUriParse) from an
    /// upstream URI-parse error. The closure shape fits `.map_err(...)`
    /// directly so the 5+ call sites don't repeat `|e|
    /// LspBackendError::InvalidUriParse { label, reason: e.to_string()
    /// }`. Generic over `Display` to avoid pulling the `url` crate in
    /// as a direct dependency of this module.
    pub fn invalid_uri_parse<E: Display>(label: &'static str) -> impl Fn(E) -> Self {
        move |error| Self::InvalidUriParse {
            label,
            reason: error.to_string(),
        }
    }

    /// Build a [`UriNotFilePath`](Self::UriNotFilePath) error closure for
    /// use with `.map_err(_)`. The upstream `Url::to_file_path()` API
    /// returns `Result<PathBuf, ()>`, so this takes no input — the
    /// closure exists only to satisfy `.map_err` signature expectations.
    pub fn uri_not_file_path(label: &'static str) -> impl Fn(()) -> Self {
        move |()| Self::UriNotFilePath { label }
    }
}

/// Render the `ParseFailure` variant's `Display` message.
///
/// Kept out of line so the format attribute can call it without
/// repeating the pluralization + optional-message logic inline in a
/// `#[error]` template, which can't express either.
fn format_parse_failure(count: usize, first_message: Option<&str>) -> String {
    match (count, first_message) {
        (0, _) => "Failed to parse document (parser returned no diagnostics)".to_string(),
        (1, Some(msg)) => format!("Failed to parse document (1 diagnostic); first: {msg}"),
        (n, Some(msg)) => format!("Failed to parse document ({n} diagnostics); first: {msg}"),
        (n, None) => format!("Failed to parse document ({n} diagnostics)"),
    }
}
