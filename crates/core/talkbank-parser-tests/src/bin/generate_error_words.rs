//! Generate error words corpus from error specifications.
//!
//! Extracts example error cases from resources/spec/errors/*.md files and creates
//! a curated corpus of words that should trigger parser/validation errors.
//!
//! ## Usage
//!
//! ```bash
//! cargo run -p talkbank-parser-tests --bin generate_error_words
//! ```
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Format>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Main_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>

use comrak::nodes::{AstNode, NodeValue};
use comrak::{Arena, Options, parse_document};
use regex::Regex;
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::sync::LazyLock;
use talkbank_model::ErrorCollector;
use talkbank_parser::TreeSitterParser;
use talkbank_parser::node_types;
use talkbank_parser_tests::test_error::TestError;
use tree_sitter::{Node, Parser as TsParser};
use tree_sitter_talkbank::LANGUAGE;

static PARSER_LAYER_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?mi)^\s*-\s+\*\*Layer\*\*:\s*parser\s*$").expect("valid regex"));

static WORD_LEVEL_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?mi)^\s*-\s+\*\*Level\*\*:\s*word\s*$").expect("valid regex"));

static ERROR_CODE_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^E\d{3}$").expect("valid regex"));

/// Data container for ErrorExample.
struct ErrorExample {
    code: String,
    description: String,
    invalid_word: String,
}

/// Extracts word from example.
fn extract_word_from_example(example: &str) -> Result<Option<String>, TestError> {
    let mut parser = TsParser::new();
    parser
        .set_language(&LANGUAGE.into())
        .map_err(|err| TestError::Failure(format!("Failed to set tree-sitter language: {err}")))?;
    let tree = parser.parse(example, None).ok_or_else(|| {
        TestError::Failure("Failed to parse example with tree-sitter".to_string())
    })?;
    let root = tree.root_node();
    let main_tier = find_first_descendant(root, &|node: Node| node.kind() == node_types::MAIN_TIER);
    let search_root = match main_tier {
        Some(node) => node,
        None => root,
    };
    let word_node = find_first_descendant(search_root, &is_word_node);
    let word_node: Node = match word_node {
        Some(node) => node,
        None => return Ok(None),
    };
    let text = word_node
        .utf8_text(example.as_bytes())
        .map_err(|err| TestError::Failure(format!("Failed to extract word text: {err}")))?
        .to_string();
    Ok(Some(text))
}

/// Parses error spec.
fn parse_error_spec(path: &Path) -> Result<Option<ErrorExample>, TestError> {
    let content = fs::read_to_string(path)?;
    let arena = Arena::new();
    let root = parse_document(&arena, &content, &Options::default());

    // Extract error code from filename (e.g., "E202_auto.md" -> "E202")
    let filename = path
        .file_stem()
        .and_then(|s| s.to_str())
        .ok_or_else(|| TestError::Failure(format!("Invalid filename for {}", path.display())))?;
    let code = filename
        .split_once('_')
        .map(|(prefix, _)| prefix)
        .ok_or_else(|| TestError::Failure(format!("Missing error code in filename {}", filename)))?
        .to_string();

    // Skip if not an error code
    if !is_error_code(&code) {
        return Ok(None);
    }

    // Extract description (first H1 heading)
    let description = extract_heading_text(root, 1).ok_or_else(|| {
        TestError::Failure(format!("Missing top-level heading in {}", path.display()))
    })?;

    // Check metadata - we only want parser-layer, word-level errors for parse_word testing.
    let is_parser_layer = has_parser_word_layer(&content);

    if !is_parser_layer {
        return Ok(None); // Skip validation-layer errors
    }

    // Extract example from ```chat code block
    let example = extract_chat_code_block(root).ok_or_else(|| {
        TestError::Failure(format!("Missing chat code block in {}", path.display()))
    })?;
    let invalid_word = extract_word_from_example(&example)?
        .or_else(|| extract_word_fallback_from_chat(&example))
        .filter(|word| !word.is_empty());
    let Some(invalid_word) = invalid_word else {
        return Ok(None);
    };

    Ok(Some(ErrorExample {
        code,
        description,
        invalid_word,
    }))
}

