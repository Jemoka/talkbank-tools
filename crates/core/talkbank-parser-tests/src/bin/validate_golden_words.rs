//! Validate and optionally clean the golden-word corpus.
//!
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Format>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Main_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>

use clap::Parser as ClapParser;
use regex::Regex;
use std::sync::LazyLock;
use std::{
    fs::{self, File},
    io::Write,
    path::PathBuf,
};
use talkbank_parser::TreeSitterParser;
use talkbank_parser_tests::test_error::TestError;

static EMPTY_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^\s*$").expect("valid regex"));

static COMMENT_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^\s*#").expect("valid regex"));

static WORD_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\s*(\S+)\s*$").expect("valid regex"));

/// Validate golden words against current parser and regenerate if needed.
#[derive(ClapParser, Debug)]
#[command(
    name = "validate-golden-words",
    about = "Validate that golden words are still parseable as standalone words"
)]
/// Command-line arguments for this tool.
struct Args {
    /// Path to golden words file.
    #[arg(long, default_value = "talkbank-parser-tests/golden_words.txt")]
    input: PathBuf,

    /// Write cleaned golden words to this file (default: overwrite input).
    #[arg(long)]
    output: Option<PathBuf>,

    /// Only report invalid words, don't write output.
    #[arg(long)]
    check_only: bool,
}

/// Entry point for this binary target.
fn main() -> Result<(), TestError> {
    let args = Args::parse();

    let parser = TreeSitterParser::new().map_err(|err| TestError::ParserInit(err.to_string()))?;

    let line_parser = LineParser;
    let content = fs::read_to_string(&args.input)?;
    let mut valid_words = Vec::new();
    let mut invalid_words = Vec::new();

    for line in content.lines() {
        let parsed = line_parser.parse_line(line)?;
        let word = match parsed {
            LineKind::EmptyOrComment => {
                valid_words.push(line.to_string());
                continue;
            }
            LineKind::Word(word) => word,
        };

        // Try to parse as a word
        let result = parser.parse_word(&word);

        match result {
            Ok(_word) => {
                // Valid word
                valid_words.push(line.to_string());
            }
            Err(parse_errors) => {
                // Invalid word - report it
                invalid_words.push((word.clone(), parse_errors.errors.clone()));
                eprintln!("INVALID: {:?}", word);
                for err in &parse_errors.errors {
                    eprintln!("  Error: {}", err.message);
                }
            }
        }
    }

    println!("\n=== Validation Summary ===");
    println!("Total lines: {}", content.lines().count());
    println!("Valid words: {}", valid_words.len());
    println!("Invalid words: {}", invalid_words.len());
    println!("==========================\n");

    if invalid_words.is_empty() {
        println!("✓ All golden words are valid!");
        return Ok(());
    }

    if args.check_only {
        println!(
            "⚠ Found {} invalid words (use without --check-only to clean)",
            invalid_words.len()
        );
        return Ok(());
    }

    // Write cleaned file
    let output_path = match args.output.as_ref() {
        Some(path) => path,
        None => &args.input,
    };
    let mut file = File::create(output_path)?;

    for line in &valid_words {
        writeln!(file, "{}", line)?;
    }

    println!(
        "✓ Wrote {} valid words to {}",
        valid_words.len(),
        output_path.display()
    );
    println!("  Removed {} invalid words", invalid_words.len());

    Ok(())
}

/// Classification of one line in a golden-words corpus file.
enum LineKind {
    EmptyOrComment,
    Word(String),
}

/// Classifier for golden-words corpus lines using module-level regex statics.
struct LineParser;

impl LineParser {
    /// Parses line.
    fn parse_line(&self, line: &str) -> Result<LineKind, TestError> {
        if EMPTY_RE.is_match(line) || COMMENT_RE.is_match(line) {
            return Ok(LineKind::EmptyOrComment);
        }

        let caps = WORD_RE
            .captures(line)
            .ok_or_else(|| TestError::Failure(format!("Invalid golden word line: {line}")))?;
        let word = caps
            .get(1)
            .ok_or_else(|| TestError::Failure(format!("Failed to capture word in line: {line}")))?
            .as_str()
            .to_string();

        Ok(LineKind::Word(word))
    }
}
