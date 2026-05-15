//! End-to-end tests for `chatter debug find`: build a temp corpus on
//! disk, invoke the top-level `run_find_impl` entry point, and verify
//! the output against expected filters.

use std::fs;
use std::path::{Path, PathBuf};

use tempfile::TempDir;

use super::output::{FindOutputFormat, FindSortOrder};
use super::{FindArgs, run_find_impl};

/// Minimal bilingual CHAT file with a configurable @Languages header
/// and a configurable number of @s-marked words.
fn mini_chat(languages_header: &str, at_s_words: usize) -> String {
    let mut body = String::from("@UTF8\n@Begin\n");
    body.push_str(languages_header);
    body.push('\n');
    body.push_str("@Participants:\tMOT Mother Mother , CHI Target_Child Target_Child\n");
    body.push_str("@ID:\teng|corpus|MOT||female|||Mother|||\n");
    body.push_str("@ID:\teng|corpus|CHI||female|||Target_Child|||\n");

    // One utterance per @s word so counts are easy to reason about.
    for i in 0..at_s_words {
        body.push_str(&format!("*MOT:\tlook at the thing@s now word{} .\n", i));
    }
    if at_s_words == 0 {
        body.push_str("*MOT:\thello .\n");
    }
    body.push_str("@End\n");
    body
}

fn write_chat(dir: &Path, name: &str, content: &str) -> PathBuf {
    let path = dir.join(name);
    fs::write(&path, content).expect("write chat");
    path
}

fn base_args(paths: Vec<PathBuf>) -> FindArgs {
    FindArgs {
        paths,
        languages: None,
        language: vec![],
        min_languages: None,
        has_token: None,
        min_token_count: 1,
        max_per_pair: None,
        format: FindOutputFormat::Paths,
        sort: FindSortOrder::Path,
    }
}

fn run_capture(args: FindArgs) -> String {
    let mut buf = Vec::new();
    run_find_impl(args, &mut buf).expect("find ok");
    String::from_utf8(buf).expect("utf8")
}

#[test]
fn discovers_chat_files_in_directory_and_ignores_other_extensions() {
    let dir = TempDir::new().expect("tmpdir");
    write_chat(dir.path(), "a.cha", &mini_chat("@Languages:\teng", 0));
    fs::write(dir.path().join("notes.txt"), "ignore me").expect("write");

    let args = base_args(vec![dir.path().to_path_buf()]);
    let output = run_capture(args);
    let paths: Vec<&str> = output.lines().collect();
    assert_eq!(paths.len(), 1);
    assert!(paths[0].ends_with("a.cha"));
}

#[test]
fn min_languages_filter_selects_bilingual_files_only() {
    let dir = TempDir::new().expect("tmpdir");
    write_chat(dir.path(), "mono.cha", &mini_chat("@Languages:\teng", 0));
    write_chat(dir.path(), "bi.cha", &mini_chat("@Languages:\tspa, eng", 0));
    write_chat(
        dir.path(),
        "tri.cha",
        &mini_chat("@Languages:\tzho, eng, yue", 0),
    );

    let mut args = base_args(vec![dir.path().to_path_buf()]);
    args.min_languages = Some(2);

    let output = run_capture(args);
    assert!(output.contains("bi.cha"));
    assert!(output.contains("tri.cha"));
    assert!(!output.contains("mono.cha"));
}

#[test]
fn languages_filter_is_order_insensitive() {
    let dir = TempDir::new().expect("tmpdir");
    let se = write_chat(
        dir.path(),
        "spa_eng.cha",
        &mini_chat("@Languages:\tspa, eng", 0),
    );
    let es = write_chat(
        dir.path(),
        "eng_spa.cha",
        &mini_chat("@Languages:\teng, spa", 0),
    );
    let other = write_chat(
        dir.path(),
        "deu_eng.cha",
        &mini_chat("@Languages:\tdeu, eng", 0),
    );

    let mut args = base_args(vec![dir.path().to_path_buf()]);
    args.languages = Some(vec!["spa".into(), "eng".into()]);

    let output = run_capture(args);
    assert!(output.contains(se.file_name().unwrap().to_str().unwrap()));
    assert!(output.contains(es.file_name().unwrap().to_str().unwrap()));
    assert!(!output.contains(other.file_name().unwrap().to_str().unwrap()));
}

