//! Typed time values for `@Time Duration` and `@Time Start` headers.
//!
//! Shared types: [`TimeValue`] represents a single parsed time point,
//! [`TimeSegment`] represents a single time or a range, and both are
//! re-used by `@Time Duration`, `@Time Start`, and the `%tim` tier.
//!
//! References:
//! - <https://talkbank.org/0info/manuals/CHAT.html#Time_Duration_Header>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Time_Start_Header>

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use smol_str::SmolStr;
use talkbank_derive::{SemanticEq, SpanShift, ValidationTagged};

// ---------------------------------------------------------------------------
// Shared structured time types
// ---------------------------------------------------------------------------

/// A single parsed time point (e.g. `01:23:45` or `23:45.678`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, SemanticEq, SpanShift)]
pub struct TimeValue {
    /// Hours component (0 for `MM:SS` format).
    #[span_shift(skip)]
    pub hours: u32,
    /// Minutes component.
    #[span_shift(skip)]
    pub minutes: u32,
    /// Seconds component.
    #[span_shift(skip)]
    pub seconds: u32,
    /// Optional milliseconds component.
    #[span_shift(skip)]
    pub millis: Option<u32>,
}

/// A time segment: either a single time or a range of two times.
#[derive(Debug, Clone, PartialEq, Eq, Hash, SemanticEq, SpanShift)]
pub enum TimeSegment {
    /// A single time point (e.g. `01:23:45`).
    Single(TimeValue),
    /// A range between two time points (e.g. `00:00:00-01:30:00`).
    Range {
        /// Start of the range.
        start: TimeValue,
        /// End of the range.
        end: TimeValue,
    },
}

// ---------------------------------------------------------------------------
// TimeDurationValue
// ---------------------------------------------------------------------------

/// Duration value from `@Time Duration`.
///
/// Formats: `HH:MM:SS`, `HH:MM:SS-HH:MM:SS`, `HH:MM-HH:MM`,
/// comma-separated combinations, and ranges with `;` separator.
///
/// Reference: <https://talkbank.org/0info/manuals/CHAT.html#Time_Duration_Header>
#[derive(Debug, Clone, PartialEq, Eq, Hash, SemanticEq, SpanShift, ValidationTagged)]
pub enum TimeDurationValue {
    /// Successfully parsed time duration with structured segments.
    Parsed {
        /// Structured time segments extracted from the text.
        #[span_shift(skip)]
        segments: Vec<TimeSegment>,
        /// Original text preserved for roundtrip.
        #[semantic_eq(skip)]
        #[span_shift(skip)]
        raw: SmolStr,
    },
    /// Unrecognized value preserved for validation.
    Unsupported(String),
}

impl TimeDurationValue {
    /// Parse a CHAT time duration string.
    ///
    /// Returns `Parsed` for well-formed durations, `Unsupported` otherwise.
    pub fn from_text(value: &str) -> Self {
        if value.is_empty() {
            return Self::Parsed {
                segments: Vec::new(),
                raw: SmolStr::from(value),
            };
        }

        match parse_duration_segments(value) {
            Some(segments) => Self::Parsed {
                segments,
                raw: SmolStr::from(value),
            },
            None => Self::Unsupported(value.to_string()),
        }
    }

    /// Returns the time duration as a string.
    pub fn as_str(&self) -> &str {
        match self {
            Self::Parsed { raw, .. } => raw.as_str(),
            Self::Unsupported(s) => s.as_str(),
        }
    }

    /// Returns the structured segments (empty for `Unsupported`).
    pub fn segments(&self) -> &[TimeSegment] {
        match self {
            Self::Parsed { segments, .. } => segments,
            Self::Unsupported(_) => &[],
        }
    }

    /// Backward-compatible constructor.
    pub fn new(value: impl AsRef<str>) -> Self {
        Self::from_text(value.as_ref())
    }

    /// Returns true when a structurally-parseable duration does not match
    /// any of the three patterns that CLAN's authoritative `depfile.cut`
    /// declares legal for `@Time Duration`:
    ///
    /// ```text
    /// @t<hh:mm-hh:mm>  @t<hh:mm:ss-hh:mm:ss>  @t<hh:mm:ss>
    /// ```
    ///
    /// Concretely, the raw text must be exactly one of:
    ///
    /// - `HH:MM:SS`
    /// - `HH:MM-HH:MM`
    /// - `HH:MM:SS-HH:MM:SS`
    ///
    /// Anything else — semicolon separator, comma-joined multi-segment
    /// values, `MM:SS` two-component form — is rejected by CLAN CHECK,
    /// and this predicate mirrors that. `Unsupported` is caught by
    /// `has_validation_issue()` so this method returns `false` for it
    /// to avoid double-reporting.
    ///
    /// Component widths are not fixed (`\d+`, not `\d{2}`) because
    /// real corpora use both `1:30` and `01:30`; depfile.cut's slots
    /// admit any digit count.
    pub fn violates_depfile_pattern(&self) -> bool {
        let Self::Parsed { raw, .. } = self else {
            return false;
        };
        !matches_duration_depfile(raw.as_str())
    }
}

