//! Find representative CHAT files from a corpus data directory for each target language
//!
//! Walks data repositories, filters by language header, scores files by
//! construct coverage, line count, tier variety, and speaker diversity.
//!
//! Usage:
//!   cargo run --bin extract_corpus_candidates -- \
//!     --data-dir ../data \
//!     --languages eng,zho,fra,deu,spa,jpn \
//!     --node-types ../../../tree-sitter-talkbank/src/node-types.json \
//!     --max-lines 200 \
//!     --top 10

use clap::ArgAction;
use clap::Parser as ClapParser;
use serde::Serialize;
use std::collections::BTreeSet;
use std::fs;
use std::path::PathBuf;
use talkbank_model::ErrorCollector;
use talkbank_parser::TreeSitterParser;
use tree_sitter::Parser as TSParser;
use tree_sitter_talkbank::LANGUAGE;
use walkdir::WalkDir;

#[derive(ClapParser)]
#[command(name = "extract_corpus_candidates")]
#[command(about = "Find representative CHAT files for the reference corpus")]
struct Args {
    /// Root directory containing data repositories
    #[arg(long)]
    data_dir: PathBuf,

    /// Comma-separated list of target language codes
    #[arg(long)]
    languages: String,

    /// Path to tree-sitter node-types.json (for scoring unique node types)
    #[arg(long)]
    node_types: PathBuf,

    /// Maximum line count (files above this are skipped)
    #[arg(long, default_value = "200")]
    max_lines: usize,

    /// Number of top candidates per language
    #[arg(long, default_value = "10")]
    top: usize,

    /// Optional cap on number of .cha files to inspect (for fast iteration).
    #[arg(long)]
    max_files: Option<usize>,

    /// Output as JSON
    #[arg(long)]
    json: bool,

    /// Require successful parsing via Rust parser API (not just CST parse).
    #[arg(long, default_value_t = true, action = ArgAction::Set)]
    require_rust_parse: bool,

    /// Require zero diagnostics from Rust model validation.
    #[arg(long, default_value_t = true, action = ArgAction::Set)]
    require_rust_validation: bool,

    /// Include alignment checks during validation (implies validation work).
    #[arg(long, default_value_t = true, action = ArgAction::Set)]
    validate_alignment: bool,

    /// Optional output file for results (JSON if --json, text otherwise).
    #[arg(long)]
    output: Option<PathBuf>,
}

#[derive(Debug, Clone, Serialize)]
struct Candidate {
    path: String,
    language: String,
    line_count: usize,
    unique_node_types: usize,
    tier_variety: Vec<String>,
    speaker_count: usize,
    has_mor: bool,
    has_gra: bool,
    has_pho: bool,
    has_wor: bool,
    rust_parse_errors: usize,
    rust_validation_errors: usize,
    score: f64,
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    if !args.node_types.exists() {
        anyhow::bail!(
            "--node-types path does not exist: {}",
            args.node_types.display()
        );
    }

    let target_langs: Vec<String> = args
        .languages
        .split(',')
        .map(|s| s.trim().to_lowercase())
        .collect();

    // Initialize tree-sitter parser
    let mut parser = TSParser::new();
    parser.set_language(&LANGUAGE.into())?;

    // Collect candidates per language
    let mut all_candidates: std::collections::BTreeMap<String, Vec<Candidate>> =
        std::collections::BTreeMap::new();

    for lang in &target_langs {
        all_candidates.insert(lang.clone(), Vec::new());
    }

    let mut data_repos: Vec<PathBuf> = std::fs::read_dir(&args.data_dir)?
        .filter_map(|e| e.ok())
        .filter_map(|e| {
            let path = e.path();
            if !path.is_dir() {
                return None;
            }
            let name = e.file_name().to_string_lossy().to_string();
            if name.ends_with("-data") && !name.ends_with("-xml") {
                Some(path)
            } else {
                None
            }
        })
        .collect();

    if data_repos.is_empty() {
        data_repos.push(args.data_dir.clone());
    }

    eprintln!(
        "Scanning {} data repos for {} languages...",
        data_repos.len(),
        target_langs.len()
    );

    let mut scanned_files = 0usize;
    let mut accepted_files = 0usize;

