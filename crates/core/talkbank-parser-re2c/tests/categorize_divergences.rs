//! Categorize semantic divergences between Re2cParser and TreeSitterParser.
//!
//! For each divergent file from the full corpus run, this test:
//! 1. Parses with both parsers
//! 2. Serializes both ChatFile to JSON
//! 3. Finds the first point of JSON divergence
//! 4. Extracts the CHAT source context around that point
//! 5. Classifies the divergence by category
//!
//! Run:
//! ```bash
//! cargo test -p talkbank-parser-re2c --test categorize_divergences --release -- --ignored --nocapture
//! ```

use std::collections::BTreeMap;
use std::path::PathBuf;
use talkbank_model::errors::ErrorCollector;
use talkbank_model::{ChatParser, ParseOutcome, SemanticEq};
use talkbank_parser::TreeSitterParser;
use talkbank_parser_re2c::Re2cParser;

/// Category of divergence between the two parsers.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
enum Category {
    /// Different number of lines in ChatFile
    LineLengthMismatch,
    /// Header content differs
    HeaderDiff { header_type: String },
    /// Main tier content differs (word parsing, annotations, etc.)
    MainTierContent { detail: String },
    /// Main tier speaker differs
    MainTierSpeaker,
    /// Dependent tier count differs
    DependentTierCount,
    /// Dependent tier content differs
    DependentTierContent { tier_kind: String },
    /// Word-level difference
    WordDiff { detail: String },
    /// JSON diff at unknown path
    Other { path: String },
}

impl std::fmt::Display for Category {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Category::LineLengthMismatch => write!(f, "line_count_mismatch"),
            Category::HeaderDiff { header_type } => write!(f, "header/{header_type}"),
            Category::MainTierContent { detail } => write!(f, "main_tier/{detail}"),
            Category::MainTierSpeaker => write!(f, "main_tier/speaker"),
            Category::DependentTierCount => write!(f, "dep_tier_count"),
            Category::DependentTierContent { tier_kind } => {
                write!(f, "dep_tier/{tier_kind}")
            }
            Category::WordDiff { detail } => write!(f, "word/{detail}"),
            Category::Other { path } => write!(f, "other/{path}"),
        }
    }
}

/// Strip all "span" and "content_span" keys from a JSON value (in place).
/// These are metadata fields skipped by SemanticEq, so including them in
/// the diff produces false positives.
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

/// Compare two ChatFile JSON values and return the first divergent path.
fn find_first_json_diff(
    ts_val: &serde_json::Value,
    re2c_val: &serde_json::Value,
    path: &str,
    depth: usize,
) -> Option<(String, String, String)> {
    if depth > 20 {
        return Some((path.to_string(), "...".into(), "...".into()));
    }
    match (ts_val, re2c_val) {
        (serde_json::Value::Object(a), serde_json::Value::Object(b)) => {
            // Check all keys in a
            for (key, val_a) in a {
                if let Some(val_b) = b.get(key) {
                    let child_path = if path.is_empty() {
                        key.clone()
                    } else {
                        format!("{path}.{key}")
                    };
                    if let Some(diff) = find_first_json_diff(val_a, val_b, &child_path, depth + 1) {
                        return Some(diff);
                    }
                } else {
                    return Some((
                        format!("{path}.{key}"),
                        format!("{val_a}"),
                        "<missing>".into(),
                    ));
                }
            }
            // Check keys in b not in a
            for key in b.keys() {
                if !a.contains_key(key) {
                    return Some((
                        format!("{path}.{key}"),
                        "<missing>".into(),
                        format!("{}", b[key]),
                    ));
                }
            }
            None
        }
        (serde_json::Value::Array(a), serde_json::Value::Array(b)) => {
            if a.len() != b.len() {
                return Some((
                    format!("{path}[]"),
                    format!("len={}", a.len()),
                    format!("len={}", b.len()),
                ));
            }
            for (i, (va, vb)) in a.iter().zip(b.iter()).enumerate() {
                let child_path = format!("{path}[{i}]");
                if let Some(diff) = find_first_json_diff(va, vb, &child_path, depth + 1) {
                    return Some(diff);
                }
            }
            None
        }
        (a, b) if a == b => None,
        (a, b) => {
            let left = format!("{a}").chars().take(120).collect::<String>();
            let right = format!("{b}").chars().take(120).collect::<String>();
            Some((path.to_string(), left, right))
        }
    }
}

