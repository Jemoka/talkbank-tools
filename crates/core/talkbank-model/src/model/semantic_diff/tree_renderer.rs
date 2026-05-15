//! Tree-based diff visualization.
//!
//! Renders semantic differences as a hierarchical tree structure,
//! making it easy to see where parsers diverge in their interpretation.
//! Reference: <https://talkbank.org/0info/manuals/CHAT.html#File_Format>

use super::types::{SemanticDiffKind, SemanticDifference};
use std::collections::BTreeMap;

/// One node in the hierarchical semantic-difference tree.
///
/// Reference: <https://talkbank.org/0info/manuals/CHAT.html#File_Format>
#[derive(Debug, Clone)]
pub struct TreeNode {
    /// Node name (field name or index)
    pub name: String,
    /// Differences directly at this node
    pub diffs: Vec<SemanticDifference>,
    /// Child nodes
    pub children: BTreeMap<String, TreeNode>,
}

impl TreeNode {
    /// Create a new empty tree node.
    ///
    /// Nodes are assembled into a hierarchy keyed by semantic-diff path
    /// components. Child ordering is deterministic because `BTreeMap` is used.
    pub fn new(name: String) -> Self {
        Self {
            name,
            diffs: Vec::new(),
            children: BTreeMap::new(),
        }
    }

    /// Build a tree from a list of differences.
    ///
    /// Parses paths like "utterances[0].main.content[2].category" into
    /// a hierarchical tree structure. Multiple diffs that share path prefixes
    /// naturally merge under common intermediate nodes.
    pub fn from_differences(differences: &[SemanticDifference]) -> Self {
        let mut root = TreeNode::new("ChatFile".to_string());

        for diff in differences {
            root.insert_diff(diff.clone());
        }

        root
    }

    /// Insert a difference into the tree.
    fn insert_diff(&mut self, diff: SemanticDifference) {
        let path_parts = parse_path(&diff.path);

        if path_parts.is_empty() {
            // Diff at root level
            self.diffs.push(diff);
            return;
        }

        // Navigate/create tree structure
        let mut current = self;
        for (i, part) in path_parts.iter().enumerate() {
            let is_last = i == path_parts.len() - 1;

            if is_last {
                // Attach diff to this node
                let child = current
                    .children
                    .entry(part.clone())
                    .or_insert_with(|| TreeNode::new(part.clone()));
                child.diffs.push(diff.clone());
            } else {
                // Create intermediate node
                let child = current
                    .children
                    .entry(part.clone())
                    .or_insert_with(|| TreeNode::new(part.clone()));
                current = child;
            }
        }
    }
}

/// Parse a semantic-diff path string into tree path components.
///
/// Example: "utterances[0].main.content[2].category"
/// Returns: ["utterances", "[0]", "main", "content", "[2]", "category"]
fn parse_path(path: &str) -> Vec<String> {
    let mut parts = Vec::new();
    let mut current = String::new();
    let mut in_bracket = false;

    for ch in path.chars() {
        match ch {
            '.' if !in_bracket => {
                if !current.is_empty() {
                    parts.push(current.clone());
                    current.clear();
                }
            }
            '[' => {
                if !current.is_empty() {
                    parts.push(current.clone());
                    current.clear();
                }
                in_bracket = true;
                current.push('[');
            }
            ']' => {
                current.push(']');
                in_bracket = false;
            }
            _ => {
                current.push(ch);
            }
        }
    }

    if !current.is_empty() {
        parts.push(current);
    }

    parts
}

/// Whether to show full tree hierarchy or collapse single-child paths.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RenderMode {
    /// Full rendering showing all intermediate nodes.
    Full,
    /// Compact rendering with collapsed intermediate nodes.
    Compact,
}

/// Render semantic-diff trees with box-drawing output.
///
/// Supports depth limiting, compact node folding, and optional byte-span
/// display for diagnostics.
///
/// Reference: <https://talkbank.org/0info/manuals/CHAT.html#File_Format>
pub struct TreeRenderer {
    max_depth: Option<usize>,
    show_spans: bool,
    mode: RenderMode,
}

