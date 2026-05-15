//! Fix layer classification in auto-generated specs
//!
//! This tool reads specs marked as 'validation' layer, tests if their
//! CHAT input parses successfully, and changes the layer to 'parser'
//! if parsing fails.
//!
//! Usage:
//!   cargo run --bin fix_spec_layers -- --spec-dir ../spec/errors

use clap::Parser as ClapParser;
use comrak::nodes::{AstNode, NodeValue};
use comrak::{format_commonmark, parse_document, Arena, Options};
use std::borrow::Cow;
use std::fs;
use std::path::{Path, PathBuf};
use thiserror::Error;
use tree_sitter::Parser as TSParser;
use tree_sitter_talkbank::LANGUAGE;

/// CLI arguments: error spec directory and optional dry-run flag.
#[derive(ClapParser, Debug)]
#[clap(name = "fix_spec_layers")]
#[clap(about = "Fix layer classification in error specs based on parse behavior")]
struct Args {
    /// Directory containing error specs
    #[clap(long, value_name = "DIR")]
    spec_dir: PathBuf,

    /// Dry run - show what would be changed without modifying files
    #[clap(long)]
    dry_run: bool,
}

#[derive(Debug, Error)]
pub enum FixLayerError {
    #[error("Failed to read spec file: {path}")]
    Read {
        path: String,
        source: std::io::Error,
    },
    #[error("Failed to write spec file: {path}")]
    Write {
        path: String,
        source: std::io::Error,
    },
    #[error("Missing metadata layer entry")]
    MissingLayer,
    #[error("Missing CHAT example")]
    MissingChatExample,
    #[error("Failed to parse CHAT example")]
    ParseFailed,
    #[error("Failed to serialize markdown")]
    Serialize,
}

/// Fixes layer classification in auto-generated error specs by testing each input against the parser.
fn main() -> Result<(), FixLayerError> {
    let args = Args::parse();

    println!(
        "Fixing layer classifications in {}",
        args.spec_dir.display()
    );
    if args.dry_run {
        println!("DRY RUN - no files will be modified\n");
    }

    let spec_files = discover_specs(&args.spec_dir)?;
    println!("Found {} auto-generated spec files\n", spec_files.len());

    let mut fixed = 0;
    let mut already_correct = 0;
    let mut skipped = 0;

    for path in &spec_files {
        match process_spec(path, args.dry_run) {
            Ok(SpecResult::Fixed) => {
                println!(
                    "✓ Fixed {} - changed to 'parser' layer",
                    display_filename(path)
                );
                fixed += 1;
            }
            Ok(SpecResult::AlreadyCorrect) => {
                already_correct += 1;
            }
            Ok(SpecResult::Skipped(reason)) => {
                println!("⊘ Skipped {} - {}", display_filename(path), reason);
                skipped += 1;
            }
            Err(err) => {
                eprintln!("⚠ Error processing {}: {}", path.display(), err);
            }
        }
    }

    println!(
        "
Summary:"
    );
    println!("  Fixed: {}", fixed);
    println!("  Already correct: {}", already_correct);
    println!("  Skipped: {}", skipped);
    println!("  Total: {}", spec_files.len());

    Ok(())
}

enum SpecResult {
    Fixed,
    AlreadyCorrect,
    Skipped(String),
}

fn discover_specs(dir: &Path) -> Result<Vec<PathBuf>, FixLayerError> {
    let mut files = Vec::new();

    for entry in fs::read_dir(dir).map_err(|source| FixLayerError::Read {
        path: dir.display().to_string(),
        source,
    })? {
        let entry = entry.map_err(|source| FixLayerError::Read {
            path: dir.display().to_string(),
            source,
        })?;
        let path = entry.path();

        if path.is_file() {
            if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                if name.ends_with("_auto.md") {
                    files.push(path);
                }
            }
        }
    }

    files.sort();
    Ok(files)
}

fn process_spec(path: &Path, dry_run: bool) -> Result<SpecResult, FixLayerError> {
    let content = fs::read_to_string(path).map_err(|source| FixLayerError::Read {
        path: path.display().to_string(),
        source,
    })?;

    let arena = Arena::new();
    let root = parse_document(&arena, &content, &Options::default());

    let layer = extract_metadata_value(root, "Layer").ok_or(FixLayerError::MissingLayer)?;
    if layer != "validation" {
        return Ok(SpecResult::Skipped(format!("layer is '{}'", layer)));
    }

    let chat_example = extract_chat_example(root).ok_or(FixLayerError::MissingChatExample)?;
    let parses = test_parse(&chat_example)?;

    if parses {
        return Ok(SpecResult::AlreadyCorrect);
    }

    update_metadata_value(root, "Layer", "parser")?;

    let mut output = String::new();
    format_commonmark(root, &Options::default(), &mut output)
        .map_err(|_| FixLayerError::Serialize)?;

    if !dry_run {
        fs::write(path, output).map_err(|source| FixLayerError::Write {
            path: path.display().to_string(),
            source,
        })?;
    }

    Ok(SpecResult::Fixed)
}

