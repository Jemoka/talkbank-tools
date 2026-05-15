//! Typed model for the `@ID` socioeconomic status (SES) field.
//!
//! The SES field in `@ID` encodes ethnicity, socioeconomic code, or both.
//! Values are parsed from text via `SesValue::from_text()`. Unrecognized
//! values are preserved as `Unsupported` and flagged by the validator (E546).
//!
//! Reference: <https://talkbank.org/0info/manuals/CHAT.html#SES_Field>

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use talkbank_derive::{SemanticEq, SpanShift, ValidationTagged};

/// Ethnicity codes recognized in the SES field.
#[derive(Debug, Clone, PartialEq, Eq, Hash, SemanticEq, SpanShift)]
pub enum Ethnicity {
    /// `White`
    White,
    /// `Black`
    Black,
    /// `Asian`
    Asian,
    /// `Latino`
    Latino,
    /// `Pacific`
    Pacific,
    /// `Native`
    Native,
    /// `Multiple`
    Multiple,
    /// `Unknown`
    Unknown,
    /// Unrecognized ethnicity string.
    Unsupported(String),
}

impl Ethnicity {
    /// Parse an ethnicity string.
    fn from_text(s: &str) -> Self {
        match s {
            "White" => Self::White,
            "Black" => Self::Black,
            "Asian" => Self::Asian,
            "Latino" => Self::Latino,
            "Pacific" => Self::Pacific,
            "Native" => Self::Native,
            "Multiple" => Self::Multiple,
            "Unknown" => Self::Unknown,
            _ => Self::Unsupported(s.to_string()),
        }
    }

    /// Returns the canonical text representation.
    fn as_str(&self) -> &str {
        match self {
            Self::White => "White",
            Self::Black => "Black",
            Self::Asian => "Asian",
            Self::Latino => "Latino",
            Self::Native => "Native",
            Self::Pacific => "Pacific",
            Self::Multiple => "Multiple",
            Self::Unknown => "Unknown",
            Self::Unsupported(s) => s.as_str(),
        }
    }
}

/// Socioeconomic status codes recognized in the SES field.
#[derive(Debug, Clone, PartialEq, Eq, Hash, SemanticEq, SpanShift)]
pub enum SesCode {
    /// `UC` — upper class
    UC,
    /// `MC` — middle class
    MC,
    /// `WC` — working class
    WC,
    /// `LI` — low income
    LI,
    /// Unrecognized SES code.
    Unsupported(String),
}

impl SesCode {
    /// Parse an SES code string.
    fn from_text(s: &str) -> Self {
        match s {
            "UC" => Self::UC,
            "MC" => Self::MC,
            "WC" => Self::WC,
            "LI" => Self::LI,
            _ => Self::Unsupported(s.to_string()),
        }
    }

    /// Returns the canonical text representation.
    fn as_str(&self) -> &str {
        match self {
            Self::UC => "UC",
            Self::MC => "MC",
            Self::WC => "WC",
            Self::LI => "LI",
            Self::Unsupported(s) => s.as_str(),
        }
    }
}

/// Parsed SES value from the `@ID` header.
///
/// The SES field can contain an ethnicity code, a socioeconomic code, both
/// (space-separated), or an unrecognized free-text value.
#[derive(Debug, Clone, PartialEq, Eq, Hash, SemanticEq, SpanShift, ValidationTagged)]
pub enum SesValue {
    /// Ethnicity only (e.g., `White`).
    EthOnly(Ethnicity),
    /// Socioeconomic status only (e.g., `WC`).
    SesOnly(SesCode),
    /// Both ethnicity and SES code (e.g., `White UC`).
    Combined {
        /// Ethnicity component.
        eth: Ethnicity,
        /// Socioeconomic status component.
        ses: SesCode,
    },
    /// Unrecognized value preserved for validation.
    Unsupported(String),
}

impl SesValue {
    /// Parse a CHAT SES field string.
    ///
    /// Recognizes ethnicity codes, SES codes, combined values (space- or
    /// comma-separated, e.g. `White UC` or `White,MC`), and falls back to
    /// `Unsupported` for anything else. The input is trimmed before
    /// classification so callers need not pre-trim.
    pub fn from_text(value: &str) -> Self {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            return Self::Unsupported(value.to_string());
        }

        // Try combined "Ethnicity<sep>SES" format.
        // Real corpus data uses both space ("White UC") and comma ("White,MC").
        if let Some(combined) = Self::try_parse_combined(trimmed) {
            return combined;
        }

        // Try as standalone SES code.
        let ses = SesCode::from_text(trimmed);
        if !matches!(ses, SesCode::Unsupported(_)) {
            return Self::SesOnly(ses);
        }

        // Try as standalone ethnicity.
        let eth = Ethnicity::from_text(trimmed);
        if !matches!(eth, Ethnicity::Unsupported(_)) {
            return Self::EthOnly(eth);
        }

