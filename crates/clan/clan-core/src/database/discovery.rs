//! Enumerate available normative databases from a library directory.
//!
//! Scans a directory (e.g., `lib/kideval/`) and reports which databases are
//! available, identified by their filename convention:
//! `<language>_<corpus_type>_db.cut` (e.g., `eng_toyplay_db.cut`).

use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::framework::TransformError;

/// Description of a single available database file.
#[derive(Debug, Clone, Serialize)]
pub struct AvailableDatabase {
    /// Full path to the `.cut` file.
    pub path: PathBuf,
    /// Language code extracted from filename (e.g., "eng", "fra").
    pub language: String,
    /// Corpus type extracted from filename (e.g., "toyplay", "narrative").
    /// `None` for databases without a corpus type suffix (e.g., legacy format).
    pub corpus_type: Option<String>,
    /// Human-readable display name.
    pub display_name: String,
}

/// Scan a library directory and enumerate all available `.cut` database files.
///
/// Returns an error only if the directory cannot be read. Missing directories
/// return an empty list.
pub fn discover_databases(lib_dir: &Path) -> Result<Vec<AvailableDatabase>, TransformError> {
    if !lib_dir.exists() {
        return Ok(Vec::new());
    }

    let entries = std::fs::read_dir(lib_dir).map_err(TransformError::Io)?;
    let mut databases = Vec::new();

    for entry in entries {
        let entry = entry.map_err(TransformError::Io)?;
        let path = entry.path();

        if !path.is_file() {
            continue;
        }

        let file_name = match path.file_name().and_then(|n| n.to_str()) {
            Some(n) => n,
            None => continue,
        };

        // Only process _db.cut files (skip 0all_norms_with_columns.csv, etc.)
        if !file_name.ends_with("_db.cut") {
            continue;
        }

        if let Some(db) = parse_database_filename(file_name, &path) {
            databases.push(db);
        }
    }

    // Sort by language then corpus type for stable ordering
    databases.sort_by(|a, b| {
        a.language
            .cmp(&b.language)
            .then_with(|| a.corpus_type.cmp(&b.corpus_type))
    });

    Ok(databases)
}

/// Parse a database filename into its components.
///
/// Expected patterns:
/// - `eng_toyplay_db.cut` → language="eng", corpus_type="toyplay"
/// - `fra_narrative_db.cut` → language="fra", corpus_type="narrative"
/// - `eng_eval_db.cut` → language="eng", corpus_type="eval"
/// - `eng_eval-d_db.cut` → language="eng", corpus_type="eval-d"
/// - `eng_fp_kideval_db.cut` → language="eng", corpus_type="fp_kideval"
/// - `jpn_td_kideval_db.cut` → language="jpn", corpus_type="td_kideval"
fn parse_database_filename(name: &str, path: &Path) -> Option<AvailableDatabase> {
    let stem = name.strip_suffix("_db.cut")?;
    let (language, corpus_type) = if let Some(idx) = stem.find('_') {
        let lang = &stem[..idx];
        let ct = &stem[idx + 1..];
        (lang.to_owned(), Some(ct.to_owned()))
    } else {
        (stem.to_owned(), None)
    };

    let display_name = match &corpus_type {
        Some(ct) => format!("{} ({})", language.to_uppercase(), ct),
        None => language.to_uppercase(),
    };

    Some(AvailableDatabase {
        path: path.to_owned(),
        language,
        corpus_type,
        display_name,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_standard_filename() {
        let db = parse_database_filename(
            "eng_toyplay_db.cut",
            Path::new("/lib/kideval/eng_toyplay_db.cut"),
        )
        .unwrap();
        assert_eq!(db.language, "eng");
        assert_eq!(db.corpus_type.as_deref(), Some("toyplay"));
    }

    #[test]
    fn parse_compound_corpus_type() {
        let db = parse_database_filename(
            "eng_fp_kideval_db.cut",
            Path::new("/lib/kideval/eng_fp_kideval_db.cut"),
        )
        .unwrap();
        assert_eq!(db.language, "eng");
        assert_eq!(db.corpus_type.as_deref(), Some("fp_kideval"));
    }

    #[test]
    fn parse_eval_d_filename() {
        let db = parse_database_filename(
            "eng_eval-d_db.cut",
            Path::new("/lib/eval/eng_eval-d_db.cut"),
        )
        .unwrap();
        assert_eq!(db.language, "eng");
        assert_eq!(db.corpus_type.as_deref(), Some("eval-d"));
    }

    #[test]
    fn skip_non_db_file() {
        assert!(parse_database_filename("0all_norms_with_columns.csv", Path::new("x")).is_none());
        assert!(parse_database_filename("nld.cut", Path::new("x")).is_none());
    }
}
