//! Convert error corpus .cha files to markdown error specifications
//!
//! This tool reads error corpus files from tests/error_corpus/
//! and generates markdown error specifications in the format expected by
//! validate_error_specs and gen_validation_tests.
//!
//! Usage:
//!   cargo run --bin corpus_to_specs -- \
//!     --corpus-dir ~/talkbank-tools/tests/error_corpus \
//!     --spec-dir ../spec/errors

use clap::Parser as ClapParser;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use thiserror::Error;

/// CLI arguments: corpus directory, output spec directory, and overwrite flag.
#[derive(ClapParser, Debug)]
#[clap(name = "corpus_to_specs")]
#[clap(about = "Convert error corpus files to markdown error specifications")]
struct Args {
    /// Directory containing error corpus files
    #[clap(long, value_name = "DIR")]
    corpus_dir: PathBuf,

    /// Output directory for generated specs
    #[clap(long, value_name = "DIR")]
    spec_dir: PathBuf,

    /// Overwrite existing spec files
    #[clap(long)]
    overwrite: bool,
}

#[derive(Debug)]
struct ErrorCorpusFile {
    path: PathBuf,
    error_code: Option<String>,
    actual_codes: Vec<String>,
    description: Option<String>,
    trigger: Option<String>,
    category: Option<String>,
    chat_example: String,
}

#[derive(Debug, Error)]
pub enum CorpusSpecError {
    #[error("Failed to read file: {path}")]
    Read {
        path: String,
        source: std::io::Error,
    },
    #[error("Failed to parse CHAT file")]
    Parse,
    #[error("Failed to write spec file: {path}")]
    Write {
        path: String,
        source: std::io::Error,
    },
}

#[derive(Debug, Clone)]
enum CommentDirective {
    ExpectedError {
        code: String,
        description: Option<String>,
    },
    ExpectedWarning {
        code: String,
    },
    Trigger(String),
    Category(String),
    CorpusMarker,
}

use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct Expectations {
    files: HashMap<String, FileExpectation>,
}

#[derive(Debug, Deserialize)]
struct FileExpectation {
    tree_sitter: TreeSitterExpectation,
}

#[derive(Debug, Deserialize)]
struct TreeSitterExpectation {
    codes: Vec<String>,
}

/// Converts legacy error corpus `.cha` files into Markdown error spec files.
fn main() -> Result<(), CorpusSpecError> {
    let args = Args::parse();

    println!(
        "Converting error corpus files from {} to specs in {}",
        args.corpus_dir.display(),
        args.spec_dir.display()
    );

    // Load expectations.json
    let expectations_path = args.corpus_dir.join("expectations.json");
    let expectations: Expectations = if expectations_path.exists() {
        let content =
            fs::read_to_string(&expectations_path).map_err(|source| CorpusSpecError::Read {
                path: expectations_path.display().to_string(),
                source,
            })?;
        serde_json::from_str(&content).unwrap_or_else(|_| Expectations {
            files: HashMap::new(),
        })
    } else {
        Expectations {
            files: HashMap::new(),
        }
    };

    let corpus_files = discover_corpus_files(&args.corpus_dir)?;
    println!("Found {} corpus files", corpus_files.len());

    let mut parsed_files = Vec::new();
    for path in &corpus_files {
        match parse_corpus_file(path, &args.corpus_dir, &expectations) {
            Ok(file) => parsed_files.push(file),
            Err(err) => eprintln!("Warning: Failed to parse {}: {}", path.display(), err),
        }
    }

    let mut by_error_code: HashMap<String, Vec<ErrorCorpusFile>> = HashMap::new();
    for file in parsed_files {
        if let Some(ref code) = file.error_code {
            by_error_code.entry(code.clone()).or_default().push(file);
        }
    }

    println!(
        "
Found {} unique error codes",
        by_error_code.len()
    );

    fs::create_dir_all(&args.spec_dir).map_err(|source| CorpusSpecError::Write {
        path: args.spec_dir.display().to_string(),
        source,
    })?;

    let mut generated = 0;
    let mut skipped = 0;

    for (error_code, files) in &by_error_code {
        let spec_path = args.spec_dir.join(format!("{}_auto.md", error_code));

        if spec_path.exists() && !args.overwrite {
            println!("Skipping {} (already exists)", error_code);
            skipped += 1;
            continue;
        }

        if let Some(spec) = generate_aggregated_spec(error_code, files) {
            fs::write(&spec_path, spec).map_err(|source| CorpusSpecError::Write {
                path: spec_path.display().to_string(),
                source,
            })?;
            println!(
                "Generated spec for {} with {} examples",
                error_code,
                files.len()
            );
            generated += 1;
        }
    }

    println!(
        "
Generated {} specs, skipped {} existing",
        generated, skipped
    );
    Ok(())
}

