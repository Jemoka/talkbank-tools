//! Pause tokens used in CHAT main-tier content.
//!
//! The model preserves both symbolic pause forms (`.`, `..`, `...`) and raw
//! numeric text for timed pauses so serialization roundtrips exactly.
//!
//! # CHAT Manual Reference
//!
//! - [Pauses](https://talkbank.org/0info/manuals/CHAT.html#Pauses)
//!
//! # Examples
//!
//! ```text
//! *CHI: I want (.) cookie .           # Short pause
//! *CHI: I (..) um .                   # Medium pause
//! *CHI: I (...) forgot .              # Long pause
//! *CHI: I (2.0) forgot .              # 2-second pause
//! ```

use super::WriteChat;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use talkbank_derive::{SemanticEq, SpanShift};

/// Encoded pause payload written inside `(...)`.
///
/// # CHAT Format Examples
///
/// ```text
/// *CHI: I want (.) cookie .              # Short pause
/// *CHI: I (..) um .                      # Medium pause
/// *CHI: I (...) forgot .                 # Long pause
/// *CHI: I (2.0) forgot .                 # 2-second pause
/// *CHI: I (0.5) think (.) so .           # Mixed pause types
/// ```
///
/// # Usage
///
/// - **Short** (`.`): Brief hesitation, typical in natural speech
/// - **Medium** (`..`): Moderate pause, often before a thought
/// - **Long** (`...`): Extended pause, significant hesitation
/// - **Timed** (numeric): Precise duration in seconds for detailed transcription
///
/// # References
///
/// - [Pauses](https://talkbank.org/0info/manuals/CHAT.html#Pauses)
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema, SemanticEq, SpanShift)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum PauseDuration {
    /// `.` short pause.
    Short,

    /// `..` medium pause.
    Medium,

    /// `...` long pause.
    Long,

    /// Numeric pause value preserved as source text (for example `1.5`, `2.0`).
    #[serde(rename = "timed")]
    Timed(PauseTimedDuration),
}

/// Pause marker serialized as `(payload)`.
///
/// # Duration Types
///
/// - **Short** (`.`): Brief pause
/// - **Medium** (`..`): Moderate pause
/// - **Long** (`...`): Extended pause
/// - **Timed** (`1.5`, `2.0`, etc.): Pause duration in seconds
///
/// # CHAT Manual Reference
///
/// - [Pause Markers](https://talkbank.org/0info/manuals/CHAT.html#Pauses)
///
/// # Examples
///
/// ```
/// use talkbank_model::model::{Pause, PauseDuration, PauseTimedDuration};
///
/// // Short pause
/// let pause = Pause::new(PauseDuration::Short);
/// assert_eq!(pause.to_string(), "(.)");
///
/// // Medium pause
/// let pause = Pause::new(PauseDuration::Medium);
/// assert_eq!(pause.to_string(), "(..)");
///
/// // Long pause
/// let pause = Pause::new(PauseDuration::Long);
/// assert_eq!(pause.to_string(), "(...)");
///
/// // Timed pause (2.5 seconds)
/// let pause = Pause::new(PauseDuration::Timed(PauseTimedDuration::new("2.5")));
/// assert_eq!(pause.to_string(), "(2.5)");
/// ```
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema, SemanticEq, SpanShift)]
pub struct Pause {
    /// Serialized payload between the parentheses.
    pub duration: PauseDuration,

    /// Source location metadata for diagnostics (not serialized).
    #[serde(skip)]
    #[schemars(skip)]
    pub span: crate::Span,
}

impl Pause {
    /// Build a pause token with dummy span metadata.
    ///
    /// Parser pipelines commonly construct with this first, then attach source
    /// spans after token boundaries are finalized.
    pub fn new(duration: PauseDuration) -> Self {
        Self {
            duration,
            span: crate::Span::DUMMY,
        }
    }

    /// Attach source span metadata used by diagnostics.
    ///
    /// Span data does not affect pause serialization or semantic equality.
    pub fn with_span(mut self, span: crate::Span) -> Self {
        self.span = span;
        self
    }
}

impl WriteChat for Pause {
    /// Serializes canonical CHAT pause syntax (`(.)`, `(..)`, `(1.5)`, ...).
    fn write_chat<W: std::fmt::Write>(&self, w: &mut W) -> std::fmt::Result {
        w.write_char('(')?;
        match &self.duration {
            PauseDuration::Short => w.write_char('.')?,
            PauseDuration::Medium => w.write_str("..")?,
            PauseDuration::Long => w.write_str("...")?,
            PauseDuration::Timed(duration) => w.write_str(duration.as_str())?,
        }
        w.write_char(')')
    }
}

