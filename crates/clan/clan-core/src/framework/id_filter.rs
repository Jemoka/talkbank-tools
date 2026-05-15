//! `--id-filter PATTERN` — match clan filtering against `@ID` headers.
//!
//! Replaces legacy CLAN `+t@ID="…"` syntax. The rewriter in
//! `crates/clan-core/src/clan_args.rs` translates `+t@ID=PATTERN`
//! into `--id-filter PATTERN`; this module is the clap-consumable side.
//!
//! ## Pattern grammar
//!
//! Pipe-separated, ordered to match the `@ID` header column order:
//! `lang|corpus|speaker|age|sex|group|ses|role|education|custom`.
//!
//! Each field is either:
//!
//! - `*` — wildcard, matches any value (including absent).
//! - empty — equivalent to `*` (forgiving). An empty field never
//!   requires the corresponding `@ID` slot to be absent.
//! - any other token — literal exact match (case-sensitive, byte-for-byte
//!   on the rendered field text). Speaker/role tokens compare by their
//!   typed `Display` impl, which is the same canonical text used in the
//!   `@ID` line itself.
//!
//! Trailing fields may be omitted entirely; missing fields are treated
//! as wildcards. So `eng|*|CHI|*`, `eng|*|CHI|`, and `eng|*|CHI` all
//! match the same set of `@ID` lines. This rule survives future
//! extensions to the CHAT spec that add more `@ID` columns.
//!
//! ## Multi-language IDs
//!
//! The `language` field of an `@ID` is a comma-separated list. The
//! filter's language constraint matches if **any** language in the
//! `@ID` matches the pattern (set membership), not the exact rendered
//! string. This mirrors CLAN's behaviour for multilingual corpora.

use std::fmt;
use std::str::FromStr;

use talkbank_model::IDHeader;
use thiserror::Error;

/// One position in an `@ID` filter pattern.
///
/// `Glob` is the wildcard, produced by `*` or an empty/omitted field.
/// `Literal` carries the exact token to match against the rendered
/// `@ID` field text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IdPatternField {
    /// Matches any value (including absent).
    Glob,
    /// Matches the exact token byte-for-byte against the rendered field.
    Literal(String),
}

impl IdPatternField {
    /// Parse one field from the pipe-split source. Empty → Glob; `*` → Glob.
    fn parse(raw: &str) -> Self {
        if raw.is_empty() || raw == "*" {
            Self::Glob
        } else {
            Self::Literal(raw.to_owned())
        }
    }
}

/// Compiled `@ID` filter pattern. Each cell carries its column up
/// front, so matching does not have to recover the column from an
/// ordinal. Trailing wildcards are normalized away so two patterns
/// that only differ in trailing `*`s compare equal.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct IdPattern {
    cells: Vec<(IdColumn, IdPatternField)>,
}

/// One column in the `@ID` header, in canonical CHAT order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum IdColumn {
    Language,
    Corpus,
    Speaker,
    Age,
    Sex,
    Group,
    Ses,
    Role,
    Education,
    Custom,
}

impl IdColumn {
    /// Every column in `@ID` order. The pattern parser zips against this.
    const ALL: [IdColumn; 10] = [
        Self::Language,
        Self::Corpus,
        Self::Speaker,
        Self::Age,
        Self::Sex,
        Self::Group,
        Self::Ses,
        Self::Role,
        Self::Education,
        Self::Custom,
    ];
}

/// Errors produced while parsing a `--id-filter` PATTERN.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ParseIdFilterError {
    /// Pattern had more pipe-separated fields than the `@ID` header has
    /// columns. The grammar is positional; trailing columns beyond what
    /// CHAT defines are an error rather than silently ignored, to keep
    /// typos from matching everything.
    #[error("--id-filter pattern has {found} fields; @ID header has only {expected}")]
    TooManyFields {
        /// Number of fields parsed.
        found: usize,
        /// Maximum allowed (length of `IdColumn::ALL`).
        expected: usize,
    },
}

impl FromStr for IdPattern {
    type Err = ParseIdFilterError;

