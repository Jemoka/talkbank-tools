//! Integration tests for the desktop app's validation pipeline and event bridge.
//!
//! These tests exercise the same code paths as the Tauri commands but without
//! the Tauri runtime — they call `validate_target_streaming()` directly and
//! verify the `FrontendEvent` serialization matches what the React frontend
//! expects.
//!
//! Run with: `cargo nextest run -p chatter-gui --test validation_bridge`

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use chatter_gui_lib::events::FrontendEvent;
use chatter_gui_lib::protocol;
use chatter_gui_lib::protocol::commands::{
    ExportFormat, ExportResultsRequest, OpenInClanRequest, ValidateRequest,
};
use chatter_gui_lib::validation::validate_target_streaming;

/// Find the workspace root by walking up from the manifest dir.
fn workspace_root() -> PathBuf {
    let mut dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    // desktop/src-tauri → desktop → talkbank-tools
    dir.pop();
    dir.pop();
    dir
}

/// Reference corpus path (97 files, must all be valid).
fn reference_corpus() -> PathBuf {
    workspace_root().join("resources/corpus/reference")
}

/// Collect all frontend events from a validation run.
fn collect_events(target: &Path) -> Vec<FrontendEvent> {
    let (rx, _cancel_tx) =
        validate_target_streaming(target.to_path_buf()).expect("desktop validation should start");

    let mut events = Vec::new();
    while let Ok(event) = rx.recv() {
        events.push(event);
    }
    events
}

/// Extract file-level stats from collected events.
struct RunSummary {
    total_files: usize,
    valid_files: usize,
    invalid_files: usize,
    /// file path → error count (errors + warnings)
    errors_by_file: BTreeMap<String, usize>,
    /// file path → count of Severity::Error only
    hard_errors_by_file: BTreeMap<String, usize>,
    finished: bool,
}