/// Single-time form `HH:MM:SS` — the `@t<hh:mm:ss>` template.
fn is_hms_strict(s: &str) -> bool {
    let mut parts = s.split(':');
    let Some(h) = parts.next() else { return false };
    let Some(m) = parts.next() else { return false };
    let Some(sec) = parts.next() else {
        return false;
    };
    if parts.next().is_some() {
        return false;
    }
    all_ascii_digits(h) && all_ascii_digits(m) && all_ascii_digits(sec)
}

/// Two-component form `HH:MM` — used on one side of an
/// `HH:MM-HH:MM` range.
fn is_hm_strict(s: &str) -> bool {
    let mut parts = s.split(':');
    let Some(h) = parts.next() else { return false };
    let Some(m) = parts.next() else { return false };
    if parts.next().is_some() {
        return false;
    }
    all_ascii_digits(h) && all_ascii_digits(m)
}

fn all_ascii_digits(s: &str) -> bool {
    !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit())
}

fn matches_duration_depfile(raw: &str) -> bool {
    // HH:MM:SS
    if is_hms_strict(raw) {
        return true;
    }
    // HH:MM-HH:MM or HH:MM:SS-HH:MM:SS — a single hyphen at top level
    // with matching component shapes on both sides. Not semicolon,
    // not comma.
    if let Some((left, right)) = raw.split_once('-')
        && !right.contains('-')
    {
        return (is_hm_strict(left) && is_hm_strict(right))
            || (is_hms_strict(left) && is_hms_strict(right));
    }
    false
}

/// Parse comma-separated time segments from a duration string.
fn parse_duration_segments(duration: &str) -> Option<Vec<TimeSegment>> {
    let mut segments = Vec::new();
    for segment in duration.split(',') {
        let segment = segment.trim();
        if segment.is_empty() {
            continue;
        }
        let parsed = if let Some((left, right)) = segment.split_once('-') {
            let start = parse_time_value(left)?;
            let end = parse_time_value(right)?;
            TimeSegment::Range { start, end }
        } else if let Some((left, right)) = segment.split_once(';') {
            let start = parse_time_value(left)?;
            let end = parse_time_value(right)?;
            TimeSegment::Range { start, end }
        } else {
            TimeSegment::Single(parse_time_value(segment)?)
        };
        segments.push(parsed);
    }
    Some(segments)
}

/// Parse a single `HH:MM:SS`, `MM:SS`, bare `SS`, or any with `.mmm` millis.
///
/// depfile.cut defines `@t<ss>` (bare seconds) as valid for `%tim`, so we
/// accept 1-part, 2-part, and 3-part time values.
pub(crate) fn parse_time_value(s: &str) -> Option<TimeValue> {
    let (hms_part, millis) = if let Some((hms, ms)) = s.split_once('.') {
        let millis: u32 = ms.parse().ok()?;
        (hms, Some(millis))
    } else {
        (s, None)
    };

    let parts: Vec<&str> = hms_part.split(':').collect();
    match parts.len() {
        1 => {
            let seconds: u32 = parts[0].parse().ok()?;
            Some(TimeValue {
                hours: 0,
                minutes: 0,
                seconds,
                millis,
            })
        }
        2 => {
            let minutes: u32 = parts[0].parse().ok()?;
            let seconds: u32 = parts[1].parse().ok()?;
            Some(TimeValue {
                hours: 0,
                minutes,
                seconds,
                millis,
            })
        }
        3 => {
            let hours: u32 = parts[0].parse().ok()?;
            let minutes: u32 = parts[1].parse().ok()?;
            let seconds: u32 = parts[2].parse().ok()?;
            Some(TimeValue {
                hours,
                minutes,
                seconds,
                millis,
            })
        }
        _ => None,
    }
}

impl std::fmt::Display for TimeDurationValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl crate::model::WriteChat for TimeDurationValue {
    fn write_chat<W: std::fmt::Write>(&self, w: &mut W) -> std::fmt::Result {
        w.write_str(self.as_str())
    }
}