/// Entry point for this binary target.
fn main() -> Result<(), TestError> {
    println!("=== Generating Error Words Corpus ===\n");

    // Find all error spec files (path from repo root)
    // CARGO_MANIFEST_DIR is crates/talkbank-parser-tests
    // We need to go up to repo root
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR")?;
    let spec_dir = Path::new(&manifest_dir).join("../../../spec/errors"); // crates/talkbank-parser-tests -> crates -> repo root

    if !spec_dir.exists() {
        return Err(TestError::Failure(format!(
            "spec/errors directory not found at {}",
            spec_dir.display()
        )));
    }

    let mut examples: HashMap<String, ErrorExample> = HashMap::new();

    // Parse all error specs
    for entry in fs::read_dir(spec_dir)? {
        let entry = entry?;
        let path = entry.path();

        let stem = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or_default();
        let is_error_spec = stem.starts_with('E')
            && stem.chars().nth(1).is_some_and(|c| c.is_ascii_digit())
            && stem.chars().nth(2).is_some_and(|c| c.is_ascii_digit())
            && stem.chars().nth(3).is_some_and(|c| c.is_ascii_digit());

        if path.extension().and_then(|s| s.to_str()) == Some("md") && is_error_spec {
            if let Some(example) = parse_error_spec(&path)? {
                println!("  Found: {} - {}", example.code, example.invalid_word);
                examples.insert(example.code.clone(), example);
            }
        }
    }

    println!(
        "
Extracted {} parser-layer error examples",
        examples.len()
    );

    if examples.is_empty() {
        println!("Warning: No parser-layer error examples found!");
        println!("Check that resources/spec/errors/*.md files contain:");
        println!("  - Layer: parser metadata");
        println!("  - ```chat example blocks");
        println!("  - Main tier lines with invalid words");
        return Ok(());
    }

    // Keep only examples that actually fail parse_word (parse failure or emitted errors).
    // This prevents corpus drift where parser-layer metadata exists but the extracted token
    // is valid as a standalone word and requires utterance/file context to fail.
    let parser = TreeSitterParser::new()
        .map_err(|err| TestError::Failure(format!("Failed to initialize parser: {err}")))?;
    let mut filtered_examples: Vec<_> = examples
        .into_iter()
        .filter(|(_, example)| word_fails_parse_word(&parser, &example.invalid_word))
        .collect();

    if filtered_examples.is_empty() {
        return Err(TestError::Failure(
            "No extracted examples failed parse_word; cannot build error words corpus".to_string(),
        ));
    }

    // Sort by error code
    filtered_examples.sort_by_key(|(code, _)| code.clone());

    // Generate error words file
    let mut output_lines = vec![
        "# Error Words Corpus".to_string(),
        "# Generated by generate_error_words binary".to_string(),
        format!(
            "# Total: {} words (verified to fail parse_word)",
            filtered_examples.len()
        ),
        "#".to_string(),
        "# Format: word | error_code | description".to_string(),
        "#".to_string(),
        "# Each word should fail parse_word (parse failure or emitted parser errors).".to_string(),
        "# Use this corpus to validate word-parser error detection behavior.".to_string(),
        "#".to_string(),
        "# Word format: <invalid_word>".to_string(),
        "# The comment shows: # <error_code>: <description>".to_string(),
        "".to_string(),
    ];

    for (code, example) in &filtered_examples {
        output_lines.push(format!("# {}: {}", code, example.description));
        output_lines.push(example.invalid_word.clone());
        output_lines.push(String::new()); // Blank line
    }

    let output_path = "error_words_corpus.txt";
    let content = output_lines.join("\n");

    fs::write(output_path, content)?;

    println!(
        "\n✓ Wrote {} error words to {}",
        filtered_examples.len(),
        output_path
    );
    println!("\nError words by code:");
    for (code, example) in filtered_examples {
        println!(
            "  {}: {} - \"{}\"",
            code, example.description, example.invalid_word
        );
    }

    println!("\n=== Summary ===");
    println!("Generated error test corpus for parse_word validation.");
    println!("These words should fail parsing or emit parser errors.");
    println!("\nNext step: Create tests/error_words_validation.rs to test these.");

    Ok(())
}

