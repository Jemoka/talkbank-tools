//! Parse diagnostics and error infrastructure.

use miette::{Diagnostic, SourceSpan};
use std::collections::BTreeSet;
use std::fmt;
use std::ops::Range;
use thiserror::Error;

/// A span in the source input (byte offsets).
pub type Span = Range<usize>;

/// Kinds of dependent tiers, for tracking parse health.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum TierKind {
    Mor,
    Gra,
    Pho,
    Sin,
    Wor,
    Act,
    Cod,
    Com,
    Other,
}

/// Tracks which parts of an utterance failed to parse.
///
/// Downstream consumers check this before operating on partial data.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ParseHealth {
    pub tainted_tiers: BTreeSet<TierKind>,
}

impl ParseHealth {
    pub fn taint(&mut self, tier: TierKind) {
        self.tainted_tiers.insert(tier);
    }

    pub fn is_clean(&self) -> bool {
        self.tainted_tiers.is_empty()
    }

    pub fn is_tainted(&self, tier: TierKind) -> bool {
        self.tainted_tiers.contains(&tier)
    }
}

/// A parse diagnostic with source location.
#[derive(Debug, Clone, Error, Diagnostic)]
pub enum ParseDiagnostic {
    #[error("unexpected token: {message}")]
    #[diagnostic(code(chat::unexpected_token))]
    UnexpectedToken {
        #[source_code]
        src: String,
        #[label("{message}")]
        span: SourceSpan,
        message: String,
    },

    #[error("lexer error: {message}")]
    #[diagnostic(code(chat::lexer_error))]
    LexerError {
        #[source_code]
        src: String,
        #[label("{message}")]
        span: SourceSpan,
        message: String,
    },

    #[error("missing expected token: {expected}")]
    #[diagnostic(code(chat::missing_token), help("expected {expected}"))]
    MissingToken {
        #[source_code]
        src: String,
        #[label("expected {expected} here")]
        span: SourceSpan,
        expected: String,
    },

    #[error("orphan dependent tier")]
    #[diagnostic(
        code(chat::orphan_dependent_tier),
        help("dependent tiers must follow a main tier (*SPK:)")
    )]
    OrphanDependentTier {
        #[source_code]
        src: String,
        #[label("this dependent tier has no preceding main tier")]
        span: SourceSpan,
    },
}

/// Severity of a diagnostic.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    Error,
    Warning,
}

impl fmt::Display for Severity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Error => write!(f, "error"),
            Self::Warning => write!(f, "warning"),
        }
    }
}

/// Parse result: AST + accumulated diagnostics.
#[derive(Debug)]
pub struct ParseResult<T> {
    /// The (possibly partial) parse output.
    pub value: T,
    /// All diagnostics accumulated during parsing.
    pub diagnostics: Vec<ParseDiagnostic>,
}

impl<T> ParseResult<T> {
    pub fn has_errors(&self) -> bool {
        !self.diagnostics.is_empty()
    }
}
