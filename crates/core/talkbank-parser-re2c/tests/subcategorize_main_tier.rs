//! Sub-categorize main_tier divergences between Re2cParser and TreeSitterParser.
//!
//! Samples divergent files and classifies the FIRST non-span diff path
//! into fine-grained categories.
//!
//! Run:
//! ```bash
//! cargo test -p talkbank-parser-re2c --test subcategorize_main_tier --release -- --ignored --nocapture
//! ```

use std::collections::BTreeMap;
use std::path::PathBuf;
use talkbank_model::errors::ErrorCollector;
use talkbank_model::{ChatParser, ParseOutcome, SemanticEq};
use talkbank_parser::TreeSitterParser;
use talkbank_parser_re2c::Re2cParser;

/// Strip all span/content_span keys from JSON.
fn strip_spans(val: &mut serde_json::Value) {
    match val {
        serde_json::Value::Object(map) => {
            map.remove("span");
            map.remove("content_span");
            for v in map.values_mut() {
                strip_spans(v);
            }
        }
        serde_json::Value::Array(arr) => {
            for v in arr {
                strip_spans(v);
            }
        }
        _ => {}
    }
}

/// Find the first JSON diff path, returning (path, ts_value, re2c_value).
fn find_first_diff(
    a: &serde_json::Value,
    b: &serde_json::Value,
    path: &str,
    depth: usize,
) -> Option<(String, String, String)> {
    if depth > 25 {
        return Some((path.to_string(), "...".into(), "...".into()));
    }
    match (a, b) {
        (serde_json::Value::Object(ma), serde_json::Value::Object(mb)) => {
            for (key, va) in ma {
                if let Some(vb) = mb.get(key) {
                    let cp = if path.is_empty() {
                        key.clone()
                    } else {
                        format!("{path}.{key}")
                    };
                    if let Some(d) = find_first_diff(va, vb, &cp, depth + 1) {
                        return Some(d);
                    }
                } else {
                    return Some((
                        format!("{path}.{key}"),
                        format!("{va}").chars().take(200).collect(),
                        "<missing>".into(),
                    ));
                }
            }
            for key in mb.keys() {
                if !ma.contains_key(key) {
                    return Some((
                        format!("{path}.{key}"),
                        "<missing>".into(),
                        format!("{}", mb[key]).chars().take(200).collect(),
                    ));
                }
            }
            None
        }
        (serde_json::Value::Array(va), serde_json::Value::Array(vb)) => {
            if va.len() != vb.len() {
                return Some((
                    format!("{path}[]"),
                    format!("len={}", va.len()),
                    format!("len={}", vb.len()),
                ));
            }
            for (i, (ea, eb)) in va.iter().zip(vb.iter()).enumerate() {
                if let Some(d) = find_first_diff(ea, eb, &format!("{path}[{i}]"), depth + 1) {
                    return Some(d);
                }
            }
            None
        }
        (a, b) if a == b => None,
        (a, b) => Some((
            path.to_string(),
            format!("{a}").chars().take(200).collect(),
            format!("{b}").chars().take(200).collect(),
        )),
    }
}

/// Extract a sub-category from a main_tier diff path.
fn sub_classify(path: &str, ts_val: &str, re2c_val: &str) -> String {
    // Content length mismatch
    if path.ends_with(".content[]") || path.ends_with(".content.content[]") {
        return "content_length_mismatch".into();
    }

    // Event type
    if path.contains("event_type") {
        return format!("event_type (ts={ts_val}, re2c={re2c_val})");
    }

    // Raw text differences
    if path.contains("raw_text") {
        // Truncate for classification
        let ts_short: String = ts_val.chars().take(30).collect();
        let re2c_short: String = re2c_val.chars().take(30).collect();
        return format!("raw_text (ts={ts_short}, re2c={re2c_short})");
    }

    // Word content items
    if path.contains(".content[") && path.contains("].content") {
        if path.contains("word_type") || path.contains("line_type") {
            return "word_type_mismatch".into();
        }
        if path.contains("category") {
            return "word_category".into();
        }
        if path.contains("form_type") {
            return "form_type".into();
        }
        if path.contains("lang") {
            return "lang_marker".into();
        }
        return "word_content_structure".into();
    }

    // Variant mismatches (different enum type)
    if path.contains("line_type") || path.contains("type") {
        return format!("type_mismatch (ts={ts_val}, re2c={re2c_val})");
    }

    // Annotation
    if path.contains("annotation") || path.contains("scoped") {
        return "annotation".into();
    }

    // Terminator
    if path.contains("terminator") || path.contains("utterance_terminator") {
        return "terminator".into();
    }

    // Catch-all: use last path segment
    let last = path.rsplit('.').next().unwrap_or(path);
    format!(
        "other:{last} (ts={}, re2c={})",
        ts_val.chars().take(40).collect::<String>(),
        re2c_val.chars().take(40).collect::<String>()
    )
}