impl Serialize for TimeDurationValue {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for TimeDurationValue {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let s = String::deserialize(deserializer)?;
        Ok(Self::from_text(&s))
    }
}

impl JsonSchema for TimeDurationValue {
    fn schema_name() -> std::borrow::Cow<'static, str> {
        "TimeDurationValue".into()
    }

    fn json_schema(_generator: &mut schemars::SchemaGenerator) -> schemars::Schema {
        schemars::json_schema!({ "type": "string" })
    }
}

impl From<String> for TimeDurationValue {
    fn from(value: String) -> Self {
        Self::from_text(&value)
    }
}

impl From<&str> for TimeDurationValue {
    fn from(value: &str) -> Self {
        Self::from_text(value)
    }
}

// ---------------------------------------------------------------------------
// TimeStartValue
// ---------------------------------------------------------------------------

/// Starting time value from `@Time Start`.
///
/// Formats: `MM:SS`, `HH:MM:SS`, or either with `.mmm` milliseconds.
///
/// Reference: <https://talkbank.org/0info/manuals/CHAT.html#Time_Start_Header>
#[derive(Debug, Clone, PartialEq, Eq, Hash, SemanticEq, SpanShift, ValidationTagged)]
pub enum TimeStartValue {
    /// Successfully parsed time start.
    Parsed {
        /// Hours component.
        #[span_shift(skip)]
        hours: u32,
        /// Minutes component.
        #[span_shift(skip)]
        minutes: u32,
        /// Seconds component.
        #[span_shift(skip)]
        seconds: u32,
        /// Optional milliseconds component.
        #[span_shift(skip)]
        millis: Option<u32>,
        /// Original text preserved for roundtrip.
        #[semantic_eq(skip)]
        #[span_shift(skip)]
        raw: SmolStr,
    },
    /// Unrecognized value preserved for validation.
    Unsupported(String),
}

impl TimeStartValue {
    /// Parse a CHAT time start string (`MM:SS`, `HH:MM:SS`, or either with `.mmm`).
    ///
    /// Returns `Parsed` for well-formed times, `Unsupported` otherwise.
    pub fn from_text(value: &str) -> Self {
        if value.is_empty() {
            return Self::Parsed {
                hours: 0,
                minutes: 0,
                seconds: 0,
                millis: None,
                raw: SmolStr::from(value),
            };
        }

        if let Some(tv) = parse_time_value(value) {
            Self::Parsed {
                hours: tv.hours,
                minutes: tv.minutes,
                seconds: tv.seconds,
                millis: tv.millis,
                raw: SmolStr::from(value),
            }
        } else {
            Self::Unsupported(value.to_string())
        }
    }

    /// Returns the time start as a string.
    pub fn as_str(&self) -> &str {
        match self {
            Self::Parsed { raw, .. } => raw.as_str(),
            Self::Unsupported(s) => s.as_str(),
        }
    }

    /// Backward-compatible constructor.
    pub fn new(value: impl AsRef<str>) -> Self {
        Self::from_text(value.as_ref())
    }

    /// Returns true when a structurally-parseable start time does not
    /// match either of the two patterns that CLAN's authoritative
    /// `depfile.cut` declares legal for `@Time Start`:
    ///
    /// ```text
    /// @t<hh:mm:ss>  @t<mm:ss>
    /// ```
    ///
    /// Concretely, the raw text must be exactly one of:
    ///
    /// - `HH:MM:SS`
    /// - `MM:SS`
    ///
    /// Anything else — millisecond suffix, range form, bare seconds —
    /// is rejected by CLAN CHECK. `Unsupported` is already caught by
    /// `has_validation_issue()`, so this method returns `false` for
    /// it to avoid double-reporting.
    pub fn violates_depfile_pattern(&self) -> bool {
        let Self::Parsed { raw, .. } = self else {
            return false;
        };
        let raw = raw.as_str();
        // HH:MM:SS or MM:SS — no dots, no hyphens, exactly 2 or 3
        // colon-separated numeric components.
        if raw.contains('.') || raw.contains('-') {
            return true;
        }
        if is_hms_strict(raw) {
            return false;
        }
        if is_hm_strict(raw) {
            return false;
        }
        true
    }
}

impl std::fmt::Display for TimeStartValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl crate::model::WriteChat for TimeStartValue {
    fn write_chat<W: std::fmt::Write>(&self, w: &mut W) -> std::fmt::Result {
        w.write_str(self.as_str())
    }
}