        Self::Unsupported(value.to_string())
    }

    /// Try to parse a combined "Ethnicity<sep>SES" value.
    ///
    /// Accepts space (`White UC`) or comma (`White,MC`) as separator.
    fn try_parse_combined(s: &str) -> Option<Self> {
        // Try comma first (more specific), then space.
        let split = s.split_once(',').or_else(|| s.split_once(' '));

        let (first, second) = split?;
        let first = first.trim();
        let second = second.trim();
        if first.is_empty() || second.is_empty() {
            return None;
        }

        let eth = Ethnicity::from_text(first);
        let ses = SesCode::from_text(second);
        if !matches!(eth, Ethnicity::Unsupported(_)) && !matches!(ses, SesCode::Unsupported(_)) {
            return Some(Self::Combined { eth, ses });
        }
        None
    }

    /// Returns the original text representation for serialization.
    pub fn as_str(&self) -> String {
        match self {
            Self::EthOnly(eth) => eth.as_str().to_string(),
            Self::SesOnly(ses) => ses.as_str().to_string(),
            Self::Combined { eth, ses } => format!("{},{}", eth.as_str(), ses.as_str()),
            Self::Unsupported(s) => s.clone(),
        }
    }
}

impl std::fmt::Display for SesValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.as_str())
    }
}

impl crate::model::WriteChat for SesValue {
    fn write_chat<W: std::fmt::Write>(&self, w: &mut W) -> std::fmt::Result {
        w.write_str(&self.as_str())
    }
}

impl Serialize for SesValue {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.as_str())
    }
}

impl<'de> Deserialize<'de> for SesValue {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let s = String::deserialize(deserializer)?;
        Ok(Self::from_text(&s))
    }
}

impl JsonSchema for SesValue {
    fn schema_name() -> std::borrow::Cow<'static, str> {
        "SesValue".into()
    }

    fn json_schema(_generator: &mut schemars::SchemaGenerator) -> schemars::Schema {
        schemars::json_schema!({ "type": "string" })
    }
}

impl From<String> for SesValue {
    fn from(value: String) -> Self {
        Self::from_text(&value)
    }
}

impl From<&str> for SesValue {
    fn from(value: &str) -> Self {
        Self::from_text(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_ses_code() {
        assert!(matches!(
            SesValue::from_text("UC"),
            SesValue::SesOnly(SesCode::UC)
        ));
        assert!(matches!(
            SesValue::from_text("MC"),
            SesValue::SesOnly(SesCode::MC)
        ));
        assert!(matches!(
            SesValue::from_text("WC"),
            SesValue::SesOnly(SesCode::WC)
        ));
        assert!(matches!(
            SesValue::from_text("LI"),
            SesValue::SesOnly(SesCode::LI)
        ));
    }

    #[test]
    fn parse_ethnicity() {
        assert!(matches!(
            SesValue::from_text("White"),
            SesValue::EthOnly(Ethnicity::White)
        ));
        assert!(matches!(
            SesValue::from_text("Black"),
            SesValue::EthOnly(Ethnicity::Black)
        ));
        assert!(matches!(
            SesValue::from_text("Asian"),
            SesValue::EthOnly(Ethnicity::Asian)
        ));
        assert!(matches!(
            SesValue::from_text("Latino"),
            SesValue::EthOnly(Ethnicity::Latino)
        ));
        assert!(matches!(
            SesValue::from_text("Pacific"),
            SesValue::EthOnly(Ethnicity::Pacific)
        ));
        assert!(matches!(
            SesValue::from_text("Native"),
            SesValue::EthOnly(Ethnicity::Native)
        ));
        assert!(matches!(
            SesValue::from_text("Multiple"),
            SesValue::EthOnly(Ethnicity::Multiple)
        ));
        assert!(matches!(
            SesValue::from_text("Unknown"),
            SesValue::EthOnly(Ethnicity::Unknown)
        ));
    }

    #[test]
    fn parse_combined_space() {
        match SesValue::from_text("White UC") {
            SesValue::Combined { eth, ses } => {
                assert_eq!(eth, Ethnicity::White);
                assert_eq!(ses, SesCode::UC);
            }
            other => panic!("expected Combined, got {:?}", other),
        }
    }

    #[test]
    fn parse_combined_comma() {
        // Real corpus data uses comma-separated format.
        for (input, expected_eth, expected_ses) in [
            ("White,MC", Ethnicity::White, SesCode::MC),
            ("Black,MC", Ethnicity::Black, SesCode::MC),
            ("White,UC", Ethnicity::White, SesCode::UC),
            ("Asian,MC", Ethnicity::Asian, SesCode::MC),
            ("White,WC", Ethnicity::White, SesCode::WC),
        ] {
            match SesValue::from_text(input) {
                SesValue::Combined { eth, ses } => {
                    assert_eq!(eth, expected_eth, "ethnicity mismatch for {input}");
                    assert_eq!(ses, expected_ses, "ses code mismatch for {input}");
                }
                other => panic!("expected Combined for {input}, got {other:?}"),
            }
        }
    }

    #[test]
    fn parse_unsupported() {
        assert!(matches!(
            SesValue::from_text("rich"),
            SesValue::Unsupported(_)
        ));
    }

    #[test]
    fn roundtrip() {
        // Exact roundtrip for canonical formats.
        for input in &["UC", "White", "White,MC", "something else"] {
            let parsed = SesValue::from_text(input);
            assert_eq!(parsed.as_str(), *input);
        }
    }

    #[test]
    fn space_normalized_to_comma() {
        // Space-separated combined values are accepted but normalized to comma on output.
        let parsed = SesValue::from_text("White UC");
        assert_eq!(parsed.as_str(), "White,UC");
    }
}