impl TreeRenderer {
    /// Creates a renderer with depth/span/mode options.
    pub fn new(max_depth: Option<usize>, show_spans: bool, mode: RenderMode) -> Self {
        Self {
            max_depth,
            show_spans,
            mode,
        }
    }

    /// Render the tree to a string.
    pub fn render(&self, tree: &TreeNode) -> String {
        let mut out = String::new();
        self.render_node(tree, &mut out, "", true, 0);
        out
    }

    /// Render a single node and its children.
    ///
    /// # Arguments
    /// * `node` - The tree node to render
    /// * `out` - Output string buffer
    /// * `prefix` - Prefix for indentation (e.g., "│  ")
    /// * `is_last` - Whether this is the last child of its parent
    /// * `depth` - Current depth in the tree
    fn render_node(
        &self,
        node: &TreeNode,
        out: &mut String,
        prefix: &str,
        is_last: bool,
        depth: usize,
    ) {
        // Compact mode: skip nodes with no diffs and only one child
        if self.mode == RenderMode::Compact
            && node.diffs.is_empty()
            && node.children.len() == 1
            && let Some((child_name, child)) = node.children.iter().next()
        {
            let combined_name = if node.name == "ChatFile" {
                child_name.clone()
            } else {
                format!("{} → {}", node.name, child_name)
            };
            let mut combined_node = child.clone();
            combined_node.name = combined_name;
            self.render_node(&combined_node, out, prefix, is_last, depth);
            return;
        }

        // Render node name
        if depth == 0 {
            // Root node - no prefix
            out.push_str(&node.name);
            out.push('\n');
        } else {
            out.push_str(prefix);
            out.push_str(if is_last { "└─ " } else { "├─ " });
            out.push_str(&node.name);
            out.push('\n');
        }

        // Render diffs at this node
        for diff in &node.diffs {
            let diff_prefix = if depth == 0 {
                "   ".to_string()
            } else if is_last {
                format!("{}   ", prefix)
            } else {
                format!("{}│  ", prefix)
            };
            self.render_diff(diff, out, &diff_prefix);
        }

        // Check depth limit before rendering children
        if let Some(max_depth) = self.max_depth
            && depth >= max_depth
            && !node.children.is_empty()
        {
            let child_prefix = if depth == 0 {
                "".to_string()
            } else if is_last {
                format!("{}   ", prefix)
            } else {
                format!("{}│  ", prefix)
            };
            out.push_str(&child_prefix);
            out.push_str("└─ ... (max depth reached)\n");
            return;
        }

        // Render children
        let child_count = node.children.len();
        for (i, (_name, child)) in node.children.iter().enumerate() {
            let is_last_child = i == child_count - 1;
            let child_prefix = if depth == 0 {
                "".to_string()
            } else if is_last {
                format!("{}   ", prefix)
            } else {
                format!("{}│  ", prefix)
            };

            self.render_node(child, out, &child_prefix, is_last_child, depth + 1);
        }
    }

    /// Render a single diff.
    fn render_diff(&self, diff: &SemanticDifference, out: &mut String, prefix: &str) {
        // Symbol for diff type
        let symbol = match diff.kind {
            SemanticDiffKind::ValueMismatch => "✗",
            SemanticDiffKind::MissingKey => "⊘",
            SemanticDiffKind::ExtraKey => "+",
            SemanticDiffKind::LengthMismatch => "≠",
            SemanticDiffKind::VariantMismatch => "↔",
        };

        // Render symbol and kind
        out.push_str(prefix);
        out.push_str(symbol);
        out.push(' ');
        out.push_str(diff.kind.as_str());

        // Show span if requested
        if self.show_spans
            && let Some(span) = diff.span
        {
            out.push_str(&format!(" [bytes {}..{}]", span.start, span.end));
        }

        out.push('\n');

        // Show left and right values (indented)
        let value_prefix = format!("{}     ", prefix);
        out.push_str(&value_prefix);
        out.push_str("Left:  ");
        out.push_str(&truncate_value(&diff.left, 80));
        out.push('\n');

        out.push_str(&value_prefix);
        out.push_str("Right: ");
        out.push_str(&truncate_value(&diff.right, 80));
        out.push('\n');
    }
}

