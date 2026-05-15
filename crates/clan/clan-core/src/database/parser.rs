//! Principled parser for the `.cut` normative database format.
//!
//! Database files contain a version header followed by repeating entry blocks:
//!
//! ```text
//! V8 50 2025-07-01, 04:01            ← header: version, utt_limit, timestamp
//! =Eng-NA/Bates/Free20/keith.cha     ← entry: source file path
//! +eng|Bates|CHI|1;08.|male|TD|MC|Target_Child|||  ← entry: @ID metadata
//! 81 84 55 53 73 76 50 94 ...        ← entry: space-separated scores
//! -                                   ← entry: end marker
//! ```
//!
//! The Eval database has an additional layer: entries may contain multiple
//! gem-grouped score lines prefixed with `G<name>` (e.g., `GSpeech`, `GCat`).
//! This parser handles both formats.

use std::path::Path;

use crate::database::entry::{
    DatabaseEntry, DatabaseHeader, DbMetadata, parse_age_to_months, parse_sex,
};
use crate::framework::TransformError;

/// A fully parsed normative database.
#[derive(Debug, Clone)]
pub struct ParsedDatabase {
    /// File header (version, utterance limit).
    pub header: DatabaseHeader,
    /// All entries in the database.
    pub entries: Vec<DatabaseEntry>,
}

/// Parse a `.cut` database file from disk.
pub fn parse_database(path: &Path) -> Result<ParsedDatabase, TransformError> {
    let content = std::fs::read_to_string(path).map_err(TransformError::Io)?;
    parse_database_str(&content, path)
}

/// Parse a `.cut` database from a string (for testing).
pub(crate) fn parse_database_str(
    content: &str,
    source: &Path,
) -> Result<ParsedDatabase, TransformError> {
    let mut lines = content.lines().peekable();

    // Parse header
    let header_line = lines.next().ok_or_else(|| {
        TransformError::Transform(format!("Empty database file: {}", source.display()))
    })?;
    let header = parse_header(header_line, source)?;

    // Parse entries
    let mut entries = Vec::new();
    while let Some(line) = lines.peek() {
        if line.starts_with('=') {
            match parse_entry(&mut lines, source) {
                Ok(entry) => entries.push(entry),
                Err(e) => {
                    tracing::warn!("Skipping malformed entry in {}: {e}", source.display());
                    // Skip to next entry boundary
                    skip_to_next_entry(&mut lines);
                }
            }
        } else {
            lines.next(); // skip blank/unknown lines
        }
    }

    Ok(ParsedDatabase { header, entries })
}

/// Parse the version header line.
fn parse_header(line: &str, source: &Path) -> Result<DatabaseHeader, TransformError> {
    let trimmed = line.trim();
    if !trimmed.starts_with('V') {
        return Err(TransformError::Transform(format!(
            "Expected version header starting with 'V', got: {trimmed:?} in {}",
            source.display()
        )));
    }

    // Format: "V8 50 2025-07-01, 04:01" or "V5 2022-01-28, 12:49"
    let after_v = &trimmed[1..];
    let mut tokens = after_v.split_whitespace();

    let version: u32 = tokens.next().and_then(|s| s.parse().ok()).ok_or_else(|| {
        TransformError::Transform(format!(
            "Invalid version number in header: {trimmed:?} in {}",
            source.display()
        ))
    })?;

    // Next token might be an utterance limit (a plain integer) or a date
    let utterance_limit = tokens.next().and_then(|s| s.parse::<u32>().ok());

    Ok(DatabaseHeader {
        version,
        utterance_limit,
    })
}

/// Parse a single database entry (4 lines: =path, +metadata, scores, -).
fn parse_entry<'a>(
    lines: &mut std::iter::Peekable<impl Iterator<Item = &'a str>>,
    source: &Path,
) -> Result<DatabaseEntry, TransformError> {
    // Line 1: =filepath
    let path_line = lines.next().ok_or_else(|| {
        TransformError::Transform(format!(
            "Unexpected EOF reading entry path in {}",
            source.display()
        ))
    })?;
    let file_path = path_line
        .strip_prefix('=')
        .ok_or_else(|| {
            TransformError::Transform(format!(
                "Expected '=' prefix on path line, got: {path_line:?} in {}",
                source.display()
            ))
        })?
        .to_owned();

    // Line 2: +metadata
    let meta_line = lines.next().ok_or_else(|| {
        TransformError::Transform(format!(
            "Unexpected EOF reading metadata for entry {file_path:?} in {}",
            source.display()
        ))
    })?;
    let metadata = parse_metadata(meta_line, &file_path, source)?;

    // Lines 3+: scores (possibly multiple gem-grouped lines for Eval format)
    let mut all_scores = Vec::new();
    while let Some(&line) = lines.peek() {
        let trimmed = line.trim();
        if trimmed == "-" {
            lines.next(); // consume the terminator
            break;
        }
        if trimmed.starts_with('=') {
            // Next entry started without a '-' terminator — tolerate
            break;
        }
        lines.next();

        // Skip gem group headers like "GSpeech", "GCat"
        if trimmed.starts_with('G') && !trimmed.contains(' ') {
            continue;
        }

        // Parse score line
        for token in trimmed.split_whitespace() {
            if let Ok(v) = token.parse::<f64>() {
                all_scores.push(v);
            }
            // Skip unparseable tokens (shouldn't happen in valid files)
        }
    }

    Ok(DatabaseEntry {
        file_path,
        metadata,
        scores: all_scores,
    })
}