fn display_filename(path: &Path) -> String {
    path.file_name()
        .and_then(|name| name.to_str())
        .map(|name| name.to_string())
        .unwrap_or_else(|| path.display().to_string())
}

fn extract_chat_example<'a>(root: &'a AstNode<'a>) -> Option<String> {
    for node in root.descendants() {
        if let NodeValue::CodeBlock(block) = &node.data.borrow().value {
            if block.info == "chat" {
                return Some(block.literal.clone());
            }
        }
    }
    None
}

fn test_parse(chat_input: &str) -> Result<bool, FixLayerError> {
    let mut parser = TSParser::new();
    parser
        .set_language(&LANGUAGE.into())
        .map_err(|_| FixLayerError::ParseFailed)?;

    let tree = parser
        .parse(chat_input, None)
        .ok_or(FixLayerError::ParseFailed)?;

    Ok(!tree.root_node().has_error())
}

fn extract_metadata_value<'a>(root: &'a AstNode<'a>, key: &str) -> Option<String> {
    let mut in_metadata = false;

    for node in root.descendants() {
        match &node.data.borrow().value {
            NodeValue::Heading(heading) if heading.level == 2 => {
                let heading_text = extract_text_from_children(node);
                in_metadata = normalize_whitespace(&heading_text) == "Metadata";
            }
            NodeValue::Item(_) if in_metadata => {
                if let Some((item_key, value)) = extract_metadata_item(node) {
                    if item_key == key {
                        return Some(value);
                    }
                }
            }
            _ => {}
        }
    }

    None
}

fn update_metadata_value<'a>(
    root: &'a AstNode<'a>,
    key: &str,
    value: &str,
) -> Result<(), FixLayerError> {
    let mut in_metadata = false;

    for node in root.descendants() {
        match &node.data.borrow().value {
            NodeValue::Heading(heading) if heading.level == 2 => {
                let heading_text = extract_text_from_children(node);
                in_metadata = normalize_whitespace(&heading_text) == "Metadata";
            }
            NodeValue::Item(_) if in_metadata => {
                if let Some((item_key, _)) = extract_metadata_item(node) {
                    if item_key == key {
                        set_metadata_item_value(node, value);
                        return Ok(());
                    }
                }
            }
            _ => {}
        }
    }

    Err(FixLayerError::MissingLayer)
}

fn extract_metadata_item<'a>(item: &'a AstNode<'a>) -> Option<(String, String)> {
    let paragraph = item
        .children()
        .find(|child| matches!(child.data.borrow().value, NodeValue::Paragraph))?;
    let mut key = None;
    let mut value_text = String::new();
    let mut seen_key = false;

    for child in paragraph.children() {
        match &child.data.borrow().value {
            NodeValue::Strong => {
                if key.is_none() {
                    key = Some(extract_text_from_children(child));
                    seen_key = true;
                }
            }
            NodeValue::Text(text) if seen_key => {
                value_text.push_str(text);
            }
            _ => {}
        }
    }

    let key = key?;
    let value = match parse_value_after_separator(&value_text) {
        Some(parsed) => parsed,
        None => value_text,
    };
    Some((key, value))
}

fn set_metadata_item_value<'a>(item: &'a AstNode<'a>, value: &str) {
    let paragraph = item
        .children()
        .find(|child| matches!(child.data.borrow().value, NodeValue::Paragraph));

    let Some(paragraph) = paragraph else {
        return;
    };

    let mut seen_key = false;
    let mut updated = false;

    for child in paragraph.children() {
        match &mut child.data.borrow_mut().value {
            NodeValue::Strong => {
                if !seen_key {
                    seen_key = true;
                }
            }
            NodeValue::Text(text) if seen_key => {
                if !updated {
                    *text = format!(": {}", value).into();
                    updated = true;
                } else {
                    *text = Cow::Borrowed("");
                }
            }
            _ => {}
        }
    }
}

fn extract_text_from_children<'a>(node: &'a AstNode<'a>) -> String {
    let mut result = String::new();
    for child in node.descendants() {
        if let NodeValue::Text(text) = &child.data.borrow().value {
            result.push_str(text);
        }
    }
    result
}

fn normalize_whitespace(value: &str) -> String {
    let mut out = String::new();
    let mut in_space = false;

    for ch in value.chars() {
        if ch.is_whitespace() {
            if !in_space {
                out.push(' ');
                in_space = true;
            }
        } else {
            in_space = false;
            out.push(ch);
        }
    }

    while out.ends_with(' ') {
        out.pop();
    }

    out
}

/// Parse `": <value>"` — strip colon prefix, trim leading whitespace, return the rest.
fn parse_value_after_separator(value: &str) -> Option<String> {
    let rest = value.strip_prefix(':')?;
    let trimmed = rest.trim_start_matches([' ', '\t']);
    let line = trimmed.lines().next().unwrap_or(trimmed);
    Some(line.to_string())
}
