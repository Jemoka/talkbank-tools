//! Fixture extraction tool.
//!
//! Run with: cargo test -p talkbank-parser-re2c --test extract_fixtures -- --ignored --nocapture
//!
//! Extracts sample CHAT lines from ~/talkbank/data/*-data and writes them
//! to tests/fixtures/ as permanent test fixtures. These fixtures are checked
//! into git so anyone cloning the repo can run parser tests without access
//! to the full corpus.
//!
//! Each fixture file contains one logical CHAT line per entry, separated by
//! a blank line. Lines include continuations (tab-indented follow-on lines).

use std::collections::BTreeMap;
use std::io::Write;
use talkbank_parser_re2c::chat_lines::{ChatLineKind, ChatLines};

/// Set TALKBANK_DATA to the path containing *-data corpus directories.
/// Example: TALKBANK_DATA=~/talkbank/data cargo test --test extract_fixtures -- --ignored
fn corpus_base() -> String {
    std::env::var("TALKBANK_DATA").unwrap_or_else(|_| {
        // Fallback: try ~/talkbank/data
        let home = std::env::var("HOME").unwrap_or_default();
        format!("{home}/talkbank/data")
    })
}
const FIXTURES_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures");
const SAMPLES_PER_TYPE: usize = 200;

/// Extract fixtures from corpus. Only runs when explicitly requested via --ignored.
#[test]
#[ignore]
fn extract_all_fixtures() {
    let mut by_prefix: BTreeMap<String, Vec<String>> = BTreeMap::new();

    // Define the prefixes we want to extract
    let prefixes = [
        // Main tier
        ("main_tier", "*"),
        // Headers - structured
        ("header_id", "@ID:\t"),
        ("header_types", "@Types:\t"),
        ("header_languages", "@Languages:\t"),
        ("header_participants", "@Participants:\t"),
        ("header_media", "@Media:\t"),
        ("header_date", "@Date:\t"),
        ("header_options", "@Options:\t"),
        ("header_comment", "@Comment:\t"),
        // Headers - no content
        ("header_utf8", "@UTF8"),
        ("header_begin", "@Begin"),
        ("header_end", "@End"),
        // Headers - text content
        ("header_location", "@Location:\t"),
        ("header_situation", "@Situation:\t"),
        ("header_activities", "@Activities:\t"),
        ("header_recording_quality", "@Recording Quality:\t"),
        ("header_pid", "@PID:\t"),
        // Headers - speaker embedded
        ("header_birth_of", "@Birth of"),
        // Headers - optional content
        ("header_bg", "@Bg"),
        ("header_eg", "@Eg"),
        // Dependent tiers - structured
        ("tier_mor", "%mor:\t"),
        ("tier_gra", "%gra:\t"),
        ("tier_pho", "%pho:\t"),
        // Dependent tiers - text
        ("tier_com", "%com:\t"),
        ("tier_act", "%act:\t"),
        ("tier_eng", "%eng:\t"),
        ("tier_flo", "%flo:\t"),
        ("tier_spa", "%spa:\t"),
        // Dependent tiers - more text
        ("tier_eng", "%eng:\t"),
        ("tier_flo", "%flo:\t"),
        ("tier_ort", "%ort:\t"),
        ("tier_wor", "%wor:\t"),
        ("tier_err", "%err:\t"),
        ("tier_add", "%add:\t"),
        ("tier_gpx", "%gpx:\t"),
        ("tier_sit", "%sit:\t"),
        ("tier_int", "%int:\t"),
        // User-defined tiers
        ("tier_xdb", "%xdb:\t"),
        ("tier_xpho", "%xpho:\t"),
        ("tier_xcod", "%xcod:\t"),
    ];

    for (name, _) in &prefixes {
        by_prefix.insert(name.to_string(), Vec::new());
    }

    let base = corpus_base();
    let data_dirs: Vec<_> = std::fs::read_dir(&base)
        .into_iter()
        .flatten()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_name().to_str().is_some_and(|n| n.ends_with("-data")))
        // Sample from ALL corpus dirs for broad coverage:
        // aphasia, asd, biling, ca-candor, ca, childes-eng-na/uk/other/romance-germanic,
        // class, dementia, fluency, homebank-*, motor, phon-*, psychosis, rhd,
        // samtale, slabank, tbi
        .collect();

    for dir in &data_dirs {
        let walker = walkdir::WalkDir::new(dir.path())
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.path().extension().is_some_and(|ext| ext == "cha"))
            .take(30); // 30 files per dir × 24 dirs = ~720 files

        for entry in walker {
            if let Ok(content) = std::fs::read_to_string(entry.path()) {
                for chat_line in ChatLines::new(&content) {
                    for (name, prefix) in &prefixes {
                        let samples = by_prefix.get_mut(*name).unwrap();
                        if samples.len() < SAMPLES_PER_TYPE && chat_line.text.starts_with(prefix) {
                            // Deduplicate
                            if !samples.contains(&chat_line.text.to_string()) {
                                samples.push(chat_line.text.to_string());
                            }
                        }
                    }
                }
            }
        }
    }

    // Write fixture files
    std::fs::create_dir_all(FIXTURES_DIR).unwrap();

    for (name, samples) in &by_prefix {
        let path = format!("{FIXTURES_DIR}/{name}.txt");
        let mut f = std::fs::File::create(&path).unwrap();
        writeln!(
            f,
            "# {name} — {} samples extracted from ~/talkbank/data/*-data",
            samples.len()
        )
        .unwrap();
        writeln!(f, "# Each entry is separated by a blank line.").unwrap();
        writeln!(
            f,
            "# Lines may include tab-continuations (multi-physical-line entries)."
        )
        .unwrap();
        writeln!(f).unwrap();
        for sample in samples {
            // Write the sample, then a blank separator line
            write!(f, "{sample}").unwrap();
            // Ensure there's a newline after each sample
            if !sample.ends_with('\n') {
                writeln!(f).unwrap();
            }
            // Blank line separator between samples
            writeln!(f).unwrap();
        }
        eprintln!("  {name}: {} samples → {path}", samples.len());
    }

    eprintln!("Fixtures written to {FIXTURES_DIR}/");
}