/// Timed pause payload with typed integer components.
///
/// Preserves the raw source text for exact roundtrip while also parsing typed
/// numeric components (seconds + optional milliseconds) for programmatic access.
/// No floating-point values are stored — durations are integer-decomposed.
///
/// # Formats
///
/// ```text
/// "1.5"   → Parsed { seconds: 1, millis: Some(500), raw: "1.5" }
/// "2"     → Parsed { seconds: 2, millis: None,      raw: "2"   }
/// "0.5"   → Parsed { seconds: 0, millis: Some(500), raw: "0.5" }
/// "3."    → Parsed { seconds: 3, millis: None,      raw: "3."  }
/// "7:1.5" → Parsed { seconds: 421, millis: Some(500), raw: "7:1.5" }
/// "abc"   → Unsupported("abc")
/// ```
///
/// References:
/// - <https://talkbank.org/0info/manuals/CHAT.html#Pause_Numeric>
/// - <https://talkbank.org/0info/manuals/CHAT.html#Duration_Scope>
#[derive(Clone, Debug, PartialEq, SemanticEq, SpanShift, talkbank_derive::ValidationTagged)]
pub enum PauseTimedDuration {
    /// Successfully parsed duration: whole seconds + optional milliseconds.
    Parsed {
        /// Total whole seconds (for `7:1.5` this is `7*60 + 1 = 421`).
        seconds: u32,
        /// Fractional milliseconds (for `1.5` this is `Some(500)`).
        millis: Option<u32>,
        /// Original source text preserved for roundtrip fidelity.
        raw: smol_str::SmolStr,
    },
    /// Unrecognized duration text, preserved for roundtrip.
    Unsupported(String),
}

impl PauseTimedDuration {
    /// Parse a timed duration string into typed components.
    ///
    /// Handles formats: `"1.5"`, `"2"`, `"3."`, `"0.5"`, `"7:1.5"` (min:sec).
    /// Falls back to `Unsupported` for non-numeric text.
    pub fn new(text: impl Into<smol_str::SmolStr>) -> Self {
        let raw: smol_str::SmolStr = text.into();
        match Self::parse_components(&raw) {
            Some((secs, millis)) => Self::Parsed {
                seconds: secs,
                millis,
                raw,
            },
            None => Self::Unsupported(raw.to_string()),
        }
    }

    /// Returns the raw source text for serialization.
    pub fn as_str(&self) -> &str {
        match self {
            Self::Parsed { raw, .. } => raw,
            Self::Unsupported(s) => s,
        }
    }

    /// Returns total duration in milliseconds, or `None` for unsupported values.
    pub fn total_millis(&self) -> Option<u64> {
        match self {
            Self::Parsed {
                seconds, millis, ..
            } => Some(*seconds as u64 * 1000 + millis.unwrap_or(0) as u64),
            Self::Unsupported(_) => None,
        }
    }

    /// Parse `"1.5"` → `(1, Some(500))`, `"2"` → `(2, None)`, `"7:1.5"` → `(421, Some(500))`.
    fn parse_components(text: &str) -> Option<(u32, Option<u32>)> {
        // Handle colon format: "M:S" or "M:S.F"
        if let Some((minutes_str, rest)) = text.split_once(':') {
            let minutes: u32 = minutes_str.parse().ok()?;
            let (secs, millis) = Self::parse_seconds_frac(rest)?;
            return Some((minutes * 60 + secs, millis));
        }

        Self::parse_seconds_frac(text)
    }

    /// Parse `"1.5"` → `(1, Some(500))`, `"2"` → `(2, None)`, `"3."` → `(3, None)`.
    fn parse_seconds_frac(text: &str) -> Option<(u32, Option<u32>)> {
        if let Some((whole, frac)) = text.split_once('.') {
            let secs: u32 = whole.parse().ok()?;
            if frac.is_empty() {
                // "3." — trailing dot, no fractional part
                Some((secs, None))
            } else {
                let millis = Self::frac_to_millis(frac)?;
                Some((secs, Some(millis)))
            }
        } else {
            // Pure integer: "2"
            let secs: u32 = text.parse().ok()?;
            Some((secs, None))
        }
    }

    /// Convert fractional digits to milliseconds: "5" → 500, "50" → 500, "500" → 500, "25" → 250.
    fn frac_to_millis(frac: &str) -> Option<u32> {
        if !frac.bytes().all(|b| b.is_ascii_digit()) {
            return None;
        }
        // Pad or truncate to 3 digits
        let millis: u32 = match frac.len() {
            1 => frac.parse::<u32>().ok()? * 100,
            2 => frac.parse::<u32>().ok()? * 10,
            3 => frac.parse::<u32>().ok()?,
            _ => {
                // More than 3 digits: take first 3
                frac[..3].parse::<u32>().ok()?
            }
        };
        Some(millis)
    }
}

