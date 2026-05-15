//! Enhance auto-generated error specifications
//!
//! This tool improves auto-generated specs by:
//! - Fixing "Expected Behavior" text to match layer (parser vs validation)
//! - Adding CHAT manual references to CHAT Rule section
//! - Improving minimal descriptions
//!
//! Usage:
//!   cargo run --bin enhance_specs -- --spec-dir ../spec/errors

use clap::Parser as ClapParser;
use comrak::nodes::{AstNode, NodeValue};
use comrak::{format_commonmark, parse_document, Arena, Options};
use std::borrow::Cow;
use std::fs;
use std::path::{Path, PathBuf};
use thiserror::Error;

/// CLI arguments: error spec directory and optional dry-run flag.
#[derive(ClapParser, Debug)]
#[clap(name = "enhance_specs")]
#[clap(about = "Enhance auto-generated error specifications")]
struct Args {
    /// Directory containing error specs
    #[clap(long, value_name = "DIR")]
    spec_dir: PathBuf,

    /// Dry run - show what would be changed without modifying files
    #[clap(long)]
    dry_run: bool,
}

#[derive(Debug, Error)]
pub enum EnhanceError {
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
    #[error("Missing error code metadata")]
    MissingErrorCode,
    #[error("Missing layer metadata")]
    MissingLayer,
    #[error("Failed to serialize markdown")]
    Serialize,
}

