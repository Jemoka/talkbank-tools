//! Report grammar node type coverage for the reference corpus
//!
//! Loads all named node types from `node-types.json`, parses every `.cha` file
//! in the corpus with tree-sitter, and reports which concrete node types are
//! exercised versus missing.
//!
//! Usage:
//!   cargo run --bin corpus_node_coverage -- \
//!     --corpus-dir ../../resources/corpus/reference \
//!     --node-types ../../../tree-sitter-talkbank/src/node-types.json
//!
//! Add `--json` for machine-readable output.
//! Exit code 1 if any concrete types are missing.

use clap::Parser as ClapParser;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use tree_sitter::Parser as TSParser;
use tree_sitter_talkbank::LANGUAGE;
use walkdir::WalkDir;

const REFERENCE_CORPUS_EXCLUDED_CONCRETE_TYPES: &[&str] = &[
    // Strict+catch-all pattern: generic variants rarely appear in well-formed corpora
    "generic_id_sex",
    "generic_media_status",
    "generic_media_type",
    "generic_number",
    "generic_option_name",
    "generic_recording_quality",
    "generic_transcription",
    "strict_date",
    "strict_time",
    // Parser-recovery concession: `;` is a `non_colon_separator` choice in
    // the main tier grammar so malformed input parses gracefully, but
    // semicolons in well-formed CHAT only appear inside `age_format`
    // tokens (`2;06.`), not as standalone separators. The validator
    // would flag a freestanding `;` on a main tier as a syntax error.
    "semicolon",
    // Structured word grammar nodes — need reference files with stress markers, POS tags
    "pos_tag",
    "stress_marker",
    // @ID SES subcategory nodes
    "ethnicity_value",
    "generic_id_ses",
    // Uncommon tier/header types not in reference corpus
    "modsyl_dependent_tier",
    "modsyl_tier_prefix",
    "phoaln_dependent_tier",
    "phoaln_tier_prefix",
    "phosyl_dependent_tier",
    "phosyl_tier_prefix",
    "thumbnail_header",
    "thumbnail_prefix",
    "unsupported_dependent_tier",
    "unsupported_header",
    "unsupported_header_prefix",
    "unsupported_line",
    "unsupported_tier_prefix",
];

#[derive(ClapParser)]
#[command(name = "corpus_node_coverage")]
#[command(about = "Report grammar node type coverage for a corpus")]
struct Args {
    /// Directory containing .cha files to analyze
    #[arg(long)]
    corpus_dir: PathBuf,

    /// Path to tree-sitter node-types.json
    #[arg(long)]
    node_types: PathBuf,

    /// Output as JSON
    #[arg(long)]
    json: bool,
}

/// A single entry from node-types.json
#[derive(Debug, Deserialize)]
struct NodeTypeEntry {
    #[serde(rename = "type")]
    type_name: String,
    named: bool,
    #[serde(default)]
    subtypes: Vec<SubtypeEntry>,
}

#[derive(Debug, Deserialize)]
struct SubtypeEntry {
    #[serde(rename = "type")]
    #[allow(dead_code)]
    type_name: String,
    #[allow(dead_code)]
    named: bool,
}

#[derive(Debug, Serialize)]
struct CoverageReport {
    total_concrete: usize,
    exercised: usize,
    missing_count: usize,
    coverage_pct: f64,
    supertype_count: usize,
    supertypes: Vec<String>,
    excluded_concrete: Vec<String>,
    missing: Vec<String>,
    files_parsed: usize,
    files_with_errors: usize,
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    // Load node-types.json
    let node_types_json = std::fs::read_to_string(&args.node_types)
        .map_err(|e| anyhow::anyhow!("Failed to read {}: {}", args.node_types.display(), e))?;

    let entries: Vec<NodeTypeEntry> = serde_json::from_str(&node_types_json)
        .map_err(|e| anyhow::anyhow!("Failed to parse node-types.json: {}", e))?;

    // Separate supertypes from concrete types
    let mut supertypes: BTreeSet<String> = BTreeSet::new();
    let mut concrete_types: BTreeSet<String> = BTreeSet::new();

    for entry in &entries {
        if !entry.named {
            continue;
        }
        if !entry.subtypes.is_empty() {
            // This is a supertype (union type)
            supertypes.insert(entry.type_name.clone());
        } else {
            concrete_types.insert(entry.type_name.clone());
        }
    }
    let excluded_concrete: BTreeSet<String> = REFERENCE_CORPUS_EXCLUDED_CONCRETE_TYPES
        .iter()
        .map(|kind| kind.to_string())
        .collect();
    for kind in &excluded_concrete {
        concrete_types.remove(kind);
    }