impl Serialize for PauseTimedDuration {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        // Serialize as `{ "seconds": "raw_text" }` for backward compatibility
        #[derive(Serialize)]
        struct Helper<'a> {
            seconds: &'a str,
        }
        Helper {
            seconds: self.as_str(),
        }
        .serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for PauseTimedDuration {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        struct Helper {
            seconds: String,
        }
        let h = Helper::deserialize(deserializer)?;
        Ok(Self::new(h.seconds))
    }
}

impl JsonSchema for PauseTimedDuration {
    fn schema_name() -> std::borrow::Cow<'static, str> {
        "PauseTimedDuration".into()
    }

    fn json_schema(_generator: &mut schemars::SchemaGenerator) -> schemars::Schema {
        schemars::json_schema!({
            "type": "object",
            "properties": {
                "seconds": { "type": "string" }
            },
            "required": ["seconds"]
        })
    }
}

impl std::fmt::Display for Pause {
    /// Formats a pause exactly as CHAT text (e.g., `(.)`, `(..)`, `(1.5)`).
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.write_chat(f)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Round-trips the short pause form `(.)`.
    #[test]
    fn pause_short_roundtrip() {
        let pause = Pause::new(PauseDuration::Short);
        let output = pause.to_string();
        assert_eq!(output, "(.)", "Short pause roundtrip failed");
    }

    /// Round-trips the medium pause form `(..)`.
    #[test]
    fn pause_medium_roundtrip() {
        let pause = Pause::new(PauseDuration::Medium);
        let output = pause.to_string();
        assert_eq!(output, "(..)", "Medium pause roundtrip failed");
    }

    /// Round-trips the long pause form `(...)`.
    #[test]
    fn pause_long_roundtrip() {
        let pause = Pause::new(PauseDuration::Long);
        let output = pause.to_string();
        assert_eq!(output, "(...)", "Long pause roundtrip failed");
    }

    /// Round-trips a numeric pause duration.
    #[test]
    fn pause_numeric_roundtrip() {
        let pause = Pause::new(PauseDuration::Timed(PauseTimedDuration::new("1.5")));
        let output = pause.to_string();
        assert_eq!(output, "(1.5)", "Numeric pause roundtrip failed");
    }

    #[test]
    fn timed_duration_decimal() {
        let d = PauseTimedDuration::new("1.5");
        assert!(matches!(
            d,
            PauseTimedDuration::Parsed {
                seconds: 1,
                millis: Some(500),
                ..
            }
        ));
        assert_eq!(d.as_str(), "1.5");
        assert_eq!(d.total_millis(), Some(1500));
    }

    #[test]
    fn timed_duration_integer() {
        let d = PauseTimedDuration::new("2");
        assert!(matches!(
            d,
            PauseTimedDuration::Parsed {
                seconds: 2,
                millis: None,
                ..
            }
        ));
        assert_eq!(d.total_millis(), Some(2000));
    }

    #[test]
    fn timed_duration_trailing_dot() {
        let d = PauseTimedDuration::new("3.");
        assert!(matches!(
            d,
            PauseTimedDuration::Parsed {
                seconds: 3,
                millis: None,
                ..
            }
        ));
        assert_eq!(d.as_str(), "3.");
    }

    #[test]
    fn timed_duration_colon_format() {
        let d = PauseTimedDuration::new("7:1.5");
        assert!(matches!(
            d,
            PauseTimedDuration::Parsed {
                seconds: 421,
                millis: Some(500),
                ..
            }
        ));
        assert_eq!(d.as_str(), "7:1.5");
        assert_eq!(d.total_millis(), Some(421_500));
    }

    #[test]
    fn timed_duration_zero_point_five() {
        let d = PauseTimedDuration::new("0.5");
        assert!(matches!(
            d,
            PauseTimedDuration::Parsed {
                seconds: 0,
                millis: Some(500),
                ..
            }
        ));
        assert_eq!(d.total_millis(), Some(500));
    }

    #[test]
    fn timed_duration_unsupported() {
        let d = PauseTimedDuration::new("abc");
        assert!(matches!(d, PauseTimedDuration::Unsupported(_)));
        assert_eq!(d.as_str(), "abc");
        assert_eq!(d.total_millis(), None);
    }

    #[test]
    fn timed_duration_has_validation_issue() {
        use crate::model::ValidationTagged;
        let parsed = PauseTimedDuration::new("1.5");
        assert!(!parsed.has_validation_issue());
        let unsupported = PauseTimedDuration::new("abc");
        assert!(unsupported.has_validation_issue());
    }
}