    'repo_loop: for repo in &data_repos {
        for entry in WalkDir::new(repo).into_iter().filter_map(|e| e.ok()) {
            if let Some(limit) = args.max_files {
                if scanned_files >= limit {
                    break 'repo_loop;
                }
            }
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("cha") {
                continue;
            }
            scanned_files += 1;

            let source = match std::fs::read_to_string(path) {
                Ok(s) => s,
                Err(_) => continue,
            };

            // Check line count
            let line_count = source.lines().count();
            if line_count > args.max_lines || line_count < 10 {
                continue;
            }

            // Extract language from @Languages header
            let file_lang = extract_language(&source);
            let file_lang = match file_lang {
                Some(l) => l.to_lowercase(),
                None => continue,
            };

            // Check if this language is one we want
            if !target_langs.contains(&file_lang) {
                continue;
            }

            // Parse with tree-sitter
            let tree = match parser.parse(&source, None) {
                Some(t) => t,
                None => continue,
            };

            // Reject files with ERROR nodes
            if has_error_node(tree.root_node()) {
                continue;
            }

            let (rust_parse_errors, rust_validation_errors) = rust_validity_counts(
                &source,
                args.require_rust_validation,
                args.validate_alignment,
            );
            if args.require_rust_parse && rust_parse_errors > 0 {
                continue;
            }
            if args.require_rust_validation && rust_validation_errors > 0 {
                continue;
            }

            // Score the file
            let unique_nodes = count_unique_node_types(tree.root_node());
            let tiers = extract_tier_types(&source);
            let speakers = count_speakers(&source);
            let has_mor = tiers.contains(&"mor".to_string());
            let has_gra = tiers.contains(&"gra".to_string());
            let has_pho = tiers.contains(&"pho".to_string());
            let has_wor = tiers.contains(&"wor".to_string());

            // Scoring: prefer variety, short files, multiple speakers
            let line_score = if line_count <= 50 {
                1.0
            } else if line_count <= 100 {
                0.8
            } else {
                0.5
            };

            let tier_score = tiers.len() as f64 * 0.2;
            let node_score = unique_nodes as f64 * 0.01;
            let speaker_score = if speakers >= 2 { 0.3 } else { 0.0 };
            let mor_bonus = if has_mor { 0.5 } else { 0.0 };

            let score = line_score + tier_score + node_score + speaker_score + mor_bonus;

            let candidate = Candidate {
                path: path.to_string_lossy().to_string(),
                language: file_lang.clone(),
                line_count,
                unique_node_types: unique_nodes,
                tier_variety: tiers,
                speaker_count: speakers,
                has_mor,
                has_gra,
                has_pho,
                has_wor,
                rust_parse_errors,
                rust_validation_errors,
                score,
            };

            if let Some(candidates) = all_candidates.get_mut(&file_lang) {
                candidates.push(candidate);
                accepted_files += 1;
            }
        }
    }

    eprintln!(
        "Scanned {} .cha files; accepted {} candidates before top-N truncation.",
        scanned_files, accepted_files
    );

    // Sort by score and take top N
    for candidates in all_candidates.values_mut() {
        candidates.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap());
        candidates.truncate(args.top);
    }

    let rendered = if args.json {
        serde_json::to_string_pretty(&all_candidates)?
    } else {
        let mut out = String::new();
        for (lang, candidates) in &all_candidates {
            out.push_str(&format!(
                "\n=== {} ({} candidates) ===\n",
                lang.to_uppercase(),
                candidates.len()
            ));
            for (i, c) in candidates.iter().enumerate() {
                out.push_str(&format!(
                    "  {}. [score={:.2}] {} ({} lines, {} nodes, {} speakers, tiers: [{}], rust_parse_errors: {}, rust_validation_errors: {})\n",
                    i + 1,
                    c.score,
                    c.path,
                    c.line_count,
                    c.unique_node_types,
                    c.speaker_count,
                    c.tier_variety.join(", "),
                    c.rust_parse_errors,
                    c.rust_validation_errors
                ));
            }
        }
        out
    };

    if let Some(path) = &args.output {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(path, &rendered)?;
        eprintln!("Wrote candidate report: {}", path.display());
    } else {
        println!("{rendered}");
    }

    Ok(())
}

fn rust_validity_counts(
    source: &str,
    run_validation: bool,
    validate_alignment: bool,
) -> (usize, usize) {
    let parser = TreeSitterParser::new().expect("grammar loads");
    let mut chat_file = match parser.parse_chat_file(source) {
        Ok(file) => file,
        Err(parse_errors) => return (parse_errors.len(), 0),
    };

    if !run_validation {
        return (0, 0);
    }

    let sink = ErrorCollector::new();
    if validate_alignment {
        chat_file.validate_with_alignment(&sink, None);
    } else {
        chat_file.validate(&sink, None);
    }
    let validation_errors = sink.into_vec();
    (0, validation_errors.len())
}

fn extract_language(source: &str) -> Option<String> {
    for line in source.lines() {
        if let Some(rest) = line.strip_prefix("@Languages:") {
            let rest = rest.trim().trim_start_matches('\t');
            // Take the first language code
            let lang = rest.split([',', ' ']).next()?.trim().to_string();
            if lang.len() == 3 {
                return Some(lang);
            }
        }
    }
    None
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

fn count_unique_node_types(node: tree_sitter::Node) -> usize {
    let mut types = BTreeSet::new();
    collect_types(node, &mut types);
    types.len()
}

fn collect_types(node: tree_sitter::Node, types: &mut BTreeSet<String>) {
    if node.is_named() {
        types.insert(node.kind().to_string());
    }
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_types(child, types);
    }
}

fn extract_tier_types(source: &str) -> Vec<String> {
    let mut tiers = BTreeSet::new();
    for line in source.lines() {
        if line.starts_with('%') {
            if let Some(label) = line.split(':').next() {
                let label = label.trim_start_matches('%');
                tiers.insert(label.to_string());
            }
        }
    }
    tiers.into_iter().collect()
}

fn count_speakers(source: &str) -> usize {
    let mut speakers = BTreeSet::new();
    for line in source.lines() {
        if line.starts_with('*') {
            if let Some(speaker) = line.split(':').next() {
                let speaker = speaker.trim_start_matches('*');
                speakers.insert(speaker.to_string());
            }
        }
    }
    speakers.len()
}