/// Parse the `+` metadata line into a [`DbMetadata`].
fn parse_metadata(
    line: &str,
    entry_path: &str,
    source: &Path,
) -> Result<DbMetadata, TransformError> {
    let content = line.strip_prefix('+').ok_or_else(|| {
        TransformError::Transform(format!(
            "Expected '+' prefix on metadata line for {entry_path:?} in {}",
            source.display()
        ))
    })?;

    let fields: Vec<&str> = content.split('|').collect();
    if fields.len() < 8 {
        return Err(TransformError::Transform(format!(
            "Metadata line has {} fields (need at least 8) for {entry_path:?} in {}",
            fields.len(),
            source.display()
        )));
    }

    let age_str = fields.get(3).copied().unwrap_or("");
    let sex_str = fields.get(4).copied().unwrap_or("");

    Ok(DbMetadata {
        language: fields[0].to_owned(),
        corpus: fields[1].to_owned(),
        speaker_code: fields[2].to_owned(),
        age_months: parse_age_to_months(age_str),
        sex: parse_sex(sex_str),
        group: fields.get(5).copied().unwrap_or("").to_owned(),
        ses: fields.get(6).copied().unwrap_or("").to_owned(),
        role: fields.get(7).copied().unwrap_or("").to_owned(),
        education: fields.get(8).copied().unwrap_or("").to_owned(),
        custom: fields.get(9).copied().unwrap_or("").to_owned(),
    })
}

/// Skip lines until we reach the next entry boundary or EOF.
fn skip_to_next_entry<'a>(lines: &mut std::iter::Peekable<impl Iterator<Item = &'a str>>) {
    for line in lines.by_ref() {
        if line.trim() == "-" {
            break;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    const SAMPLE_DB: &str = "\
V8 50 2025-07-01, 04:01
=Eng-NA/Bates/Free20/keith.cha
+eng|Bates|CHI|1;08.|male|TD|MC|Target_Child|||
81 84 55 53 73 76 50 94 38 38 94 20.231045 15 89 57 0 0 0 1 0 9 39 33 18 94 0 3 0 0 0 0 1 1 0 3 0 0 4 0
-
=Eng-NA/Bates/Free20/olivia.cha
+eng|Bates|CHI|1;08.|female|TD|MC|Target_Child|||
73 73 72 66 56 56 50 84 37 37 84 17.433138 4 84 87 0 0 0 0 0 2 5 47 8 84 0 1 0 0 0 0 0 2 0 0 0 0 0 0
-
";

    #[test]
    fn parse_kideval_database() {
        let source = PathBuf::from("test.cut");
        let db = parse_database_str(SAMPLE_DB, &source).unwrap();
        assert_eq!(db.header.version, 8);
        assert_eq!(db.header.utterance_limit, Some(50));
        assert_eq!(db.entries.len(), 2);

        let e0 = &db.entries[0];
        assert_eq!(e0.file_path, "Eng-NA/Bates/Free20/keith.cha");
        assert_eq!(e0.metadata.language, "eng");
        assert_eq!(e0.metadata.age_months, Some(20));
        assert_eq!(e0.metadata.sex, Some(super::super::entry::Sex::Male));
        assert_eq!(e0.scores.len(), 39);
        assert!((e0.scores[0] - 81.0).abs() < f64::EPSILON);
        assert!((e0.scores[11] - 20.231045).abs() < 1e-6);
    }

    const EVAL_SAMPLE: &str = "\
V6
=English/Protocol/ACWT/PWA/ACWT01a.cha
+eng|ACWT|PAR|69;11.|female|Broca||Participant||63.9|
GSpeech
4 9 7 0 0 0 7 7 2 2 0 1 7 0 0 0 0 0 0 0 0 0 1 0 0 0 0 0 0 0 0 3 0 0 3 5 2 1 4
GStroke
34 41 27 3 3 3 24 27 8 8 5 2 27 7 3 3 0 2 1 0 0 0 4 0 1 0 0 0 1 0 6 15 1 2 3 14 13 20 8 11 15 7 12 16 18 17 10 9 19 1 6
-
";

    #[test]
    fn parse_eval_database() {
        let source = PathBuf::from("eval.cut");
        let db = parse_database_str(EVAL_SAMPLE, &source).unwrap();
        assert_eq!(db.header.version, 6);
        assert_eq!(db.header.utterance_limit, None);
        assert_eq!(db.entries.len(), 1);

        let e = &db.entries[0];
        assert_eq!(e.metadata.language, "eng");
        assert_eq!(e.metadata.age_months, Some(839));
        assert_eq!(e.metadata.sex, Some(super::super::entry::Sex::Female));
        assert_eq!(e.metadata.group, "Broca");
        // Scores from both gem groups are concatenated
        assert_eq!(e.scores.len(), 39 + 51);
    }

    const V5_SAMPLE: &str = "\
V5 2022-01-28, 12:49
=Eng-NA/Bates/Free20/keith.cha
+eng|Bates|CHI|1;08.|male|TD|MC|Target_Child|||
81 90 55 53 73 82 50 94 38 38 94 20.053291 15 89 57 0 0 0 1 0 9 39 33 16 94 0 3 0 0 0 0 1 1 0 3 0 0 4 0
-
";

    #[test]
    fn parse_v5_header() {
        let source = PathBuf::from("v5.cut");
        let db = parse_database_str(V5_SAMPLE, &source).unwrap();
        assert_eq!(db.header.version, 5);
        // "2022-01-28," is not a valid u32, so utterance_limit is None
        assert_eq!(db.header.utterance_limit, None);
        assert_eq!(db.entries.len(), 1);
    }
}