    fn from_str(input: &str) -> Result<Self, Self::Err> {
        let raw_fields: Vec<&str> = input.split('|').collect();
        if raw_fields.len() > IdColumn::ALL.len() {
            return Err(ParseIdFilterError::TooManyFields {
                found: raw_fields.len(),
                expected: IdColumn::ALL.len(),
            });
        }
        let mut cells: Vec<(IdColumn, IdPatternField)> = raw_fields
            .iter()
            .zip(IdColumn::ALL.iter().copied())
            .map(|(raw, column)| (column, IdPatternField::parse(raw)))
            .collect();
        // Forgiving tail: strip trailing globs so canonical Eq is preserved.
        while matches!(cells.last(), Some((_, IdPatternField::Glob))) {
            cells.pop();
        }
        Ok(Self { cells })
    }
}

impl fmt::Display for IdPattern {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (i, (_, field)) in self.cells.iter().enumerate() {
            if i > 0 {
                write!(f, "|")?;
            }
            match field {
                IdPatternField::Glob => write!(f, "*")?,
                IdPatternField::Literal(s) => write!(f, "{s}")?,
            }
        }
        Ok(())
    }
}

/// Top-level filter newtype carried in `FilterConfig`.
///
/// Wraps an `IdPattern` so callers don't pass raw patterns around and so
/// future extensions (e.g. multiple OR'd patterns) have a stable type.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct IdFilter {
    pattern: IdPattern,
}

impl IdFilter {
    /// Borrow the compiled pattern.
    pub fn pattern(&self) -> &IdPattern {
        &self.pattern
    }

    /// Whether the given `@ID` header satisfies this filter.
    pub fn matches(&self, header: &IDHeader) -> bool {
        self.pattern
            .cells
            .iter()
            .all(|(column, field)| column_matches(*column, header, field))
    }
}

impl FromStr for IdFilter {
    type Err = ParseIdFilterError;

    fn from_str(input: &str) -> Result<Self, Self::Err> {
        Ok(Self {
            pattern: input.parse::<IdPattern>()?,
        })
    }
}

impl fmt::Display for IdFilter {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.pattern.fmt(f)
    }
}

/// Match one `IdColumn` of an `IDHeader` against an `IdPatternField`.
///
/// Field-by-field rendering uses each typed slot's `to_string()`; for
/// the language column, the pattern matches if **any** language in the
/// `@ID`'s `LanguageCodes` matches (set membership, mirroring CLAN).
fn column_matches(column: IdColumn, header: &IDHeader, field: &IdPatternField) -> bool {
    // Glob matches everything by definition; skip the column-specific
    // rendering (which allocates) entirely.
    let want = match field {
        IdPatternField::Glob => return true,
        IdPatternField::Literal(want) => want,
    };

    // Language is set-valued (comma-separated `LanguageCodes`); match if
    // any language in the @ID matches the pattern.
    if matches!(column, IdColumn::Language) {
        return header.language.0.iter().any(|lc| lc.to_string() == *want);
    }

    // For every other column, render via Display and compare. `Display`
    // is the canonical CHAT-line rendering for every typed field used
    // in `@ID`, so the literal in the pattern compares against the same
    // text a user would see in the source file.
    let candidate: Option<String> = match column {
        IdColumn::Language => unreachable!("handled above"),
        IdColumn::Corpus => header.corpus.as_ref().map(|v| v.to_string()),
        IdColumn::Speaker => Some(header.speaker.to_string()),
        IdColumn::Age => header.age.as_ref().map(|v| v.to_string()),
        IdColumn::Sex => header.sex.as_ref().map(|v| v.as_str().to_owned()),
        IdColumn::Group => header.group.as_ref().map(|v| v.to_string()),
        IdColumn::Ses => header.ses.as_ref().map(|v| v.to_string()),
        IdColumn::Role => Some(header.role.to_string()),
        IdColumn::Education => header.education.as_ref().map(|v| v.to_string()),
        IdColumn::Custom => header.custom_field.as_ref().map(|v| v.to_string()),
    };
    candidate.as_deref() == Some(want.as_str())
}