impl Serialize for TimeStartValue {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for TimeStartValue {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let s = String::deserialize(deserializer)?;
        Ok(Self::from_text(&s))
    }
}

impl JsonSchema for TimeStartValue {
    fn schema_name() -> std::borrow::Cow<'static, str> {
        "TimeStartValue".into()
    }

    fn json_schema(_generator: &mut schemars::SchemaGenerator) -> schemars::Schema {
        schemars::json_schema!({ "type": "string" })
    }
}

impl From<String> for TimeStartValue {
    fn from(value: String) -> Self {
        Self::from_text(&value)
    }
}

impl From<&str> for TimeStartValue {
    fn from(value: &str) -> Self {
        Self::from_text(value)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn time_value_parse_hms() {
        let tv = parse_time_value("01:23:45").unwrap();
        assert_eq!(
            (tv.hours, tv.minutes, tv.seconds, tv.millis),
            (1, 23, 45, None)
        );
    }

    #[test]
    fn time_value_parse_ms() {
        let tv = parse_time_value("23:45").unwrap();
        assert_eq!(
            (tv.hours, tv.minutes, tv.seconds, tv.millis),
            (0, 23, 45, None)
        );
    }

    #[test]
    fn time_value_parse_bare_seconds() {
        let tv = parse_time_value("45").unwrap();
        assert_eq!(
            (tv.hours, tv.minutes, tv.seconds, tv.millis),
            (0, 0, 45, None)
        );
    }

    #[test]
    fn time_value_parse_bare_seconds_with_millis() {
        let tv = parse_time_value("45.678").unwrap();
        assert_eq!(
            (tv.hours, tv.minutes, tv.seconds, tv.millis),
            (0, 0, 45, Some(678))
        );
    }

    #[test]
    fn time_value_parse_with_millis() {
        let tv = parse_time_value("01:23:45.678").unwrap();
        assert_eq!(
            (tv.hours, tv.minutes, tv.seconds, tv.millis),
            (1, 23, 45, Some(678))
        );
    }

    #[test]
    fn duration_single() {
        let d = TimeDurationValue::from_text("01:23:45");
        assert!(matches!(d, TimeDurationValue::Parsed { .. }));
        assert_eq!(d.segments().len(), 1);
        assert!(matches!(d.segments()[0], TimeSegment::Single(_)));
    }

    #[test]
    fn duration_range_hyphen() {
        let d = TimeDurationValue::from_text("00:00:00-01:30:00");
        assert_eq!(d.segments().len(), 1);
        assert!(matches!(d.segments()[0], TimeSegment::Range { .. }));
    }

    #[test]
    fn duration_range_semicolon() {
        let d = TimeDurationValue::from_text("00:00:00;01:30:00");
        assert_eq!(d.segments().len(), 1);
        assert!(matches!(d.segments()[0], TimeSegment::Range { .. }));
    }

    #[test]
    fn duration_comma_separated() {
        let d = TimeDurationValue::from_text("01:00:00, 02:00:00-03:00:00");
        assert_eq!(d.segments().len(), 2);
        assert!(matches!(d.segments()[0], TimeSegment::Single(_)));
        assert!(matches!(d.segments()[1], TimeSegment::Range { .. }));
    }

    #[test]
    fn duration_unsupported() {
        let d = TimeDurationValue::from_text("foobar");
        assert!(matches!(d, TimeDurationValue::Unsupported(_)));
        assert_eq!(d.segments().len(), 0);
    }

    #[test]
    fn duration_roundtrip() {
        for input in &["01:23:45", "00:00:00-01:30:00", "01:00:00, 02:00:00"] {
            let d = TimeDurationValue::from_text(input);
            assert_eq!(d.as_str(), *input);
        }
    }

    #[test]
    fn start_hms() {
        let s = TimeStartValue::from_text("01:23:45");
        assert!(matches!(
            s,
            TimeStartValue::Parsed {
                hours: 1,
                minutes: 23,
                seconds: 45,
                ..
            }
        ));
    }

    #[test]
    fn start_ms() {
        let s = TimeStartValue::from_text("23:45");
        assert!(matches!(
            s,
            TimeStartValue::Parsed {
                hours: 0,
                minutes: 23,
                seconds: 45,
                ..
            }
        ));
    }

    #[test]
    fn start_with_millis() {
        let s = TimeStartValue::from_text("01:23:45.678");
        assert!(matches!(
            s,
            TimeStartValue::Parsed {
                hours: 1,
                minutes: 23,
                seconds: 45,
                millis: Some(678),
                ..
            }
        ));
    }
}
