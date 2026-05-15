use crate::SemanticEq;

/// Explicit parser outcome for streaming parse APIs.
///
/// This replaces ambiguous `Option<T>` parse returns:
/// - `Parsed(T)`: parser produced semantic output.
/// - `Rejected`: parser could not produce semantic output.
///
/// Parse diagnostics are still streamed through the provided `ErrorSink`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParseOutcome<T> {
    /// Parser produced semantic output.
    Parsed(T),
    /// Parser could not produce semantic output.
    Rejected,
}

impl<T> ParseOutcome<T> {
    /// Create a `Parsed` outcome wrapping the given value.
    #[inline]
    pub fn parsed(value: T) -> Self {
        Self::Parsed(value)
    }

    /// Create a `Rejected` outcome.
    #[inline]
    pub fn rejected() -> Self {
        Self::Rejected
    }

    /// Returns `true` if this outcome contains a parsed value.
    #[inline]
    pub fn is_parsed(&self) -> bool {
        matches!(self, Self::Parsed(_))
    }

    /// Returns `true` if parsing was rejected.
    #[inline]
    pub fn is_rejected(&self) -> bool {
        matches!(self, Self::Rejected)
    }

    /// Alias for [`is_parsed`](Self::is_parsed).
    #[inline]
    pub fn is_some(&self) -> bool {
        self.is_parsed()
    }

    /// Alias for [`is_rejected`](Self::is_rejected).
    #[inline]
    pub fn is_none(&self) -> bool {
        self.is_rejected()
    }

    /// Convert into an `Option`, mapping `Parsed(T)` to `Some(T)`.
    #[inline]
    pub fn into_option(self) -> Option<T> {
        match self {
            Self::Parsed(value) => Some(value),
            Self::Rejected => None,
        }
    }

    /// Convert to a reference-based outcome.
    #[inline]
    pub fn as_ref(&self) -> ParseOutcome<&T> {
        match self {
            Self::Parsed(value) => ParseOutcome::Parsed(value),
            Self::Rejected => ParseOutcome::Rejected,
        }
    }

    /// Map the parsed value using the given function.
    #[inline]
    pub fn map<U, F>(self, f: F) -> ParseOutcome<U>
    where
        F: FnOnce(T) -> U,
    {
        match self {
            Self::Parsed(value) => ParseOutcome::Parsed(f(value)),
            Self::Rejected => ParseOutcome::Rejected,
        }
    }

    /// Convert to `Result`, using the provided error if rejected.
    #[inline]
    pub fn ok_or<E>(self, err: E) -> Result<T, E> {
        match self {
            Self::Parsed(value) => Ok(value),
            Self::Rejected => Err(err),
        }
    }

    /// Convert to `Result`, lazily computing the error if rejected.
    #[inline]
    pub fn ok_or_else<E, F>(self, err: F) -> Result<T, E>
    where
        F: FnOnce() -> E,
    {
        match self {
            Self::Parsed(value) => Ok(value),
            Self::Rejected => Err(err()),
        }
    }

    /// Combine two outcomes, returning `Parsed` only if both are parsed.
    #[inline]
    pub fn zip<U>(self, other: ParseOutcome<U>) -> ParseOutcome<(T, U)> {
        match (self, other) {
            (Self::Parsed(lhs), ParseOutcome::Parsed(rhs)) => ParseOutcome::Parsed((lhs, rhs)),
            _ => ParseOutcome::Rejected,
        }
    }
}

impl<T> From<Option<T>> for ParseOutcome<T> {
    /// Convert an `Option` into a parser outcome.
    fn from(value: Option<T>) -> Self {
        match value {
            Some(value) => Self::Parsed(value),
            None => Self::Rejected,
        }
    }
}

impl<T> From<ParseOutcome<T>> for Option<T> {
    /// Convert a parser outcome into an `Option`.
    fn from(value: ParseOutcome<T>) -> Self {
        value.into_option()
    }
}

impl<T: SemanticEq> SemanticEq for ParseOutcome<T> {
    /// Compare outcomes using semantic equality on parsed values.
    fn semantic_eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Parsed(lhs), Self::Parsed(rhs)) => lhs.semantic_eq(rhs),
            (Self::Rejected, Self::Rejected) => true,
            _ => false,
        }
    }
}