fn corpus_base() -> PathBuf {
    PathBuf::from(
        std::env::var("TALKBANK_DATA")
            .unwrap_or_else(|_| format!("{}/talkbank/data", std::env::var("HOME").unwrap())),
    )
}

fn collect_cha_files(base: &std::path::Path) -> Vec<PathBuf> {
    walkdir::WalkDir::new(base)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().is_some_and(|ext| ext == "cha"))
        .map(|e| e.path().to_path_buf())
        .collect()
}

#[test]
#[ignore]
fn subcategorize_main_tier_divergences() {
    let base = corpus_base();
    if !base.exists() {
        eprintln!("Skipping: {} not found", base.display());
        return;
    }

    let mut files = collect_cha_files(&base);
    files.sort();
    eprintln!("Found {} .cha files", files.len());

    let ts = TreeSitterParser::new().expect("tree-sitter grammar loads");
    let re2c = Re2cParser::new();

    let mut subcats: BTreeMap<String, usize> = BTreeMap::new();
    let mut subcat_examples: BTreeMap<String, Vec<(String, String)>> = BTreeMap::new();
    let mut total_divergent = 0;
    let mut main_tier_divergent = 0;

    for (i, path) in files.iter().enumerate() {
        if i > 0 && i % 10000 == 0 {
            eprintln!(
                "  Progress: {}/{} ({main_tier_divergent} main_tier divergences)",
                i,
                files.len()
            );
        }

        let content = match std::fs::read_to_string(path) {
            Ok(c) => c,
            Err(_) => continue,
        };

        let ts_errors = ErrorCollector::new();
        let ts_file = ts.parse_chat_file_streaming(&content, &ts_errors);

        let re2c_errors = ErrorCollector::new();
        let re2c_file = match re2c.parse_chat_file(&content, 0, &re2c_errors) {
            ParseOutcome::Parsed(f) => f,
            ParseOutcome::Rejected => continue,
        };

        if ts_file.semantic_eq(&re2c_file) {
            continue;
        }
        total_divergent += 1;

        let mut ts_json = serde_json::to_value(&ts_file).unwrap_or_default();
        let mut re2c_json = serde_json::to_value(&re2c_file).unwrap_or_default();
        strip_spans(&mut ts_json);
        strip_spans(&mut re2c_json);

        if let Some((diff_path, ts_val, re2c_val)) = find_first_diff(&ts_json, &re2c_json, "", 0) {
            // Only sub-categorize main tier divergences
            if diff_path.contains(".main.") || diff_path.contains(".main_") {
                main_tier_divergent += 1;
                let subcat = sub_classify(&diff_path, &ts_val, &re2c_val);
                *subcats.entry(subcat.clone()).or_insert(0) += 1;

                let examples = subcat_examples.entry(subcat).or_default();
                if examples.len() < 5 {
                    let file_str = path
                        .strip_prefix(&base)
                        .unwrap_or(path)
                        .display()
                        .to_string();
                    examples.push((file_str, diff_path));
                }
            }
        }
    }

    eprintln!("\n=== MAIN TIER SUB-CATEGORIZATION ===");
    eprintln!("Total divergent: {total_divergent}");
    eprintln!("Main tier divergent: {main_tier_divergent}");
    eprintln!();

    let mut sorted: Vec<_> = subcats.iter().collect();
    sorted.sort_by(|a, b| b.1.cmp(a.1));

    for (subcat, count) in &sorted {
        let pct = **count as f64 / main_tier_divergent.max(1) as f64 * 100.0;
        eprintln!("  {subcat}");
        eprintln!("    count: {count} ({pct:.1}%)");
        if let Some(examples) = subcat_examples.get(*subcat) {
            for (file, path) in examples.iter().take(3) {
                eprintln!("    ex: {file}");
                eprintln!("        {path}");
            }
        }
        eprintln!();
    }

    // Write report
    let report: Vec<serde_json::Value> = sorted
        .iter()
        .map(|(subcat, count)| {
            let examples = subcat_examples
                .get(*subcat)
                .map(|ex| {
                    ex.iter()
                        .map(|(f, p)| serde_json::json!({"file": f, "path": p}))
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            serde_json::json!({ "subcategory": subcat, "count": count, "examples": examples })
        })
        .collect();
    let _ = std::fs::write(
        "/tmp/re2c_main_tier_subcategories.json",
        serde_json::to_string_pretty(&report).unwrap_or_default(),
    );
    eprintln!("Report: /tmp/re2c_main_tier_subcategories.json");
}