/// Classify a JSON diff path into a Category.
fn classify_diff_path(path: &str) -> Category {
    // Examples of paths:
    // lines[3].utterance.main.content[2].word.raw_text
    // lines[0].header.participants[1].role
    // lines[5].utterance.dependent_tiers[0].mor.items[1]

    if path.contains("lines[]") {
        return Category::LineLengthMismatch;
    }

    if path.contains(".header.") || (path.contains(".header") && !path.contains(".utterance")) {
        let header_type = if path.contains("participants") {
            "participants"
        } else if path.contains("id_header") || path.contains("id.") {
            "id"
        } else if path.contains("languages") {
            "languages"
        } else if path.contains("media") {
            "media"
        } else {
            "other"
        };
        return Category::HeaderDiff {
            header_type: header_type.to_string(),
        };
    }

    if path.contains(".dependent_tiers[]")
        || path.contains(".dependent_tiers[") && path.ends_with(']') && !path.contains('.')
    {
        return Category::DependentTierCount;
    }

    if path.contains(".dependent_tiers[") {
        let tier_kind = if path.contains(".mor") {
            "mor"
        } else if path.contains(".gra") {
            "gra"
        } else if path.contains(".pho") {
            "pho"
        } else if path.contains(".sin") {
            "sin"
        } else if path.contains(".cod") {
            "cod"
        } else if path.contains(".act") {
            "act"
        } else if path.contains(".com") {
            "com"
        } else if path.contains(".user_defined") || path.contains(".unsupported") {
            "user_defined"
        } else {
            "other"
        };
        return Category::DependentTierContent {
            tier_kind: tier_kind.to_string(),
        };
    }

    if path.contains(".main.speaker") {
        return Category::MainTierSpeaker;
    }

    if path.contains(".main.content[") || path.contains(".main.content[]") {
        let detail = if path.contains(".raw_text") {
            "raw_text"
        } else if path.contains(".word.content") || path.contains(".word_content") {
            "word_content"
        } else if path.contains(".category") {
            "word_category"
        } else if path.contains(".form_type") {
            "form_type"
        } else if path.contains(".lang") {
            "lang"
        } else if path.contains("content[]") {
            "content_length"
        } else if path.contains("annotation") {
            "annotations"
        } else if path.contains("terminator") || path.contains("utterance_terminator") {
            "terminator"
        } else {
            "other"
        };
        return Category::MainTierContent {
            detail: detail.to_string(),
        };
    }

    if path.contains(".main.") {
        let detail = if path.contains("terminator") {
            "terminator"
        } else {
            "other"
        };
        return Category::MainTierContent {
            detail: detail.to_string(),
        };
    }

    Category::Other {
        path: path.chars().take(60).collect(),
    }
}

fn corpus_base() -> PathBuf {
    PathBuf::from(
        std::env::var("TALKBANK_DATA")
            .unwrap_or_else(|_| format!("{}/talkbank/data", std::env::var("HOME").unwrap())),
    )
}

fn collect_cha_files(base: &std::path::Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    for entry in walkdir::WalkDir::new(base)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        if entry.path().extension().is_some_and(|ext| ext == "cha") {
            files.push(entry.path().to_path_buf());
        }
    }
    files.sort();
    files
}