/// Truncate long diff values for compact terminal display.
///
/// The suffix preserves original length so reviewers can tell truncation
/// occurred without losing cardinality information.
fn truncate_value(value: &str, max_len: usize) -> String {
    if value.len() <= max_len {
        value.to_string()
    } else {
        format!("{}... ({} chars)", &value[..max_len - 15], value.len())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Span;

    /// Dot-separated semantic paths split into field components.
    ///
    /// This is the baseline parser behavior for non-indexed paths.
    #[test]
    fn test_parse_path_simple() {
        let path = "field1.field2.field3";
        let parts = parse_path(path);
        assert_eq!(parts, vec!["field1", "field2", "field3"]);
    }

    /// Paths with list indices preserve bracket tokens as distinct segments.
    ///
    /// Keeping indices explicit is required for stable tree insertion.
    #[test]
    fn test_parse_path_with_indices() {
        let path = "utterances[0].main.content[2].category";
        let parts = parse_path(path);
        assert_eq!(
            parts,
            vec!["utterances", "[0]", "main", "content", "[2]", "category"]
        );
    }

    /// Root path marker is preserved as a single component.
    ///
    /// This supports diffs that conceptually attach to the document root.
    #[test]
    fn test_parse_path_root() {
        let path = "/";
        let parts = parse_path(path);
        assert_eq!(parts, vec!["/"]);
    }

    /// Inserting one diff builds expected nested tree nodes.
    ///
    /// The structure check guards the internal path-to-tree projection logic.
    #[test]
    fn test_tree_node_insert_simple() {
        let diff = SemanticDifference {
            path: "field1.field2".to_string(),
            kind: SemanticDiffKind::ValueMismatch,
            left: "a".to_string(),
            right: "b".to_string(),
            span: None,
        };

        let mut root = TreeNode::new("Root".to_string());
        root.insert_diff(diff);

        assert!(root.children.contains_key("field1"));
        let field1 = &root.children["field1"];
        assert!(field1.children.contains_key("field2"));
        let field2 = &field1.children["field2"];
        assert_eq!(field2.diffs.len(), 1);
    }

    /// Renderer output includes tree labels, value pairs, and byte spans.
    ///
    /// This is a smoke test for standard non-compact rendering mode.
    #[test]
    fn test_tree_renderer_basic() {
        let diff = SemanticDifference {
            path: "field1.field2".to_string(),
            kind: SemanticDiffKind::ValueMismatch,
            left: "value1".to_string(),
            right: "value2".to_string(),
            span: Some(Span::new(10, 20)),
        };

        let tree = TreeNode::from_differences(&[diff]);
        let renderer = TreeRenderer::new(None, true, RenderMode::Full);
        let output = renderer.render(&tree);

        assert!(output.contains("ChatFile"));
        assert!(output.contains("field1"));
        assert!(output.contains("field2"));
        assert!(output.contains("value1"));
        assert!(output.contains("value2"));
        assert!(output.contains("bytes 10..20"));
    }

    /// Compact renderer collapses intermediate nodes into arrowed paths.
    ///
    /// The output should include compact markers rather than full branch depth.
    #[test]
    fn test_tree_renderer_compact() {
        let diff = SemanticDifference {
            path: "a.b.c.d".to_string(),
            kind: SemanticDiffKind::ValueMismatch,
            left: "x".to_string(),
            right: "y".to_string(),
            span: None,
        };

        let tree = TreeNode::from_differences(&[diff]);
        let renderer = TreeRenderer::new(None, false, RenderMode::Compact);
        let output = renderer.render(&tree);

        // In compact mode, intermediate nodes should be collapsed
        assert!(output.contains("→"));
    }

    /// Value truncation leaves short strings untouched and shortens long strings.
    ///
    /// The long-path assertion ensures length metadata is appended for context.
    #[test]
    fn test_truncate_value() {
        let short = "short";
        assert_eq!(truncate_value(short, 80), "short");

        let long = "a".repeat(100);
        let truncated = truncate_value(&long, 80);
        assert!(truncated.len() < 100);
        assert!(truncated.contains("..."));
        assert!(truncated.contains("100 chars"));
    }

    /// Multiple diffs from different branches render into one merged tree.
    ///
    /// The output should retain both path hierarchy and per-diff value/span details.
    #[test]
    fn test_multiple_differences_tree() {
        let diffs = vec![
            SemanticDifference {
                path: "lines[0].main.content[0].cleaned_text".to_string(),
                kind: SemanticDiffKind::ValueMismatch,
                left: "foo".to_string(),
                right: "bar".to_string(),
                span: Some(Span::new(10, 13)),
            },
            SemanticDifference {
                path: "lines[0].main.content[1].raw_text".to_string(),
                kind: SemanticDiffKind::ValueMismatch,
                left: "hello".to_string(),
                right: "world".to_string(),
                span: Some(Span::new(14, 19)),
            },
            SemanticDifference {
                path: "lines[1].dependent_tiers[0]".to_string(),
                kind: SemanticDiffKind::MissingKey,
                left: "present".to_string(),
                right: "missing".to_string(),
                span: None,
            },
        ];

        let tree = TreeNode::from_differences(&diffs);
        let renderer = TreeRenderer::new(None, true, RenderMode::Full);
        let output = renderer.render(&tree);

        // Verify structure
        assert!(output.contains("ChatFile"));
        assert!(output.contains("lines"));
        assert!(output.contains("[0]"));
        assert!(output.contains("[1]"));
        assert!(output.contains("main"));
        assert!(output.contains("content"));
        assert!(output.contains("dependent_tiers"));

        // Verify all differences are present
        assert!(output.contains("cleaned_text"));
        assert!(output.contains("raw_text"));
        assert!(output.contains("foo"));
        assert!(output.contains("bar"));
        assert!(output.contains("hello"));
        assert!(output.contains("world"));
        assert!(output.contains("bytes 10..13"));
        assert!(output.contains("bytes 14..19"));
    }

    /// Compact mode produces shorter output by collapsing intermediate nodes.
    ///
    /// The paired assertions compare compact and non-compact renderer behavior.
    #[test]
    fn test_compact_mode_collapses_intermediate() {
        let diff = SemanticDifference {
            path: "a.b.c.d.e.f".to_string(),
            kind: SemanticDiffKind::ValueMismatch,
            left: "1".to_string(),
            right: "2".to_string(),
            span: None,
        };

        let tree = TreeNode::from_differences(&[diff]);

        // Non-compact should show full tree
        let full_renderer = TreeRenderer::new(None, false, RenderMode::Full);
        let full_output = full_renderer.render(&tree);
        assert!(full_output.contains("├─") || full_output.contains("└─"));

        // Compact should use arrows
        let compact_renderer = TreeRenderer::new(None, false, RenderMode::Compact);
        let compact_output = compact_renderer.render(&tree);
        assert!(compact_output.contains("→"));
        // Compact output should be shorter
        assert!(compact_output.len() < full_output.len());
    }

    /// Every diff kind maps to its expected symbol in renderer output.
    ///
    /// This prevents silent regressions in visual diagnostics.
    #[test]
    fn test_all_diff_kinds() {
        use SemanticDiffKind::*;

        let kinds = vec![
            (ValueMismatch, "✗"),
            (MissingKey, "⊘"),
            (ExtraKey, "+"),
            (LengthMismatch, "≠"),
            (VariantMismatch, "↔"),
        ];

        for (kind, symbol) in kinds {
            let diff = SemanticDifference {
                path: "test".to_string(),
                kind,
                left: "a".to_string(),
                right: "b".to_string(),
                span: None,
            };

            let tree = TreeNode::from_differences(&[diff]);
            let renderer = TreeRenderer::new(None, false, RenderMode::Full);
            let output = renderer.render(&tree);

            assert!(
                output.contains(symbol),
                "Symbol '{}' not found for {:?}",
                symbol,
                kind
            );
        }
    }
}
