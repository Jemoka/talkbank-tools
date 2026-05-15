//! Single file roundtrip test for focused debugging.
//!
//! Usage:
//!   cargo test --test single_file_roundtrip -- --file action.cha
//!
//! Or use the helper script:
//!   ./scripts/test-single-file.sh action.cha

use clap::Parser;
use std::path::PathBuf;
use talkbank_model::WriteChat;

#[path = "test_utils/mod.rs"]
mod test_utils;

#[path = "single_file_roundtrip/config.rs"]
mod config;
#[path = "single_file_roundtrip/diagnostics.rs"]
mod diagnostics;
#[path = "single_file_roundtrip/io.rs"]
mod io;

/// Type representing Args.
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// The CHAT file to test
    #[arg(long, short = 'f')]
    file: Option<String>,

    /// Optional corpus directory (defaults to resources/corpus/reference)
    #[arg(long, short = 'd')]
    corpus_dir: Option<PathBuf>,

    /// Print the original CHAT file before diagnostics (for debugging)
    #[arg(long)]
    show_original: bool,
}

/// Entry point for this binary target.
fn main() {
    let args = Args::parse();

    let test_file = match args.file {
        Some(f) => f,
        None => {
            eprintln!("Error: --file is required.");
            eprintln!("Usage: cargo test --test single_file_roundtrip -- --file action.cha");
            std::process::exit(0);
        }
    };
    let corpus_path = args
        .corpus_dir
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/corpus/reference"));

    let file_path = io::resolve_test_file(&corpus_path, &test_file);

    if !file_path.exists() {
        eprintln!("Test file does not exist: {}", file_path.display());
        std::process::exit(1);
    }

    diagnostics::print_header(&test_file, &file_path);

    let original_content = io::read_file(&file_path);

    if args.show_original {
        diagnostics::print_original(&original_content);
    }

    let chat_file = diagnostics::parse_chat_file(&file_path, &original_content);

    diagnostics::verify_json_serialization(&chat_file);

    let serialized = chat_file.to_chat_string();
    diagnostics::print_serialized(&serialized);

    // Semantic comparison - parse serialized output and compare models
    diagnostics::assert_roundtrip_semantic(&chat_file, &serialized, &original_content, &file_path);
}
