//! Typed representation of a normative database entry.
//!
//! Each entry in a `.cut` database file represents one speaker from one transcript,
//! storing demographic metadata (from `@ID` headers) and a vector of numeric scores.

use serde::Serialize;

/// Version and metadata from the database file header line.
///
/// Format: `V<version> [<utt_limit>] <YYYY-MM-DD>, <HH:MM>`
#[derive(Debug, Clone, Serialize)]
pub struct DatabaseHeader {
    /// Database format version (e.g., 8 for KidEval V8).
    pub version: u32,
    /// Optional utterance count limit (e.g., 50).
    pub utterance_limit: Option<u32>,
}

/// Demographic metadata parsed from the `+` line of a database entry.
///
/// Fields are pipe-delimited and correspond to `@ID` header fields in CHAT:
/// `language|corpus|speaker_code|age|sex|group|ses|role|education|custom`
#[derive(Debug, Clone, Serialize)]
pub struct DbMetadata {
    /// Language code (e.g., "eng", "fra", "spa").
    pub language: String,
    /// Corpus name (e.g., "Bates", "ACWT").
    pub corpus: String,
    /// Speaker code (e.g., "CHI", "PAR").
    pub speaker_code: String,
    /// Age in months (parsed from "Y;MM." format). `None` if absent or unparseable.
    pub age_months: Option<u32>,
    /// Sex as parsed from the raw field.
    pub sex: Option<Sex>,
    /// Group label (e.g., "TD", "Broca").
    pub group: String,
    /// Socioeconomic status.
    pub ses: String,
    /// Participant role (e.g., "Target_Child", "Participant").
    pub role: String,
    /// Education field.
    pub education: String,
    /// Custom/unique field.
    pub custom: String,
}

/// Biological sex as recorded in database entries.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum Sex {
    /// Male participant.
    Male,
    /// Female participant.
    Female,
}

/// A single entry in a normative database.
///
/// Contains the source file path, demographic metadata, and a flat vector
/// of numeric scores whose column semantics depend on the database type
/// (KidEval, Eval, or Eval-D).
#[derive(Debug, Clone, Serialize)]
pub struct DatabaseEntry {
    /// Source transcript path (from the `=` line).
    pub file_path: String,
    /// Demographic metadata (from the `+` line).
    pub metadata: DbMetadata,
    /// Numeric scores (from the data line). Column order is database-type-specific.
    pub scores: Vec<f64>,
}

/// Parse an age string in CHAT "Y;MM." or "Y;MM.DD" format to total months.
///
/// Returns `None` for empty or unparseable age strings.
///
/// # Examples
///
/// - `"1;08."` → `Some(20)`
/// - `"2;04."` → `Some(28)`
/// - `"69;11."` → `Some(839)`
/// - `""` → `None`
pub(crate) fn parse_age_to_months(age_str: &str) -> Option<u32> {
    let trimmed = age_str.trim().trim_end_matches('.');
    if trimmed.is_empty() {
        return None;
    }

    // Handle age ranges like "2;04-2;08" — use the midpoint
    if let Some((from, to)) = trimmed.split_once('-') {
        let from_months = parse_single_age(from)?;
        let to_months = parse_single_age(to)?;
        return Some((from_months + to_months) / 2);
    }

    parse_single_age(trimmed)
}

/// Parse a single "Y;MM" age value to months.
fn parse_single_age(s: &str) -> Option<u32> {
    let trimmed = s.trim().trim_end_matches('.');
    let (years_str, months_str) = trimmed.split_once(';')?;
    let years: u32 = years_str.trim().parse().ok()?;
    let months_part = months_str.split('.').next().unwrap_or("");
    let months: u32 = if months_part.is_empty() {
        0
    } else {
        months_part.trim().parse().ok()?
    };
    Some(years * 12 + months)
}

/// Parse the sex field from a database metadata line.
pub(crate) fn parse_sex(s: &str) -> Option<Sex> {
    match s.trim().to_lowercase().as_str() {
        "male" => Some(Sex::Male),
        "female" => Some(Sex::Female),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn age_parsing() {
        assert_eq!(parse_age_to_months("1;08."), Some(20));
        assert_eq!(parse_age_to_months("2;04."), Some(28));
        assert_eq!(parse_age_to_months("69;11."), Some(839));
        assert_eq!(parse_age_to_months("3;00"), Some(36));
        assert_eq!(parse_age_to_months(""), None);
        assert_eq!(parse_age_to_months("1;06.01"), Some(18));
    }

    #[test]
    fn age_range_parsing() {
        // "2;04-2;08" → midpoint of 28..32 = 30
        assert_eq!(parse_age_to_months("2;04-2;08"), Some(30));
    }

    #[test]
    fn sex_parsing() {
        assert_eq!(parse_sex("male"), Some(Sex::Male));
        assert_eq!(parse_sex("female"), Some(Sex::Female));
        assert_eq!(parse_sex("Male"), Some(Sex::Male));
        assert_eq!(parse_sex(""), None);
        assert_eq!(parse_sex("unknown"), None);
    }
}
