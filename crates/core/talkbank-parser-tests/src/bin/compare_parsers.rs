//! Parse a CHAT file with TreeSitterParser and report results.
//!
//! Usage:
//! ```bash
//! cargo run -p talkbank-parser-tests --bin compare_parsers -- path/to/file.cha
//! ```
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Format>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Main_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>

use std::env;
use std::fs;
use std::path::PathBuf;
use talkbank_model::ErrorCollector;
use talkbank_model::ParseOutcome;
use talkbank_parser::TreeSitterParser;

/// Entry point for this binary target.
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();
    if args.len() != 2 {
        eprintln!("Usage: {} <file.cha>", args[0]);
        eprintln!("Example: {} resources/corpus/reference/dep-tiers.cha", args[0]);
        std::process::exit(1);
    }

    let path = PathBuf::from(&args[1]);
    let input = fs::read_to_string(&path)?;

    println!("=== Parsing file: {} ===\n", path.display());

    let parser = TreeSitterParser::new()?;
    let errors = ErrorCollector::new();
    let result = parser.parse_chat_file_fragment(&input, 0, &errors);

    match result {
        ParseOutcome::Parsed(chat_file) => {
            println!("Parse succeeded");

            let errs = errors.to_vec();
            if errs.is_empty() {
                println!("No errors");
            } else {
                println!("\nErrors ({}):", errs.len());
                for err in &errs {
                    println!("  - {}", err.message);
                }
            }

            // Serialize to JSON for inspection
            let json = match serde_json::to_string_pretty(&chat_file) {
                Ok(json) => json,
                Err(err) => {
                    eprintln!("JSON serialization failed: {err}");
                    "JSON serialization failed".to_string()
                }
            };

            let output_path = "/tmp/parser_output.json";
            fs::write(output_path, &json)?;
            println!("\nOutput written to: {}", output_path);
        }
        ParseOutcome::Rejected => {
            println!("Parse FAILED");
            let errs = errors.into_vec();
            println!("\nErrors ({}):", errs.len());
            for err in errs {
                println!("  - {}", err.message);
            }
        }
    }

    Ok(())
}
