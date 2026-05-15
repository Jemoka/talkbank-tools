//! Test that all utterances in the corpus have valid tier alignment.

use clap::Parser;
use std::path::PathBuf;

#[path = "alignment_validation/config.rs"]
mod config;
#[path = "alignment_validation/discovery.rs"]
mod discovery;
#[path = "alignment_validation/runner.rs"]
mod runner;
#[path = "alignment_validation/stats.rs"]
mod stats;
#[path = "alignment_validation/validation.rs"]
mod validation;

/// Type representing Args.
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Directory containing the corpus to test
    #[arg(long, short = 'd')]
    corpus_dir: Option<PathBuf>,
}

/// Entry point for this binary target.
fn main() {
    let args = Args::parse();
    let corpus_dir = match args.corpus_dir {
        Some(dir) => dir,
        None => {
            eprintln!("Error: --corpus-dir is required.");
            eprintln!("Usage: cargo test --test alignment_validation -- --corpus-dir ~/my-corpus");
            std::process::exit(0);
        }
    };

    if !corpus_dir.exists() {
        eprintln!("Corpus directory does not exist: {}", corpus_dir.display());
        std::process::exit(1);
    }

    let files = match discovery::list_chat_files(&corpus_dir) {
        Ok(files) => files,
        Err(err) => {
            eprintln!("Failed to list corpus files: {err}");
            std::process::exit(1);
        }
    };
    if files.is_empty() {
        eprintln!("No .cha files found in {}", corpus_dir.display());
        std::process::exit(1);
    }

    let parser = match talkbank_parser::TreeSitterParser::new() {
        Ok(parser) => parser,
        Err(err) => {
            eprintln!("Failed to create parser: {err}");
            std::process::exit(1);
        }
    };
    let mut stats = stats::AlignmentStats::default();

    for path in files {
        if let Err(err) = validation::validate_file(&parser, &path, &mut stats) {
            eprintln!("Validation error: {err}");
        }
    }

    stats.print_summary();
}