/// Improves auto-generated error specs: fixes expected behavior text, adds CHAT manual references.
fn main() -> Result<(), EnhanceError> {
    let args = Args::parse();

    println!(
        "Enhancing error specifications in {}",
        args.spec_dir.display()
    );
    if args.dry_run {
        println!("DRY RUN - no files will be modified\n");
    }

    let spec_files = discover_specs(&args.spec_dir)?;
    println!("Found {} auto-generated spec files\n", spec_files.len());

    let mut enhanced = 0;
    let mut skipped = 0;

    for path in &spec_files {
        match process_spec(path, args.dry_run) {
            Ok(true) => {
                println!("✓ Enhanced {}", display_filename(path));
                enhanced += 1;
            }
            Ok(false) => {
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
    println!("  Enhanced: {}", enhanced);
    println!("  Skipped: {}", skipped);
    println!("  Total: {}", spec_files.len());

    Ok(())
}

fn discover_specs(dir: &Path) -> Result<Vec<PathBuf>, EnhanceError> {
    let mut files = Vec::new();

    for entry in fs::read_dir(dir).map_err(|source| EnhanceError::Read {
        path: dir.display().to_string(),
        source,
    })? {
        let entry = entry.map_err(|source| EnhanceError::Read {
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

fn process_spec(path: &Path, dry_run: bool) -> Result<bool, EnhanceError> {
    let content = fs::read_to_string(path).map_err(|source| EnhanceError::Read {
        path: path.display().to_string(),
        source,
    })?;

    let arena = Arena::new();
    let root = parse_document(&arena, &content, &Options::default());

    let error_code =
        extract_metadata_value(root, "Error Code").ok_or(EnhanceError::MissingErrorCode)?;
    let layer = extract_metadata_value(root, "Layer").ok_or(EnhanceError::MissingLayer)?;

    let mut modified = false;

    if fix_expected_behavior(root, &arena, &layer) {
        modified = true;
    }

    if update_chat_rule(root, &error_code) {
        modified = true;
    }

    if modified && !dry_run {
        let mut output = String::new();
        format_commonmark(root, &Options::default(), &mut output)
            .map_err(|_| EnhanceError::Serialize)?;
        fs::write(path, output).map_err(|source| EnhanceError::Write {
            path: path.display().to_string(),
            source,
        })?;
    }

    Ok(modified)
}

fn display_filename(path: &Path) -> String {
    path.file_name()
        .and_then(|name| name.to_str())
        .map(|name| name.to_string())
        .unwrap_or_else(|| path.display().to_string())
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
    let value = parse_value_after_separator(&value_text).unwrap_or(value_text);
    Some((key, value))
}

fn fix_expected_behavior<'a>(root: &'a AstNode<'a>, arena: &'a Arena<'a>, layer: &str) -> bool {
    let mut in_section = false;
    let mut target_paragraph = None;
    let mut trigger_value = None;
    let mut modified = false;

    for node in root.descendants() {
        match &node.data.borrow().value {
            NodeValue::Heading(heading) if heading.level == 2 => {
                let heading_text = extract_text_from_children(node);
                in_section = normalize_whitespace(&heading_text) == "Expected Behavior";
            }
            NodeValue::Paragraph if in_section && target_paragraph.is_none() => {
                target_paragraph = Some(node);
            }
            NodeValue::Paragraph if in_section => {
                if trigger_value.is_none() {
                    trigger_value = extract_trigger_from_paragraph(node);
                }
            }
            _ => {}
        }
    }

    let desired = if layer == "parser" {
        "The parser should reject this CHAT input and report a parse error at the location of the invalid syntax."
    } else {
        "The parser should successfully parse this CHAT file, but validation should report the error."
    };

    if let Some(paragraph) = target_paragraph {
        if update_paragraph_text(paragraph, desired) {
            modified = true;
        }
    }

    let trigger = trigger_value.unwrap_or_else(|| "See example above".to_string());
    if ensure_trigger_paragraph(root, arena, &trigger) {
        modified = true;
    }

    modified
}

fn update_chat_rule<'a>(root: &'a AstNode<'a>, error_code: &str) -> bool {
    let mut in_section = false;
    let mut modified = false;
    let replacement = generate_chat_rule(error_code);

    for node in root.descendants() {
        match &node.data.borrow().value {
            NodeValue::Heading(heading) if heading.level == 2 => {
                let heading_text = extract_text_from_children(node);
                in_section = normalize_whitespace(&heading_text) == "CHAT Rule";
            }
            NodeValue::Paragraph if in_section => {
                let paragraph_text = extract_text_from_children(node);
                if paragraph_text == "[Add link to relevant CHAT manual section]"
                    && update_paragraph_text(node, &replacement)
                {
                    modified = true;
                }
            }
            _ => {}
        }
    }

    modified
}

fn extract_trigger_from_paragraph<'a>(paragraph: &'a AstNode<'a>) -> Option<String> {
    let mut seen_trigger = false;
    let mut value_text = String::new();

    for child in paragraph.children() {
        match &child.data.borrow().value {
            NodeValue::Strong => {
                let strong_text = extract_text_from_children(child);
                if strong_text == "Trigger" {
                    seen_trigger = true;
                }
            }
            NodeValue::Text(text) if seen_trigger => {
                value_text.push_str(text);
            }
            _ => {}
        }
    }

    if seen_trigger {
        Some(parse_value_after_separator(&value_text).unwrap_or(value_text))
    } else {
        None
    }
}

fn ensure_trigger_paragraph<'a>(
    root: &'a AstNode<'a>,
    arena: &'a Arena<'a>,
    trigger: &str,
) -> bool {
    let mut in_section = false;
    let mut has_trigger = false;
    let mut last_node = None;

    for node in root.descendants() {
        match &node.data.borrow().value {
            NodeValue::Heading(heading) if heading.level == 2 => {
                let heading_text = extract_text_from_children(node);
                in_section = normalize_whitespace(&heading_text) == "Expected Behavior";
            }
            NodeValue::Paragraph if in_section => {
                if extract_trigger_from_paragraph(node).is_some() {
                    has_trigger = true;
                }
                last_node = Some(node);
            }
            _ => {}
        }
    }

    if has_trigger {
        return false;
    }

    let Some(last_node) = last_node else {
        return false;
    };

    let paragraph = arena.alloc(AstNode::from(NodeValue::Paragraph));
    let strong = arena.alloc(AstNode::from(NodeValue::Strong));
    let strong_text = arena.alloc(AstNode::from(NodeValue::Text("Trigger".into())));
    strong.append(strong_text);
    paragraph.append(strong);
    paragraph.append(arena.alloc(AstNode::from(NodeValue::Text(
        format!(": {}", trigger).into(),
    ))));
    last_node.insert_after(paragraph);
    true
}

fn update_paragraph_text<'a>(paragraph: &'a AstNode<'a>, text: &str) -> bool {
    let mut updated = false;
    for child in paragraph.children() {
        if let NodeValue::Text(value) = &mut child.data.borrow_mut().value {
            if !updated {
                *value = text.to_string().into();
                updated = true;
            } else {
                *value = Cow::Borrowed("");
            }
        }
    }
    updated
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

fn generate_chat_rule(error_code: &str) -> String {
    match error_code_prefix(error_code) {
        Some(b'2') => "See CHAT manual sections on word-level syntax and special markers. \
             The CHAT manual is available at: https://talkbank.org/0info/manuals/CHAT.pdf"
            .to_string(),
        Some(b'3') => "See CHAT manual sections on main tier syntax and utterance structure. \
             Every utterance must end with a terminator (., ?, or !). \
             The CHAT manual is available at: https://talkbank.org/0info/manuals/CHAT.pdf"
            .to_string(),
        Some(b'4') => "See CHAT manual sections on dependent tier syntax (%mor, %gra, etc.). \
             The CHAT manual is available at: https://talkbank.org/0info/manuals/CHAT.pdf"
            .to_string(),
        Some(b'5') => "See CHAT manual sections on file headers and metadata. \
             Headers like @Participants, @Languages, and @ID have specific format requirements. \
             The CHAT manual is available at: https://talkbank.org/0info/manuals/CHAT.pdf"
            .to_string(),
        Some(b'6') => "See CHAT manual sections on tier alignment and word counting. \
             Dependent tiers must align with main tier word counts. \
             The CHAT manual is available at: https://talkbank.org/0info/manuals/CHAT.pdf"
            .to_string(),
        Some(b'7') => {
            "See CHAT manual sections on dependent tier formats (%mor, %gra, %pho, etc.). \
             Each tier type has specific syntax requirements. \
             The CHAT manual is available at: https://talkbank.org/0info/manuals/CHAT.pdf"
                .to_string()
        }
        _ => "See the CHAT manual for format specifications: \
             https://talkbank.org/0info/manuals/CHAT.pdf"
            .to_string(),
    }
}

fn error_code_prefix(error_code: &str) -> Option<u8> {
    let bytes = error_code.as_bytes();
    if bytes.len() < 2 {
        return None;
    }
    if bytes[0] != b'E' {
        return None;
    }
    Some(bytes[1])
}
