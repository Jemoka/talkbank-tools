//! Integration tests for normative database parsing against real CLAN database files.
//!
//! These tests are skipped when the CLAN library directory is not present (CI-safe).

use std::path::Path;

use clan_core::database::{
    DatabaseFilter, Gender, compare_to_norms, discover_databases, parse_database,
};

mod common;

/// Resolve CLAN library path: CLAN_SOURCE_DIR env → meta-repo sibling → ~/OSX-CLAN/
fn clan_lib_path(subpath: &str) -> String {
    if let Ok(dir) = std::env::var("CLAN_SOURCE_DIR") {
        return format!("{dir}/{subpath}");
    }
    let sibling = common::meta_repo_root().join("OSX-CLAN").join(subpath);
    if sibling.exists() {
        return sibling.to_string_lossy().into_owned();
    }
    format!("{}/OSX-CLAN/{subpath}", env!("HOME"))
}

const KIDEVAL_SUBPATH: &str = "src/lib/kideval";
const EVAL_SUBPATH: &str = "src/lib/eval";

fn skip_if_missing(path: &str) -> bool {
    if !Path::new(path).exists() {
        eprintln!("Skipping: {path} not found");
        true
    } else {
        false
    }
}

#[test]
fn discover_kideval_databases() {
    if skip_if_missing(&clan_lib_path(KIDEVAL_SUBPATH)) {
        return;
    }
    let dbs = discover_databases(Path::new(&clan_lib_path(KIDEVAL_SUBPATH))).unwrap();
    // Should find eng_toyplay, eng_narrative, fra_toyplay, etc.
    assert!(
        dbs.len() >= 10,
        "Expected at least 10 databases, found {}",
        dbs.len()
    );

    // Check that eng_toyplay is present
    let eng_tp = dbs
        .iter()
        .find(|d| d.language == "eng" && d.corpus_type.as_deref() == Some("toyplay"));
    assert!(eng_tp.is_some(), "eng_toyplay_db.cut not found");
}

#[test]
fn parse_eng_toyplay_db() {
    let path = Path::new(&clan_lib_path(KIDEVAL_SUBPATH)).join("eng_toyplay_db.cut");
    if skip_if_missing(path.to_str().unwrap()) {
        return;
    }
    let db = parse_database(&path).unwrap();
    assert_eq!(db.header.version, 8);
    assert_eq!(db.header.utterance_limit, Some(50));
    // eng_toyplay has ~2,600 entries (10,505 lines / 4 lines per entry)
    assert!(
        db.entries.len() > 2000,
        "Expected >2000 entries, got {}",
        db.entries.len()
    );

    // Spot-check first entry
    let first = &db.entries[0];
    assert_eq!(first.metadata.language, "eng");
    assert!(first.metadata.age_months.is_some());
    assert!(!first.scores.is_empty());
}

#[test]
fn filter_by_age_and_gender() {
    let path = Path::new(&clan_lib_path(KIDEVAL_SUBPATH)).join("eng_toyplay_db.cut");
    if skip_if_missing(path.to_str().unwrap()) {
        return;
    }
    let db = parse_database(&path).unwrap();

    // Filter: English, male, age 24-36 months
    let filter = DatabaseFilter {
        language: Some("eng".to_owned()),
        gender: Some(Gender::Male),
        age_from_months: Some(24),
        age_to_months: Some(36),
        ..Default::default()
    };
    let matched = filter.apply(&db.entries);
    assert!(!matched.is_empty(), "Expected at least some matches");

    // Verify all matched entries actually match
    for entry in &matched {
        assert_eq!(entry.metadata.language, "eng");
        assert_eq!(entry.metadata.sex, Some(clan_core::database::Sex::Male));
        let age = entry.metadata.age_months.unwrap();
        assert!((24..=36).contains(&age), "Age {age} out of range 24..=36");
    }
}

#[test]
fn compare_against_norms() {
    let path = Path::new(&clan_lib_path(KIDEVAL_SUBPATH)).join("eng_toyplay_db.cut");
    if skip_if_missing(path.to_str().unwrap()) {
        return;
    }
    let db = parse_database(&path).unwrap();

    let filter = DatabaseFilter {
        language: Some("eng".to_owned()),
        age_from_months: Some(24),
        age_to_months: Some(30),
        ..Default::default()
    };
    let matched = filter.apply(&db.entries);
    assert!(!matched.is_empty());

    // Use the first matched entry's scores as the "speaker"
    let speaker_scores = matched[0].scores.clone();
    let result = compare_to_norms(&speaker_scores, &matched);

    assert_eq!(result.matched_entries, matched.len());
    assert!(!result.measures.is_empty());

    // Mean should be finite
    for m in &result.measures {
        assert!(m.db_mean.is_finite(), "Non-finite mean");
        assert!(m.db_sd.is_finite(), "Non-finite SD");
        if let Some(z) = m.z_score {
            assert!(z.is_finite(), "Non-finite z-score");
        }
    }
}

#[test]
fn discover_eval_databases() {
    if skip_if_missing(&clan_lib_path(EVAL_SUBPATH)) {
        return;
    }
    let dbs = discover_databases(Path::new(&clan_lib_path(EVAL_SUBPATH))).unwrap();
    assert!(!dbs.is_empty(), "Expected at least one eval database");
}

#[test]
fn parse_eval_db() {
    let path = Path::new(&clan_lib_path(EVAL_SUBPATH)).join("eng_eval_db.cut");
    if skip_if_missing(path.to_str().unwrap()) {
        return;
    }
    let db = parse_database(&path).unwrap();
    assert_eq!(db.header.version, 6);
    assert!(!db.entries.is_empty());

    // Eval entries should have scores (concatenated across gem groups)
    let first = &db.entries[0];
    assert!(!first.scores.is_empty(), "Expected scores in eval entry");
    assert!(
        !first.metadata.group.is_empty(),
        "Expected group (aphasia type) in eval entry"
    );
}