    // Initialize tree-sitter parser
    let mut parser = TSParser::new();
    parser
        .set_language(&LANGUAGE.into())
        .map_err(|e| anyhow::anyhow!("Failed to set tree-sitter language: {}", e))?;

    // Walk corpus and collect exercised node types
    let mut exercised: BTreeSet<String> = BTreeSet::new();
    let mut files_parsed: usize = 0;
    let mut files_with_errors: usize = 0;
    // Track which files exercise each node type (for debugging)
    let mut type_to_files: BTreeMap<String, Vec<String>> = BTreeMap::new();

    for entry in WalkDir::new(&args.corpus_dir)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("cha") {
            continue;
        }

        let source = match std::fs::read_to_string(path) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("Warning: failed to read {}: {}", path.display(), e);
                continue;
            }
        };

        let tree = match parser.parse(&source, None) {
            Some(t) => t,
            None => {
                eprintln!(
                    "Warning: tree-sitter returned no tree for {}",
                    path.display()
                );
                continue;
            }
        };

        files_parsed += 1;

        // Check for ERROR nodes
        let root = tree.root_node();
        if has_error_node(root) {
            files_with_errors += 1;
        }

        // Walk CST and collect all named node types
        let file_name = path
            .strip_prefix(&args.corpus_dir)
            .unwrap_or(path)
            .to_string_lossy()
            .to_string();

        collect_node_types(root, &mut exercised, &mut type_to_files, &file_name);
    }

    // Compute coverage against concrete types only
    let exercised_concrete: BTreeSet<String> =
        exercised.intersection(&concrete_types).cloned().collect();

    let missing: Vec<String> = concrete_types
        .difference(&exercised_concrete)
        .cloned()
        .collect();

    let total = concrete_types.len();
    let exercised_count = exercised_concrete.len();
    let coverage_pct = if total > 0 {
        (exercised_count as f64 / total as f64) * 100.0
    } else {
        100.0
    };

    let report = CoverageReport {
        total_concrete: total,
        exercised: exercised_count,
        missing_count: missing.len(),
        coverage_pct,
        supertype_count: supertypes.len(),
        supertypes: supertypes.iter().cloned().collect(),
        excluded_concrete: excluded_concrete.iter().cloned().collect(),
        missing: missing.clone(),
        files_parsed,
        files_with_errors,
    };

    if args.json {
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        println!("=== Corpus Node Coverage ===\n");
        println!("Corpus: {}", args.corpus_dir.display());
        println!(
            "Files parsed: {} ({} with ERROR nodes)",
            files_parsed, files_with_errors
        );
        println!();
        println!(
            "Supertypes (excluded from coverage): {} types",
            supertypes.len()
        );
        for st in &supertypes {
            println!("  - {}", st);
        }
        println!();
        println!(
            "Concrete types excluded from gate: {} types",
            excluded_concrete.len()
        );
        for kind in &excluded_concrete {
            println!("  - {}", kind);
        }
        println!();
        println!(
            "Concrete types: {}/{} exercised ({:.1}%)",
            exercised_count, total, coverage_pct
        );
        println!();

        if missing.is_empty() {
            println!("All concrete node types are exercised!");
        } else {
            println!("Missing concrete types ({}):", missing.len());
            for m in &missing {
                println!("  - {}", m);
            }
        }

        // Show node types that appear in CST but are not in concrete_types
        // (these are supertypes or unnamed nodes that appeared)
        let exercised_supertypes: Vec<&String> = exercised.intersection(&supertypes).collect();
        if !exercised_supertypes.is_empty() {
            println!();
            println!("Supertypes seen in CST: {}", exercised_supertypes.len());
        }
    }

    if !missing.is_empty() {
        std::process::exit(1);
    }

    Ok(())
}

fn has_error_node(node: tree_sitter::Node) -> bool {
    if node.is_error() || node.is_missing() {
        return true;
    }
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if has_error_node(child) {
            return true;
        }
    }
    false
}

fn collect_node_types(
    node: tree_sitter::Node,
    exercised: &mut BTreeSet<String>,
    type_to_files: &mut BTreeMap<String, Vec<String>>,
    file_name: &str,
) {
    if node.is_named() {
        let kind = node.kind().to_string();
        if exercised.insert(kind.clone()) {
            // First time seeing this type
            type_to_files
                .entry(kind)
                .or_default()
                .push(file_name.to_string());
        } else {
            // Already seen, but track files for debugging
            type_to_files
                .entry(node.kind().to_string())
                .or_default()
                .push(file_name.to_string());
        }
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_node_types(child, exercised, type_to_files, file_name);
    }
}