/// Parse a clap-friendly `--id-filter` argument.
pub fn parse_id_filter(input: &str) -> Result<IdFilter, String> {
    input.parse::<IdFilter>().map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use talkbank_model::{LanguageCode, LanguageCodes};

    fn header_chi() -> IDHeader {
        IDHeader::new("eng", "CHI", "Target_Child")
    }

    fn header_mot() -> IDHeader {
        IDHeader::new("eng", "MOT", "Mother")
    }

    fn header_multilang() -> IDHeader {
        let mut h = IDHeader::new("eng", "CHI", "Target_Child");
        h.language = LanguageCodes::new(vec![LanguageCode::from("eng"), LanguageCode::from("yue")]);
        h
    }

    #[test]
    fn empty_pattern_matches_everything() {
        let filter: IdFilter = "".parse().unwrap();
        assert!(filter.matches(&header_chi()));
        assert!(filter.matches(&header_mot()));
    }

    #[test]
    fn glob_each_field_matches_everything() {
        let filter: IdFilter = "*|*|*|*|*|*|*|*|*|*".parse().unwrap();
        assert!(filter.matches(&header_chi()));
        assert!(filter.matches(&header_mot()));
    }

    #[test]
    fn speaker_literal_filters_by_speaker() {
        let filter: IdFilter = "*|*|CHI|*".parse().unwrap();
        assert!(filter.matches(&header_chi()));
        assert!(!filter.matches(&header_mot()));
    }

    #[test]
    fn language_literal_filters_by_language() {
        let filter: IdFilter = "eng|*|*|*".parse().unwrap();
        assert!(filter.matches(&header_chi()));
        assert!(!filter.matches(&IDHeader::new("fra", "CHI", "Target_Child")));
    }

    #[test]
    fn multilingual_id_matches_if_any_language_matches() {
        let filter_eng: IdFilter = "eng|*|*|*".parse().unwrap();
        let filter_yue: IdFilter = "yue|*|*|*".parse().unwrap();
        let filter_fra: IdFilter = "fra|*|*|*".parse().unwrap();
        assert!(filter_eng.matches(&header_multilang()));
        assert!(filter_yue.matches(&header_multilang()));
        assert!(!filter_fra.matches(&header_multilang()));
    }

    #[test]
    fn trailing_empty_is_forgiving() {
        // `eng|*|CHI|` should match the same set as `eng|*|CHI|*`.
        let with_pipe: IdFilter = "eng|*|CHI|".parse().unwrap();
        let with_star: IdFilter = "eng|*|CHI|*".parse().unwrap();
        let bare: IdFilter = "eng|*|CHI".parse().unwrap();
        assert_eq!(with_pipe, with_star);
        assert_eq!(with_pipe, bare);
        assert!(with_pipe.matches(&header_chi()));
        assert!(!with_pipe.matches(&header_mot()));
    }

    #[test]
    fn role_literal_filters_by_role() {
        let filter: IdFilter = "*|*|*|*|*|*|*|Target_Child|*|*".parse().unwrap();
        assert!(filter.matches(&header_chi()));
        assert!(!filter.matches(&header_mot()));
    }

    #[test]
    fn over_length_pattern_is_an_error() {
        let result: Result<IdFilter, _> = "a|b|c|d|e|f|g|h|i|j|k".parse();
        assert!(matches!(
            result,
            Err(ParseIdFilterError::TooManyFields {
                found: 11,
                expected: 10
            })
        ));
    }

    #[test]
    fn display_roundtrips_through_parse() {
        for raw in &["", "eng|*|CHI", "yue|*|CHI", "*|abc|*|*|*|*|*|Mother"] {
            let parsed: IdFilter = raw.parse().unwrap();
            let printed = parsed.to_string();
            let reparsed: IdFilter = printed.parse().unwrap();
            assert_eq!(parsed, reparsed, "roundtrip failed for {raw:?}");
        }
    }

    #[test]
    fn corpus_literal_filters_present_corpus() {
        // Build a header with a corpus field, then a header without.
        let mut with_corpus = header_chi();
        with_corpus.corpus = Some(talkbank_model::CorpusName::from("MyCorpus"));
        let without_corpus = header_chi();

        let filter: IdFilter = "*|MyCorpus|*|*".parse().unwrap();
        assert!(filter.matches(&with_corpus));
        assert!(!filter.matches(&without_corpus));
    }
}
