//! Typed model for `%tim` (timing) dependent tier.
//!
//! Reference: <https://talkbank.org/0info/manuals/CHAT.html#Timing_Tier>

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use talkbank_derive::{SemanticEq, SpanShift, ValidationTagged};

use crate::model::NonEmptyString;
use crate::model::TimeValue;
use crate::model::header::parse_time_value;

/// A time segment in a `%tim` tier: single time or range.
#[derive(Debug, Clone, PartialEq, Eq, Hash, SemanticEq, SpanShift)]
pub enum TimSegment {
    /// A single time point (e.g. `7:55`).
    Single(TimeValue),
    /// A range between two time points (e.g. `00:01:30-00:02:00`).
    Range {
        /// Start of the range.
        start: TimeValue,
        /// End of the range.
        end: TimeValue,
    },
}

/// Timing tier content from `%tim`.
///
/// Time-like content (tokens with colons and digits, e.g. `7:55` or
/// `00:01:30-00:02:00`) is parsed into structured [`TimSegment`]s.
/// Free-text descriptions (e.g. `afternoon session`) are stored as
/// `Unsupported` and flagged by validation (E603).
///
/// Reference: <https://talkbank.org/0info/manuals/CHAT.html#Timing_Tier>
#[derive(Clone, Debug, PartialEq, SemanticEq, SpanShift, ValidationTagged)]
pub enum TimTier {
    /// Structured time-like content.
    Parsed {
        /// Structured time segments extracted from the text.
        #[span_shift(skip)]
        segments: Vec<TimSegment>,
        /// Raw text payload preserved for roundtrip.
        #[span_shift(skip)]
        content: NonEmptyString,
        /// Source span for error reporting.
        #[semantic_eq(skip)]
        #[span_shift(skip)]
        span: crate::Span,
    },
    /// Non-time content (free text like "afternoon session").
    Unsupported {
        /// Raw text payload.
        #[span_shift(skip)]
        content: NonEmptyString,
        /// Source span for error reporting.
        #[semantic_eq(skip)]
        #[span_shift(skip)]
        span: crate::Span,
    },
}

impl TimTier {
    /// Parse a `%tim` tier body, classifying as `Parsed` or `Unsupported`.
    pub fn from_text(content: NonEmptyString) -> Self {
        if let Some(segments) = parse_tim_segments(content.as_str()) {
            Self::Parsed {
                segments,
                content,
                span: crate::Span::DUMMY,
            }
        } else {
            Self::Unsupported {
                content,
                span: crate::Span::DUMMY,
            }
        }
    }

    /// Sets the source span.
    pub fn with_span(mut self, span: crate::Span) -> Self {
        match &mut self {
            Self::Parsed { span: s, .. } => *s = span,
            Self::Unsupported { span: s, .. } => *s = span,
        }
        self
    }

    /// Returns the raw text content.
    pub fn as_str(&self) -> &str {
        match self {
            Self::Parsed { content, .. } => content.as_str(),
            Self::Unsupported { content, .. } => content.as_str(),
        }
    }

    /// Returns the source span.
    pub fn span(&self) -> crate::Span {
        match self {
            Self::Parsed { span, .. } => *span,
            Self::Unsupported { span, .. } => *span,
        }
    }

    /// Returns the structured time segments (empty for `Unsupported`).
    pub fn segments(&self) -> &[TimSegment] {
        match self {
            Self::Parsed { segments, .. } => segments,
            Self::Unsupported { .. } => &[],
        }
    }
}

/// Parse whitespace-separated time tokens from `%tim` content.
///
/// Each token is either a single time or a hyphen-separated range.
fn parse_tim_segments(s: &str) -> Option<Vec<TimSegment>> {
    let trimmed = s.trim();
    if trimmed.is_empty() {
        return None;
    }

    let mut segments = Vec::new();
    for token in trimmed.split_whitespace() {
        // Try as a range (hyphen-separated).
        if let Some((left, right)) = token.split_once('-') {
            let start = parse_time_value(left)?;
            let end = parse_time_value(right)?;
            segments.push(TimSegment::Range { start, end });
        } else {
            let tv = parse_time_value(token)?;
            segments.push(TimSegment::Single(tv));
        }
    }

    if segments.is_empty() {
        None
    } else {
        Some(segments)
    }
}

impl std::fmt::Display for TimTier {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

// --- Serde: serialize/deserialize as plain string for backward compat ---

impl Serialize for TimTier {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for TimTier {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let s = String::deserialize(deserializer)?;
        let content = NonEmptyString::new(&s)
            .ok_or_else(|| serde::de::Error::custom("TimTier content cannot be empty"))?;
        Ok(Self::from_text(content))
    }
}

impl JsonSchema for TimTier {
    fn schema_name() -> std::borrow::Cow<'static, str> {
        "TimTier".into()
    }

    fn json_schema(_generator: &mut schemars::SchemaGenerator) -> schemars::Schema {
        schemars::json_schema!({ "type": "string" })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parsed_single_time() {
        let content = NonEmptyString::new("7:55").unwrap();
        let tim = TimTier::from_text(content);
        assert!(matches!(tim, TimTier::Parsed { .. }));
        assert_eq!(tim.segments().len(), 1);
        match &tim.segments()[0] {
            TimSegment::Single(tv) => {
                assert_eq!((tv.hours, tv.minutes, tv.seconds), (0, 7, 55));
            }
            other => panic!("expected Single, got {:?}", other),
        }
    }

    #[test]
    fn parsed_range() {
        let content = NonEmptyString::new("00:01:30-00:02:00").unwrap();
        let tim = TimTier::from_text(content);
        assert_eq!(tim.segments().len(), 1);
        assert!(matches!(tim.segments()[0], TimSegment::Range { .. }));
    }

    #[test]
    fn parsed_multiple_tokens() {
        let content = NonEmptyString::new("7:55 00:01:30-00:02:00").unwrap();
        let tim = TimTier::from_text(content);
        assert_eq!(tim.segments().len(), 2);
    }

    #[test]
    fn parsed_bare_seconds() {
        let content = NonEmptyString::new("45").unwrap();
        let tim = TimTier::from_text(content);
        assert!(matches!(tim, TimTier::Parsed { .. }));
        assert_eq!(tim.segments().len(), 1);
        match &tim.segments()[0] {
            TimSegment::Single(tv) => {
                assert_eq!((tv.hours, tv.minutes, tv.seconds), (0, 0, 45));
            }
            other => panic!("expected Single, got {:?}", other),
        }
    }

    #[test]
    fn unsupported_free_text() {
        let content = NonEmptyString::new("afternoon session").unwrap();
        let tim = TimTier::from_text(content);
        assert!(matches!(tim, TimTier::Unsupported { .. }));
        assert_eq!(tim.segments().len(), 0);
    }

    #[test]
    fn roundtrip_text() {
        let input = "00:01:30-00:02:00";
        let content = NonEmptyString::new(input).unwrap();
        let tim = TimTier::from_text(content);
        assert_eq!(tim.as_str(), input);
    }
}