fn summarize(events: &[FrontendEvent]) -> RunSummary {
    let mut summary = RunSummary {
        total_files: 0,
        valid_files: 0,
        invalid_files: 0,
        errors_by_file: BTreeMap::new(),
        hard_errors_by_file: BTreeMap::new(),
        finished: false,
    };

    for event in events {
        match event {
            FrontendEvent::Started { total_files } => {
                summary.total_files = *total_files;
            }
            FrontendEvent::Errors {
                file, diagnostics, ..
            } => {
                *summary.errors_by_file.entry(file.clone()).or_default() += diagnostics.len();
                let hard = diagnostics
                    .iter()
                    .filter(|diagnostic| {
                        let json = serde_json::to_value(&diagnostic.error).unwrap();
                        json["severity"].as_str() == Some("Error")
                    })
                    .count();
                if hard > 0 {
                    *summary.hard_errors_by_file.entry(file.clone()).or_default() += hard;
                }
            }
            FrontendEvent::FileComplete { status, .. } => {
                let json = serde_json::to_value(status).unwrap();
                match json["type"].as_str() {
                    Some("valid") => summary.valid_files += 1,
                    Some("invalid") => summary.invalid_files += 1,
                    _ => {}
                }
            }
            FrontendEvent::Finished { .. } => {
                summary.finished = true;
            }
            _ => {}
        }
    }
    summary
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[test]
fn reference_corpus_no_hard_errors() {
    let corpus = reference_corpus();
    if !corpus.exists() {
        println!(
            "Skipping: reference corpus not present at {}",
            corpus.display()
        );
        return;
    }

    let events = collect_events(&corpus);
    let summary = summarize(&events);

    assert!(summary.finished, "validation run did not finish");
    assert_eq!(
        summary.total_files, 97,
        "expected 97 reference corpus files"
    );

    // Reference corpus may have warnings but must have zero hard errors
    assert!(
        summary.hard_errors_by_file.is_empty(),
        "reference corpus should produce zero errors (Severity::Error), but got errors in: {:?}",
        summary.hard_errors_by_file
    );

    // All files should complete (valid or invalid-with-warnings-only)
    assert_eq!(
        summary.valid_files + summary.invalid_files,
        97,
        "all 97 files should complete: {} valid + {} invalid = {}",
        summary.valid_files,
        summary.invalid_files,
        summary.valid_files + summary.invalid_files,
    );
}

#[test]
fn event_lifecycle_has_correct_sequence() {
    let corpus = reference_corpus();
    if !corpus.exists() {
        println!("Skipping: reference corpus not present");
        return;
    }

    let events = collect_events(&corpus);

    // First event should be Discovering
    assert!(
        matches!(events.first(), Some(FrontendEvent::Discovering)),
        "first event should be Discovering"
    );

    // Second event should be Started
    assert!(
        matches!(events.get(1), Some(FrontendEvent::Started { .. })),
        "second event should be Started"
    );

    // Last event should be Finished
    assert!(
        matches!(events.last(), Some(FrontendEvent::Finished { .. })),
        "last event should be Finished"
    );

    // Count FileComplete events — should equal total_files
    let file_completes = events
        .iter()
        .filter(|e| matches!(e, FrontendEvent::FileComplete { .. }))
        .count();

    if let Some(FrontendEvent::Started { total_files }) = events.get(1) {
        assert_eq!(
            file_completes, *total_files,
            "number of FileComplete events should match total_files"
        );
    }
}

#[test]
fn frontend_events_serialize_to_expected_json_shape() {
    let corpus = reference_corpus();
    if !corpus.exists() {
        println!("Skipping: reference corpus not present");
        return;
    }

    let events = collect_events(&corpus);

    for event in &events {
        let json = serde_json::to_value(event).unwrap();

        // Every event must have a "type" field (from #[serde(tag = "type")])
        assert!(
            json.get("type").is_some(),
            "event missing 'type' field: {json}"
        );

        let ty = json["type"].as_str().unwrap();
        match ty {
            "discovering" => {}
            "started" => {
                assert!(
                    json.get("totalFiles").is_some(),
                    "started missing totalFiles"
                );
            }
            "errors" => {
                assert!(json.get("file").is_some(), "errors missing file");
                assert!(
                    json.get("diagnostics").is_some(),
                    "errors missing diagnostics array"
                );
                assert!(json.get("source").is_some(), "errors missing source");
                if let Some(first) = json["diagnostics"]
                    .as_array()
                    .and_then(|items| items.first())
                {
                    assert!(first.get("error").is_some(), "diagnostic missing error");
                    assert!(
                        first.get("renderedHtml").is_some(),
                        "diagnostic missing renderedHtml"
                    );
                    assert!(
                        first.get("renderedText").is_some(),
                        "diagnostic missing renderedText"
                    );
                }
            }
            "fileComplete" => {
                assert!(json.get("file").is_some(), "fileComplete missing file");
                assert!(json.get("status").is_some(), "fileComplete missing status");
                let status = &json["status"];
                assert!(
                    status.get("type").is_some(),
                    "fileComplete status missing type"
                );
            }
            "finished" => {
                let stats = &json["stats"];
                assert!(
                    stats.get("totalFiles").is_some(),
                    "finished missing totalFiles"
                );
                assert!(
                    stats.get("validFiles").is_some(),
                    "finished missing validFiles"
                );
                assert!(
                    stats.get("invalidFiles").is_some(),
                    "finished missing invalidFiles"
                );
            }
            other => panic!("unexpected event type: {other}"),
        }
    }
}

#[test]
fn protocol_contracts_serialize_to_expected_json_shape() {
    assert_eq!(protocol::events::VALIDATION, "validation-event");
    assert_eq!(protocol::commands::VALIDATE, "validate");
    assert_eq!(protocol::commands::CANCEL_VALIDATION, "cancel_validation");
    assert_eq!(
        protocol::commands::CHECK_CLAN_AVAILABLE,
        "check_clan_available"
    );
    assert_eq!(protocol::commands::OPEN_IN_CLAN, "open_in_clan");
    assert_eq!(protocol::commands::EXPORT_RESULTS, "export_results");
    assert_eq!(
        protocol::commands::REVEAL_IN_FILE_MANAGER,
        "reveal_in_file_manager"
    );

    let validate = serde_json::to_value(ValidateRequest {
        path: "/tmp/reference".into(),
    })
    .unwrap();
    assert_eq!(validate["path"], "/tmp/reference");

    let open_in_clan = serde_json::to_value(OpenInClanRequest {
        file: "/tmp/reference.cha".into(),
        line: 12,
        col: 4,
        byte_offset: 33,
        msg: "E001: bad".into(),
    })
    .unwrap();
    assert_eq!(open_in_clan["file"], "/tmp/reference.cha");
    assert_eq!(open_in_clan["line"], 12);
    assert_eq!(open_in_clan["col"], 4);
    assert_eq!(open_in_clan["byteOffset"], 33);
    assert_eq!(open_in_clan["msg"], "E001: bad");

    let export_request = serde_json::to_value(ExportResultsRequest {
        results: "[]".into(),
        format: ExportFormat::Json,
        path: "/tmp/results.json".into(),
    })
    .unwrap();
    assert_eq!(export_request["results"], "[]");
    assert_eq!(export_request["format"], "json");
    assert_eq!(export_request["path"], "/tmp/results.json");
}

#[test]
fn single_file_validation() {
    // Use a known file from the reference corpus
    let file = workspace_root().join("resources/corpus/reference/core/basic-conversation.cha");
    if !file.exists() {
        println!("Skipping: {} not present", file.display());
        return;
    }

    // Desktop single-file validation should validate exactly the selected file.
    let events = collect_events(&file);
    let summary = summarize(&events);

    assert!(
        summary.finished,
        "run did not finish, got {} events",
        events.len()
    );
    assert_eq!(
        summary.total_files, 1,
        "single-file runs should report one file"
    );
    assert!(
        summary.hard_errors_by_file.is_empty(),
        "core/ files should have no hard errors"
    );

    let completed_files: Vec<_> = events
        .iter()
        .filter_map(|event| {
            if let FrontendEvent::FileComplete { file, .. } = event {
                Some(file.clone())
            } else {
                None
            }
        })
        .collect();

    assert_eq!(
        completed_files.len(),
        1,
        "single-file runs should complete one file"
    );
    assert_eq!(
        completed_files[0],
        file.to_string_lossy(),
        "single-file runs should only complete the selected file"
    );
}

#[test]
fn finished_stats_match_file_events() {
    let corpus = reference_corpus();
    if !corpus.exists() {
        println!("Skipping: reference corpus not present");
        return;
    }

    let events = collect_events(&corpus);

    let file_completes = events
        .iter()
        .filter(|e| matches!(e, FrontendEvent::FileComplete { .. }))
        .count();

    if let Some(FrontendEvent::Finished { stats }) = events.last() {
        let stats_json = serde_json::to_value(stats).unwrap();
        let total = stats_json["totalFiles"].as_u64().unwrap() as usize;
        let valid = stats_json["validFiles"].as_u64().unwrap() as usize;
        let invalid = stats_json["invalidFiles"].as_u64().unwrap() as usize;

        assert_eq!(
            file_completes, total,
            "FileComplete count should match stats.totalFiles"
        );
        assert_eq!(
            valid + invalid + stats_json["parseErrors"].as_u64().unwrap() as usize,
            total,
            "valid + invalid + parseErrors should equal total"
        );
    } else {
        panic!("last event should be Finished");
    }
}

/// Test with a corpus that has known subdirectories to verify tree structure.
/// Uses the reference corpus which has core/, tiers/, etc. subdirectories.
#[test]
fn nested_directory_produces_relative_paths_with_subdirs() {
    let corpus = reference_corpus();
    if !corpus.exists() {
        println!("Skipping: reference corpus not present");
        return;
    }

    let events = collect_events(&corpus);

    // Collect all file paths from Errors and FileComplete events
    let mut all_files: Vec<String> = Vec::new();
    for event in &events {
        match event {
            FrontendEvent::Errors { file, .. } => {
                all_files.push(file.clone());
            }
            FrontendEvent::FileComplete { file, .. } => {
                if !all_files.contains(file) {
                    all_files.push(file.clone());
                }
            }
            _ => {}
        }
    }

    // All paths should be absolute (the event bridge sends absolute paths)
    for file in &all_files {
        assert!(
            file.starts_with('/') || file.contains(":\\"),
            "file path should be absolute: {file}"
        );
    }

    // There should be files from multiple subdirectories
    let unique_dirs: std::collections::BTreeSet<_> = all_files
        .iter()
        .filter_map(|f| {
            let p = std::path::Path::new(f);
            p.parent().map(|d| d.to_string_lossy().into_owned())
        })
        .collect();

    assert!(
        unique_dirs.len() > 1,
        "reference corpus should have files in multiple subdirectories, but found dirs: {:?}",
        unique_dirs
    );
    println!("Found {} unique directories in events:", unique_dirs.len());
    for dir in &unique_dirs {
        println!("  {dir}");
    }
}

/// Verify that files with errors (including warnings) appear in Errors events.
/// This is what the FileTree filters on.
#[test]
fn files_with_any_errors_appear_in_error_events() {
    let corpus = reference_corpus();
    if !corpus.exists() {
        println!("Skipping: reference corpus not present");
        return;
    }

    let events = collect_events(&corpus);

    let mut error_files: Vec<String> = Vec::new();
    for event in &events {
        if let FrontendEvent::Errors {
            file, diagnostics, ..
        } = event
        {
            println!("Errors event: {} ({} diagnostics)", file, diagnostics.len());
            for diagnostic in diagnostics {
                let json = serde_json::to_value(&diagnostic.error).unwrap();
                println!(
                    "  {} [{}] {}",
                    json["code"], json["severity"], json["message"]
                );
            }
            error_files.push(file.clone());
        }
    }

    println!("\nTotal files with errors: {}", error_files.len());
    println!("These are the files the FileTree would show.");
}

/// Test against ~/testchat/bad/ which has both root-level and nested/ files.
/// Skips gracefully if the directory doesn't exist.
#[test]
fn testchat_bad_nested_directory() {
    let testchat = std::path::PathBuf::from(env!("HOME")).join("testchat/bad");
    if !testchat.exists() {
        println!("Skipping: ~/testchat/bad/ not present");
        return;
    }

    let events = collect_events(&testchat);
    let summary = summarize(&events);

    assert!(summary.finished);
    println!("Total files: {}", summary.total_files);
    println!("Files with errors: {}", summary.errors_by_file.len());

    // Check that nested/ files appear
    let nested_count = summary
        .errors_by_file
        .keys()
        .filter(|k| k.contains("/nested/"))
        .count();
    println!("Files in nested/: {nested_count}");

    // Print tree structure that the frontend would build
    let root_str = testchat.to_string_lossy();
    println!("\nTree structure (files with errors only):");
    let mut sorted_paths: Vec<_> = summary.errors_by_file.keys().collect();
    sorted_paths.sort();
    for path in &sorted_paths {
        let rel = if path.starts_with(&*root_str) {
            let r = &path[root_str.len()..];
            if r.starts_with('/') { &r[1..] } else { r }
        } else {
            path.as_str()
        };
        let depth = rel.matches('/').count();
        let indent = "  ".repeat(depth);
        let name = rel.rsplit('/').next().unwrap_or(rel);
        let errors = summary.errors_by_file[*path];
        println!("  {indent}✗ {name} ({errors})");
    }

    assert!(summary.total_files > 0, "should find files");
    assert!(
        !summary.errors_by_file.is_empty(),
        "should have some files with errors"
    );
    assert!(
        nested_count > 0,
        "nested/ directory files should have errors too"
    );
}

/// Verify that error events carry paired rendered miette HTML per diagnostic.
#[test]
fn rendered_html_present_for_errors() {
    let corpus = reference_corpus();
    if !corpus.exists() {
        println!("Skipping: reference corpus not present");
        return;
    }

    let events = collect_events(&corpus);

    for event in &events {
        if let FrontendEvent::Errors { diagnostics, .. } = event {
            for diagnostic in diagnostics {
                assert!(
                    !diagnostic.rendered_html.is_empty(),
                    "rendered HTML must not be empty"
                );
                // Should contain miette box-drawing characters
                assert!(
                    diagnostic.rendered_html.contains("│")
                        || diagnostic.rendered_html.contains("╭")
                        || diagnostic.rendered_html.contains("warning")
                        || diagnostic.rendered_html.contains("error"),
                    "rendered HTML should contain miette-style content, got: {}",
                    &diagnostic.rendered_html[..diagnostic.rendered_html.len().min(200)]
                );
                // ANSI colors should be converted to HTML style attributes
                assert!(
                    diagnostic.rendered_html.contains("style="),
                    "rendered HTML should contain ANSI-to-HTML color styles, got: {}",
                    &diagnostic.rendered_html[..diagnostic.rendered_html.len().min(200)]
                );
            }
        }
    }
}

#[test]
fn non_chat_files_are_rejected_by_path_contract() {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let file = std::env::temp_dir().join(format!(
        "chatter-gui-path-contract-{}-{}.txt",
        std::process::id(),
        now
    ));
    std::fs::write(&file, "not a .cha file").unwrap();

    let result = validate_target_streaming(file.clone());

    std::fs::remove_file(&file).ok();

    let message = result.expect_err("non-.cha file should be rejected");
    assert!(
        message.contains("one .cha file or one folder at a time"),
        "unexpected rejection message: {message}"
    );
}