fn discover_corpus_files(dir: &Path) -> Result<Vec<PathBuf>, CorpusSpecError> {
    let mut files = Vec::new();

    for entry in fs::read_dir(dir).map_err(|source| CorpusSpecError::Read {
        path: dir.display().to_string(),
        source,
    })? {
        let entry = entry.map_err(|source| CorpusSpecError::Read {
            path: dir.display().to_string(),
            source,
        })?;
        let path = entry.path();

        if path.is_dir() {
            files.extend(discover_corpus_files(&path)?);
        } else if path.extension().and_then(|s| s.to_str()) == Some("cha") {
            files.push(path);
        }
    }

    Ok(files)
}

fn parse_corpus_file(
    path: &Path,
    corpus_root: &Path,
    expectations: &Expectations,
) -> Result<ErrorCorpusFile, CorpusSpecError> {
    let content = fs::read_to_string(path).map_err(|source| CorpusSpecError::Read {
        path: path.display().to_string(),
        source,
    })?;

    // Get relative path for expectations lookup
    let rel_path = path.strip_prefix(corpus_root).unwrap_or(path);
    let rel_path_str = rel_path.to_string_lossy().to_string();
    let actual_codes = expectations
        .files
        .get(&rel_path_str)
        .map(|e| e.tree_sitter.codes.clone())
        .unwrap_or_default();

    let mut error_code = None;
    let mut description = None;
    let mut trigger = None;
    let mut category = None;
    let mut filtered_lines = Vec::new();

    for line in content.lines() {
        if line.starts_with("@Comment:") {
            let text = line.trim_start_matches("@Comment:").trim();
            let mut is_directive = false;
            if let Some(directive) = parse_comment_directive(text) {
                is_directive = true;
                match directive {
                    CommentDirective::ExpectedError {
                        code,
                        description: desc,
                    } => {
                        // Only set error_code from the FIRST directive
                        if error_code.is_none() {
                            error_code = Some(code);
                            description = desc;
                        }
                    }
                    CommentDirective::ExpectedWarning { code } => {
                        if error_code.is_none() {
                            error_code = Some(code);
                        }
                    }
                    CommentDirective::Trigger(value) => {
                        trigger = Some(value);
                    }
                    CommentDirective::Category(value) => {
                        category = Some(value);
                    }
                    CommentDirective::CorpusMarker => {}
                }
            } else if text.contains("Expected error:")
                || text.contains("Expected tree-sitter error:")
                || text.contains("Expected direct error:")
                || text.contains("Expected warning:")
                || text.contains("Trigger:")
                || text.contains("Category:")
                || text.contains("ERROR CORPUS TEST FILE")
            {
                is_directive = true;
            }

            if !is_directive {
                filtered_lines.push(line.to_string());
            }
        } else {
            filtered_lines.push(line.to_string());
        }
    }

    let chat_example = filtered_lines.join("\n");

    // Fallback: extract error code from filename if no directive found
    if error_code.is_none() {
        if let Some(code) = extract_code_from_filename(path) {
            error_code = Some(code);
        }
    }

    Ok(ErrorCorpusFile {
        path: rel_path.to_path_buf(),
        error_code,
        actual_codes,
        description,
        trigger,
        category,
        chat_example,
    })
}

fn generate_aggregated_spec(error_code: &str, files: &[ErrorCorpusFile]) -> Option<String> {
    if files.is_empty() {
        return None;
    }

    let primary = &files[0];
    let description = primary
        .description
        .as_deref()
        .unwrap_or("Auto-generated from corpus");
    let category = primary.category.as_deref().unwrap_or("validation");

    let (level, _) = infer_metadata(error_code);

    // Infer layer: if ANY example is in parse_errors, mark as parser
    let is_parser = files.iter().any(|f| {
        let path_str = f.path.to_string_lossy();
        path_str.contains("parse_errors")
            || path_str.contains("E2xx")
            || path_str.contains("E3xx")
            || path_str.contains("E7xx")
    });
    let layer = if is_parser { "parser" } else { "validation" };

    let mut output = format!(
        r#"# {}: {}

## Description

{}

## Metadata

- **Error Code**: {}
- **Category**: {}
- **Level**: {}
- **Layer**: {}

"#,
        error_code, description, description, error_code, category, level, layer
    );

    for (i, file) in files.iter().enumerate() {
        let trigger = file.trigger.as_deref().unwrap_or("See example below");
        let codes = if file.actual_codes.is_empty() {
            error_code.to_string()
        } else {
            file.actual_codes.join(", ")
        };

        output.push_str(&format!(
            r#"## Example {}

**Source**: `{}`
**Trigger**: {}
**Expected Error Codes**: {}

```chat
{}
```

"#,
            i + 1,
            file.path.display(),
            trigger,
            codes,
            file.chat_example
        ));
    }

    output.push_str(
        r#"## Expected Behavior

The parser should successfully parse these CHAT files (unless marked as parser layer), and the appropriate error should be reported.

## CHAT Rule

[Add link to relevant CHAT manual section]

## Notes

- Auto-generated from error corpus
- Review and enhance this specification as needed
"#,
    );

    Some(output)
}