#[test]
fn has_token_filters_by_body_count() {
    let dir = TempDir::new().expect("tmpdir");
    write_chat(
        dir.path(),
        "rich.cha",
        &mini_chat("@Languages:\tspa, eng", 10),
    );
    write_chat(
        dir.path(),
        "sparse.cha",
        &mini_chat("@Languages:\tspa, eng", 2),
    );
    write_chat(
        dir.path(),
        "none.cha",
        &mini_chat("@Languages:\tspa, eng", 0),
    );

    let mut args = base_args(vec![dir.path().to_path_buf()]);
    args.has_token = Some("@s".into());
    args.min_token_count = 5;

    let output = run_capture(args);
    assert!(output.contains("rich.cha"));
    assert!(!output.contains("sparse.cha"));
    assert!(!output.contains("none.cha"));
}

#[test]
fn max_per_pair_caps_language_set_buckets() {
    let dir = TempDir::new().expect("tmpdir");
    for i in 0..5 {
        write_chat(
            dir.path(),
            &format!("se_{}.cha", i),
            &mini_chat("@Languages:\tspa, eng", 1),
        );
    }
    for i in 0..5 {
        write_chat(
            dir.path(),
            &format!("de_{}.cha", i),
            &mini_chat("@Languages:\tdeu, eng", 1),
        );
    }

    let mut args = base_args(vec![dir.path().to_path_buf()]);
    args.max_per_pair = Some(2);

    let output = run_capture(args);
    let se_count = output.matches("se_").count();
    let de_count = output.matches("de_").count();
    assert_eq!(se_count, 2);
    assert_eq!(de_count, 2);
}

#[test]
fn jsonl_output_carries_metadata() {
    let dir = TempDir::new().expect("tmpdir");
    write_chat(dir.path(), "bi.cha", &mini_chat("@Languages:\tspa, eng", 3));

    let mut args = base_args(vec![dir.path().to_path_buf()]);
    args.has_token = Some("@s".into());
    args.format = FindOutputFormat::Jsonl;

    let output = run_capture(args);
    let line = output.lines().next().expect("one line");
    let record: serde_json::Value = serde_json::from_str(line).expect("parse json");
    assert_eq!(record["languages"], "spa,eng");
    assert_eq!(record["at_s_count"], 3);
    assert!(record["path"].as_str().unwrap().ends_with("bi.cha"));
    assert!(record["utterance_count"].as_u64().unwrap() >= 3);
}

#[test]
fn token_count_desc_sort_ranks_densest_files_first() {
    let dir = TempDir::new().expect("tmpdir");
    // Same @Languages, different @s density — sort must order by density.
    write_chat(
        dir.path(),
        "dense.cha",
        &mini_chat("@Languages:\tspa, eng", 20),
    );
    write_chat(
        dir.path(),
        "thin.cha",
        &mini_chat("@Languages:\tspa, eng", 1),
    );
    write_chat(
        dir.path(),
        "mid.cha",
        &mini_chat("@Languages:\tspa, eng", 5),
    );

    let mut args = base_args(vec![dir.path().to_path_buf()]);
    args.has_token = Some("@s".into());
    args.sort = FindSortOrder::TokenCountDesc;

    let output = run_capture(args);
    let lines: Vec<&str> = output.lines().collect();
    assert_eq!(lines.len(), 3);
    assert!(lines[0].ends_with("dense.cha"), "got {:?}", lines);
    assert!(lines[1].ends_with("mid.cha"), "got {:?}", lines);
    assert!(lines[2].ends_with("thin.cha"), "got {:?}", lines);
}

#[test]
fn csv_output_has_header_and_rows() {
    let dir = TempDir::new().expect("tmpdir");
    write_chat(dir.path(), "bi.cha", &mini_chat("@Languages:\tspa, eng", 2));

    let mut args = base_args(vec![dir.path().to_path_buf()]);
    args.has_token = Some("@s".into());
    args.format = FindOutputFormat::Csv;

    let output = run_capture(args);
    let mut lines = output.lines();
    assert_eq!(
        lines.next(),
        Some("path,languages,at_s_count,utterance_count,file_bytes")
    );
    let row = lines.next().expect("row");
    assert!(row.contains("\"spa,eng\""));
    assert!(row.contains(",2,"));
}