#[test]
#[ignore]
fn categorize_corpus_divergences() {
    let base = corpus_base();
    if !base.exists() {
        eprintln!("Skipping: {} not found", base.display());
        return;
    }

    eprintln!("Collecting .cha files from {}...", base.display());
    let files = collect_cha_files(&base);
    eprintln!("Found {} .cha files", files.len());

    let ts = TreeSitterParser::new().expect("tree-sitter grammar loads");
    let re2c = Re2cParser::new();

    let mut category_counts: BTreeMap<String, usize> = BTreeMap::new();
    let mut category_examples: BTreeMap<String, Vec<(String, String, String, String)>> =
        BTreeMap::new();
    let mut total = 0;
    let mut divergent = 0;

    for (i, path) in files.iter().enumerate() {
        if i > 0 && i % 10000 == 0 {
            eprintln!("  Progress: {}/{} ({divergent} divergent)", i, files.len());
        }

        let content = match std::fs::read_to_string(path) {
            Ok(c) => c,
            Err(_) => continue,
        };
        total += 1;

        let ts_errors = ErrorCollector::new();
        let ts_file = ts.parse_chat_file_streaming(&content, &ts_errors);

        let re2c_errors = ErrorCollector::new();
        let re2c_result = re2c.parse_chat_file(&content, 0, &re2c_errors);
        let re2c_file = match re2c_result {
            ParseOutcome::Parsed(f) => f,
            ParseOutcome::Rejected => continue,
        };

        if ts_file.semantic_eq(&re2c_file) {
            continue;
        }

        divergent += 1;

        // Serialize both to JSON for structural comparison.
        // Strip span fields (skipped by SemanticEq) to avoid false positives.
        let mut ts_json = serde_json::to_value(&ts_file).unwrap_or_default();
        let mut re2c_json = serde_json::to_value(&re2c_file).unwrap_or_default();
        strip_spans(&mut ts_json);
        strip_spans(&mut re2c_json);

        if let Some((diff_path, ts_val, re2c_val)) =
            find_first_json_diff(&ts_json, &re2c_json, "", 0)
        {
            let category = classify_diff_path(&diff_path);
            let cat_str = category.to_string();
            *category_counts.entry(cat_str.clone()).or_insert(0) += 1;

            let examples = category_examples.entry(cat_str).or_default();
            if examples.len() < 3 {
                let file_str = path
                    .strip_prefix(&base)
                    .unwrap_or(path)
                    .display()
                    .to_string();
                examples.push((file_str, diff_path, ts_val, re2c_val));
            }
        }
    }

    // Report
    eprintln!("\n=== DIVERGENCE CATEGORIZATION ===");
    eprintln!("Total files: {total}");
    eprintln!("Divergent: {divergent}");
    eprintln!("\nCategories (sorted by count):\n");

    let mut sorted: Vec<_> = category_counts.iter().collect();
    sorted.sort_by(|a, b| b.1.cmp(a.1));

    for (cat, count) in &sorted {
        let pct = **count as f64 / divergent as f64 * 100.0;
        eprintln!("  {cat:45} {count:6} ({pct:5.1}%)");
        if let Some(examples) = category_examples.get(*cat) {
            for (file, path, ts_val, re2c_val) in examples {
                eprintln!("    file: {file}");
                eprintln!("    path: {path}");
                eprintln!("    ts:   {}", ts_val.chars().take(100).collect::<String>());
                eprintln!(
                    "    re2c: {}",
                    re2c_val.chars().take(100).collect::<String>()
                );
                eprintln!();
            }
        }
    }

    // Write JSON report
    let report: Vec<serde_json::Value> = sorted
        .iter()
        .map(|(cat, count)| {
            let examples = category_examples
                .get(*cat)
                .map(|ex| {
                    ex.iter()
                        .map(|(f, p, t, r)| {
                            serde_json::json!({
                                "file": f,
                                "diff_path": p,
                                "tree_sitter": t,
                                "re2c": r,
                            })
                        })
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            serde_json::json!({
                "category": cat,
                "count": count,
                "percentage": format!("{:.1}%", **count as f64 / divergent as f64 * 100.0),
                "examples": examples,
            })
        })
        .collect();

    let report_path = "/tmp/re2c_divergence_categories.json";
    if let Ok(json) = serde_json::to_string_pretty(&report) {
        let _ = std::fs::write(report_path, &json);
        eprintln!("\nFull categorized report written to {report_path}");
    }
}