/// Extracts heading text.
fn extract_heading_text<'a>(root: &'a AstNode<'a>, level: u8) -> Option<String> {
    for node in root.descendants() {
        if let NodeValue::Heading(heading) = &node.data.borrow().value {
            if heading.level == level {
                return Some(collect_text(node));
            }
        }
    }
    None
}

/// Returns whether parser word layer.
fn has_parser_word_layer(content: &str) -> bool {
    PARSER_LAYER_RE.is_match(content) && WORD_LEVEL_RE.is_match(content)
}

/// Extracts chat code block.
fn extract_chat_code_block<'a>(root: &'a AstNode<'a>) -> Option<String> {
    for node in root.descendants() {
        if let NodeValue::CodeBlock(code_block) = &node.data.borrow().value {
            let lang = code_block.info.split_whitespace().next();
            if matches!(lang, Some("chat")) {
                return Some(code_block.literal.to_string());
            }
        }
    }
    None
}

/// Collects text.
fn collect_text<'a>(node: &'a AstNode<'a>) -> String {
    let mut text = String::new();
    collect_text_into(node, &mut text);
    normalize_whitespace(&text)
}

/// Collects text into.
fn collect_text_into<'a>(node: &'a AstNode<'a>, text: &mut String) {
    for child in node.children() {
        match &child.data.borrow().value {
            NodeValue::Text(value) => text.push_str(value),
            NodeValue::Code(code) => text.push_str(&code.literal),
            NodeValue::LineBreak | NodeValue::SoftBreak => text.push(' '),
            _ => collect_text_into(child, text),
        }
    }
}

/// Returns whether error code.
fn is_error_code(code: &str) -> bool {
    ERROR_CODE_RE.is_match(code)
}

/// Normalize whitespace in extracted example text.
fn normalize_whitespace(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Returns whether word node.
fn is_word_node(node: Node<'_>) -> bool {
    matches!(
        node.kind(),
        node_types::STANDALONE_WORD
            | node_types::WORD_WITH_OPTIONAL_ANNOTATIONS
            | node_types::NONWORD_WITH_OPTIONAL_ANNOTATIONS
    )
}

/// Finds first descendant.
fn find_first_descendant<'a, F>(node: Node<'a>, predicate: &F) -> Option<Node<'a>>
where
    F: Fn(Node<'a>) -> bool,
{
    if predicate(node) {
        return Some(node);
    }
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if let Some(found) = find_first_descendant(child, predicate) {
            return Some(found);
        }
    }
    None
}

/// Check whether a standalone word fails parser word parsing.
fn word_fails_parse_word(parser: &TreeSitterParser, word: &str) -> bool {
    let errors = ErrorCollector::new();
    let result = parser.parse_word_fragment(word, 0, &errors);
    result.is_rejected() || !errors.into_vec().is_empty()
}

/// Extracts word fallback from chat.
fn extract_word_fallback_from_chat(example: &str) -> Option<String> {
    let utterance_line = example
        .lines()
        .map(str::trim)
        .find(|line| line.starts_with('*') && line.contains(':'))?;

    let after_speaker = utterance_line
        .split_once('\t')
        .map(|(_, rhs)| rhs)
        .or_else(|| utterance_line.split_once(':').map(|(_, rhs)| rhs))
        .unwrap_or_default()
        .trim();

    let token = after_speaker.split_whitespace().next()?;
    let cleaned = token.trim_end_matches(['.', '?', '!']);
    if cleaned.is_empty() {
        None
    } else {
        Some(cleaned.to_string())
    }
}
