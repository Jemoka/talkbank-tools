//! Typed calendar date for CHAT `@Date` and `@Birth of` headers.
//!
//! Format: `DD-MMM-YYYY` (e.g., `01-JAN-2024`).
//!
//! Reference: <https://talkbank.org/0info/manuals/CHAT.html#Date_Header>

use crate::validation::{Validate, ValidationContext};
use crate::{ErrorCode, ErrorContext, ParseError, Severity, SourceLocation};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use smol_str::SmolStr;
use talkbank_derive::{SemanticEq, SpanShift, ValidationTagged};

/// Three-letter month abbreviations used in CHAT dates.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, SemanticEq, SpanShift)]
pub enum Month {
    /// January.
    Jan,
    /// February.
    Feb,
    /// March.
    Mar,
    /// April.
    Apr,
    /// May.
    May,
    /// June.
    Jun,
    /// July.
    Jul,
    /// August.
    Aug,
    /// September.
    Sep,
    /// October.
    Oct,
    /// November.
    Nov,
    /// December.
    Dec,
}

impl Month {
    /// Parse a three-letter month abbreviation (case-sensitive, uppercase).
    fn from_text(s: &str) -> Option<Self> {
        match s {
            "JAN" => Some(Self::Jan),
            "FEB" => Some(Self::Feb),
            "MAR" => Some(Self::Mar),
            "APR" => Some(Self::Apr),
            "MAY" => Some(Self::May),
            "JUN" => Some(Self::Jun),
            "JUL" => Some(Self::Jul),
            "AUG" => Some(Self::Aug),
            "SEP" => Some(Self::Sep),
            "OCT" => Some(Self::Oct),
            "NOV" => Some(Self::Nov),
            "DEC" => Some(Self::Dec),
            _ => None,
        }
    }

    /// Returns the canonical three-letter uppercase abbreviation.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Jan => "JAN",
            Self::Feb => "FEB",
            Self::Mar => "MAR",
            Self::Apr => "APR",
            Self::May => "MAY",
            Self::Jun => "JUN",
            Self::Jul => "JUL",
            Self::Aug => "AUG",
            Self::Sep => "SEP",
            Self::Oct => "OCT",
            Self::Nov => "NOV",
            Self::Dec => "DEC",
        }
    }
}

/// Calendar date value for `@Date` and `@Birth of` headers.
///
/// Successfully parsed dates store typed components; malformed dates are
/// preserved as `Unsupported` so the validator can report actionable errors.
///
/// Reference: <https://talkbank.org/0info/manuals/CHAT.html#Date_Header>
#[derive(Debug, Clone, PartialEq, Eq, Hash, SemanticEq, SpanShift, ValidationTagged)]
pub enum ChatDate {
    /// Successfully parsed `DD-MMM-YYYY` date.
    Valid {
        /// Day of month (1–31).
        #[span_shift(skip)]
        day: u8,
        /// Month abbreviation.
        #[span_shift(skip)]
        month: Month,
        /// Four-digit year.
        #[span_shift(skip)]
        year: u16,
        /// Original text preserved for roundtrip.
        #[semantic_eq(skip)]
        #[span_shift(skip)]
        raw: SmolStr,
    },
    /// Unrecognized value preserved for validation.
    Unsupported(String),
}

impl ChatDate {
    /// Parse a CHAT date string (`DD-MMM-YYYY`).
    ///
    /// Returns `Valid` for well-formed dates, `Unsupported` otherwise.
    pub fn from_text(value: &str) -> Self {
        let parts: Vec<&str> = value.split('-').collect();
        if parts.len() != 3 {
            return Self::Unsupported(value.to_string());
        }

        let (day_str, month_str, year_str) = (parts[0], parts[1], parts[2]);

        let Some(day) = parse_day(day_str) else {
            return Self::Unsupported(value.to_string());
        };

        let Some(month) = Month::from_text(month_str) else {
            return Self::Unsupported(value.to_string());
        };

        let Some(year) = parse_year(year_str) else {
            return Self::Unsupported(value.to_string());
        };

        Self::Valid {
            day,
            month,
            year,
            raw: SmolStr::from(value),
        }
    }

    /// Returns the date as a string.
    ///
    /// Returns the original text for both valid and unsupported dates.
    pub fn as_str(&self) -> &str {
        match self {
            Self::Valid { raw, .. } => raw.as_str(),
            Self::Unsupported(s) => s.as_str(),
        }
    }

    /// Backward-compatible constructor matching the old `string_newtype` API.
    pub fn new(value: impl AsRef<str>) -> Self {
        Self::from_text(value.as_ref())
    }
}

/// Parse a two-digit day (01–31).
fn parse_day(s: &str) -> Option<u8> {
    if s.len() != 2 {
        return None;
    }
    let day: u8 = s.parse().ok()?;
    if (1..=31).contains(&day) {
        Some(day)
    } else {
        None
    }
}

/// Parse a four-digit year.
fn parse_year(s: &str) -> Option<u16> {
    if s.len() != 4 {
        return None;
    }
    s.parse().ok()
}

impl std::fmt::Display for ChatDate {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl crate::model::WriteChat for ChatDate {
    fn write_chat<W: std::fmt::Write>(&self, w: &mut W) -> std::fmt::Result {
        w.write_str(self.as_str())
    }
}

impl Serialize for ChatDate {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for ChatDate {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let s = String::deserialize(deserializer)?;
        Ok(Self::from_text(&s))
    }
}

impl JsonSchema for ChatDate {
    fn schema_name() -> std::borrow::Cow<'static, str> {
        "ChatDate".into()
    }

    fn json_schema(_generator: &mut schemars::SchemaGenerator) -> schemars::Schema {
        schemars::json_schema!({ "type": "string" })
    }
}

impl From<String> for ChatDate {
    fn from(value: String) -> Self {
        Self::from_text(&value)
    }
}

impl From<&str> for ChatDate {
    fn from(value: &str) -> Self {
        Self::from_text(value)
    }
}

impl Validate for ChatDate {
    /// Reports `InvalidDateFormat` for `Unsupported` dates.
    fn validate(&self, _context: &ValidationContext, errors: &impl crate::ErrorSink) {
        if let Self::Unsupported(raw) = self {
            errors.report(
                ParseError::new(
                    ErrorCode::InvalidDateFormat,
                    Severity::Error,
                    SourceLocation::at_offset(0),
                    ErrorContext::new(raw, 0..raw.len(), raw.as_str()),
                    format!("Invalid @Date format '{}': expected DD-MMM-YYYY", raw),
                )
                .with_suggestion(
                    "Use format: 01-JAN-2024 (two-digit day, uppercase month, four-digit year)",
                ),
            );
        }
    }
}
