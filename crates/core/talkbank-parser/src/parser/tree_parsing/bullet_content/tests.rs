//! Tests for this subsystem.
//!

use super::parse_bullet_content;
use crate::error::{ErrorCollector, ParseError};
use crate::node_types::{TEXT_WITH_BULLETS, TEXT_WITH_BULLETS_AND_PICS};
use std::fs;
use std::path::PathBuf;
use talkbank_model::model::{BulletContent, BulletContentSegment};
use tree_sitter::Parser;

/// Parse a real test file and extract the %act tier content
fn parse_test_file(filename: &str) -> Result<(BulletContent, Vec<ParseError>), String> {
    let mut parser = Parser::new();
    parser
        .set_language(&tree_sitter_talkbank::LANGUAGE.into())
        .map_err(|err| format!("Failed to set tree-sitter language: {err}"))?;

    // Read test file from local reference corpus.
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.pop(); // crates
    path.pop(); // repo root
    path.push("resources/corpus/reference");
    path.push(filename);

    let source = fs::read_to_string(&path)
        .map_err(|err| format!("Could not read test file {:?}: {err}", path))?;

    let tree = parser
        .parse(&source, None)
        .ok_or_else(|| "Failed to parse test file".to_string())?;
    let root = tree.root_node();

    // Recursively find act_dependent_tier or com_dependent_tier
    /// Finds tier node.
    fn find_tier_node(node: tree_sitter::Node) -> Option<tree_sitter::Node> {
        use crate::node_types::{ACT_DEPENDENT_TIER, COM_DEPENDENT_TIER};

        if node.kind() == ACT_DEPENDENT_TIER || node.kind() == COM_DEPENDENT_TIER {
            return Some(node);
        }

        for i in 0..node.child_count() {
            if let Some(child) = node.child(i as u32)
                && let Some(found) = find_tier_node(child)
            {
                return Some(found);
            }
        }

        None
    }

    let tier = find_tier_node(root)
        .ok_or_else(|| "Should find act or com tier in test file".to_string())?;
    let mut cursor = tier.walk();
    let content_node = tier
        .children(&mut cursor)
        .find(|child| {
            let kind = child.kind();
            kind == TEXT_WITH_BULLETS || kind == TEXT_WITH_BULLETS_AND_PICS
        })
        .ok_or_else(|| "Should find text_with_bullets(_and_pics) content node".to_string())?;

    let error_sink = ErrorCollector::new();
    let content = parse_bullet_content(content_node, &source, &error_sink);
    Ok((content, error_sink.into_vec()))
}

/// Tests real file with bullets.
#[test]
fn test_real_file_with_bullets() -> Result<(), String> {
    let (content, errors) = parse_test_file("content/media-bullets.cha")?;
    assert!(errors.is_empty(), "Should have no errors: {:?}", errors);

    // From media-bullets.cha line 14: %act:\tfoo 2061689_2062652 bar 2061689_2062652
    // Expected: text("foo "), bullet(2061689, 2062652), text(" bar "), bullet(2061689, 2062652)
    assert!(
        content.segments.len() >= 2,
        "Should have at least 2 segments"
    );

    // Check that we have bullet segments
    let has_bullet = content
        .segments
        .iter()
        .any(|seg| matches!(seg, BulletContentSegment::Bullet(_)));
    assert!(has_bullet, "Should have at least one bullet segment");
    Ok(())
}

/// Tests plain text only.
#[test]
fn test_plain_text_only() {
    // Test with a simple construction for plain text (no real file needed)
    let segments = vec![BulletContentSegment::text("hello world")];
    let content = BulletContent::new(segments);
    assert_eq!(content.segments.len(), 1);
    assert!(!content.is_empty());
}