fn infer_metadata(error_code: &str) -> (&'static str, &'static str) {
    let prefix = error_code_prefix(error_code);
    match prefix {
        Some(b'2') => ("word", "validation"),
        Some(b'3') => ("utterance", "validation"),
        Some(b'4') => ("tier", "validation"),
        Some(b'5') => ("header", "validation"),
        Some(b'6') => ("tier", "validation"),
        Some(b'7') => ("tier", "parser"),
        _ => ("file", "validation"),
    }
}

fn error_code_prefix(error_code: &str) -> Option<u8> {
    let bytes = error_code.as_bytes();
    if bytes.len() < 2 {
        return None;
    }
    if bytes[0] != b'E' && bytes[0] != b'W' {
        return None;
    }
    // For W codes, use the second digit to infer category
    Some(bytes[1])
}

/// Extract error code from filename, e.g. "E003_empty_string.cha" → "E003"
fn extract_code_from_filename(path: &Path) -> Option<String> {
    let stem = path.file_stem()?.to_str()?;
    // Match E### or W### at start of filename
    if stem.len() >= 4
        && (stem.starts_with('E') || stem.starts_with('W'))
        && stem[1..4].chars().all(|c| c.is_ascii_digit())
    {
        Some(stem[..4].to_string())
    } else {
        None
    }
}

/// Parse a comment directive from a CHAT comment line.
///
/// Recognized forms:
/// - `"Expected error: E123"` or `"Expected error: E123 (description)"`
/// - `"Expected tree-sitter error: E456 (description)"`
/// - `"Expected direct error: E789"`
/// - `"Expected warning: W100"`
/// - `"Trigger: some text"`
/// - `"Category: some text"`
/// - `"ERROR CORPUS TEST FILE"`
fn parse_comment_directive(text: &str) -> Option<CommentDirective> {
    let text = text.trim();

    if text == "ERROR CORPUS TEST FILE" {
        return Some(CommentDirective::CorpusMarker);
    }

    // Try each "Expected ..." prefix, all mapping to the same code+description parse.
    for prefix in [
        "Expected error:",
        "Expected tree-sitter error:",
        "Expected direct error:",
    ] {
        if let Some(rest) = text.strip_prefix(prefix) {
            let (code, description) = parse_code_and_description(rest)?;
            return Some(CommentDirective::ExpectedError { code, description });
        }
    }

    if let Some(rest) = text.strip_prefix("Expected warning:") {
        let (code, _) = parse_code_and_description(rest)?;
        return Some(CommentDirective::ExpectedWarning { code });
    }

    if let Some(rest) = text.strip_prefix("Trigger:") {
        let value = rest.trim_start_matches([' ', '\t']);
        return Some(CommentDirective::Trigger(value.to_string()));
    }

    if let Some(rest) = text.strip_prefix("Category:") {
        let value = rest.trim_start_matches([' ', '\t']);
        return Some(CommentDirective::Category(value.to_string()));
    }

    None
}

/// Parse an error/warning code like `"E123"` or `"W456"`, optionally followed
/// by `"(description text)"`.
fn parse_code_and_description(input: &str) -> Option<(String, Option<String>)> {
    let input = input.trim_start_matches([' ', '\t']);

    // Code must start with E or W followed by digits.
    let first = input.chars().next()?;
    if first != 'E' && first != 'W' {
        return None;
    }
    let digit_end = input[1..]
        .find(|c: char| !c.is_ascii_digit())
        .map(|i| i + 1)
        .unwrap_or(input.len());
    if digit_end <= 1 {
        return None; // No digits after prefix.
    }
    let code = input[..digit_end].to_string();

    // Optional description in parentheses after whitespace.
    let rest = input[digit_end..].trim_start_matches([' ', '\t']);
    let description = rest
        .strip_prefix('(')
        .and_then(|inner| inner.strip_suffix(')'))
        .map(|s| s.to_string());

    Some((code, description))
}
