//! Typed age value for `@ID` header field 4.
//!
//! Format: `years;months.days` (e.g., `3;06.15`, `2;08`, `1;04.`).
//!
//! Reference: <https://talkbank.org/0info/manuals/CHAT.html#Age_Field>

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use smol_str::SmolStr;
use talkbank_derive::{SemanticEq, SpanShift, ValidationTagged};

/// Age string recorded in `@ID` (field 4, format `years;months.days`).
///
/// Successfully parsed ages store typed numeric components; malformed ages
/// are preserved as `Unsupported` so the validator can report actionable errors.
///
/// Reference: <https://talkbank.org/0info/manuals/CHAT.html#Age_Field>
#[derive(Debug, Clone, PartialEq, Eq, Hash, SemanticEq, SpanShift, ValidationTagged)]
pub enum AgeValue {
    /// Successfully parsed age.
    Valid {
        /// Years component.
        #[span_shift(skip)]
        years: u16,
        /// Months component (0–11).
        #[span_shift(skip)]
        months: Option<u8>,
        /// Days component (0–30).
        #[span_shift(skip)]
        days: Option<u8>,
        /// Original text preserved for exact roundtrip.
        #[semantic_eq(skip)]
        #[span_shift(skip)]
        raw: SmolStr,
    },
    /// Unrecognized value preserved for validation.
    Unsupported(String),
}

impl AgeValue {
    /// Parse a CHAT age string (`years;months.days`).
    ///
    /// Returns `Valid` for well-formed ages, `Unsupported` otherwise.
    pub fn from_text(value: &str) -> Self {
        let Some((years_str, rest)) = value.split_once(';') else {
            return Self::Unsupported(value.to_string());
        };

        if years_str.is_empty() || !years_str.bytes().all(|b| b.is_ascii_digit()) {
            return Self::Unsupported(value.to_string());
        }

        let Ok(years) = years_str.parse::<u16>() else {
            return Self::Unsupported(value.to_string());
        };

        let (months, days) = if let Some((months_str, days_str)) = rest.split_once('.') {
            let months = if months_str.is_empty() {
                None
            } else if months_str.bytes().all(|b| b.is_ascii_digit()) {
                Some(months_str.parse::<u8>().unwrap_or(0))
            } else {
                return Self::Unsupported(value.to_string());
            };

            let days = if days_str.is_empty() {
                None
            } else if days_str.bytes().all(|b| b.is_ascii_digit()) {
                Some(days_str.parse::<u8>().unwrap_or(0))
            } else {
                return Self::Unsupported(value.to_string());
            };

            (months, days)
        } else if rest.is_empty() {
            (None, None)
        } else if rest.bytes().all(|b| b.is_ascii_digit()) {
            (Some(rest.parse::<u8>().unwrap_or(0)), None)
        } else {
            return Self::Unsupported(value.to_string());
        };

        Self::Valid {
            years,
            months,
            days,
            raw: SmolStr::from(value),
        }
    }

    /// Returns the age as a string.
    ///
    /// Returns the original text for both valid and unsupported ages.
    pub fn as_str(&self) -> &str {
        match self {
            Self::Valid { raw, .. } => raw.as_str(),
            Self::Unsupported(s) => s.as_str(),
        }
    }

    /// Returns true when a structurally-parseable age does not match any
    /// of the three date patterns that CLAN's authoritative `depfile.cut`
    /// declares legal for `@ID` field 4:
    ///
    /// ```text
    /// @d<yy;>  @d<yy;mm.>  @d<yy;mm.dd>
    /// ```
    ///
    /// Concretely, the raw text must be exactly one of:
    ///
    /// - `YY;` — year, semicolon, nothing else
    /// - `YY;MM.` — year, semicolon, two-digit month, trailing period
    /// - `YY;MM.DD` — year, semicolon, two-digit month, period, two-digit day
    ///
    /// Anything else — one-digit month (`3;0`), two-digit month without
    /// period (`2;06`), single-digit month with period (`3;0.15`),
    /// single-digit day (`3;06.5`) — is rejected by CLAN CHECK as error 34
    /// ("Illegal date representation"). This predicate exists to make
    /// Rust chatter match that behavior.
    ///
    /// Note: `Unsupported` is already caught by `has_validation_issue()`
    /// (the derive-macro-generated predicate on the `Valid` vs
    /// `Unsupported` tag), so this method returns `false` for
    /// `Unsupported` to avoid double-reporting. The two checks are
    /// chained in `check_id_header`.
    pub fn violates_depfile_pattern(&self) -> bool {
        let Self::Valid { raw, .. } = self else {
            return false;
        };

        let raw = raw.as_str();
        let Some((years, rest)) = raw.split_once(';') else {
            return true;
        };
        if years.is_empty() || !years.bytes().all(|b| b.is_ascii_digit()) {
            return true;
        }

        // Matches `yy;` — year plus semicolon, nothing after.
        if rest.is_empty() {
            return false;
        }

        // Anything non-empty after the semicolon must contain a period —
        // depfile.cut has no template for `yy;mm` without trailing dot.
        let Some((months, days)) = rest.split_once('.') else {
            return true;
        };

        // `mm` must be exactly two digits.
        if months.len() != 2 || !months.bytes().all(|b| b.is_ascii_digit()) {
            return true;
        }

        // Matches `yy;mm.` — year, two-digit month, trailing period.
        if days.is_empty() {
            return false;
        }

        // `dd` (when present) must be exactly two digits.
        if days.len() != 2 || !days.bytes().all(|b| b.is_ascii_digit()) {
            return true;
        }

        // Matches `yy;mm.dd`.
        false
    }

    /// Backward-compatible constructor matching the old `string_newtype` API.
    pub fn new(value: impl AsRef<str>) -> Self {
        Self::from_text(value.as_ref())
    }
}

impl std::fmt::Display for AgeValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl crate::model::WriteChat for AgeValue {
    fn write_chat<W: std::fmt::Write>(&self, w: &mut W) -> std::fmt::Result {
        w.write_str(self.as_str())
    }
}

impl Serialize for AgeValue {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for AgeValue {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let s = String::deserialize(deserializer)?;
        Ok(Self::from_text(&s))
    }
}

impl JsonSchema for AgeValue {
    fn schema_name() -> std::borrow::Cow<'static, str> {
        "AgeValue".into()
    }

    fn json_schema(_generator: &mut schemars::SchemaGenerator) -> schemars::Schema {
        schemars::json_schema!({ "type": "string" })
    }
}

impl From<String> for AgeValue {
    fn from(value: String) -> Self {
        Self::from_text(&value)
    }
}

impl From<&str> for AgeValue {
    fn from(value: &str) -> Self {
        Self::from_text(value)
    }
}
