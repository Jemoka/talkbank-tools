//! Penn Treebank bracket notation parser and tree utilities.
//!
//! Parses constituency tree strings like `(S (NP (DT the) (NN cat)) (VP (VBD
//! sat)))` into a typed tree structure, and provides utilities for extracting
//! S-level phrase ranges needed by utterance segmentation.

/// A constituency parse tree node.
#[derive(Debug, Clone, PartialEq)]
pub struct Tree {
    /// Node label (e.g., "S", "NP", "VP", "DT").
    pub label: String,
    /// Child nodes. Empty for leaf (terminal) nodes.
    pub children: Vec<Tree>,
    /// Leaf text, if this is a terminal node.
    pub leaf_text: Option<String>,
}

impl Tree {
    /// Returns `true` if this node is a leaf (terminal) node.
    pub fn is_leaf(&self) -> bool {
        self.children.is_empty() && self.leaf_text.is_some()
    }
}

/// Count the number of leaf (terminal) nodes under a tree.
pub fn leaf_count(tree: &Tree) -> usize {
    if tree.is_leaf() {
        return 1;
    }
    tree.children.iter().map(leaf_count).sum()
}

/// Parse a Penn Treebank bracket notation string into a [`Tree`].
///
/// Handles standard PTB format: `(LABEL child1 child2 ...)` where children can
/// be subtrees or bare words (leaves).
///
/// # Errors
///
/// Returns `Err` if the input is malformed.
pub fn parse_bracket_notation(s: &str) -> Result<Tree, ParseError> {
    let tokens = tokenize(s);
    if tokens.is_empty() {
        return Err(ParseError::Empty);
    }
    let (tree, consumed) = parse_tree(&tokens, 0)?;
    if consumed != tokens.len() {
        return Err(ParseError::TrailingTokens { at: consumed });
    }
    Ok(tree)
}

/// Recursively extract S-level phrase leaf-index ranges from a constituency
/// tree.
///
/// This mirrors the Python `_parse_tree_indices()` function. It walks the tree
/// looking for coordination structures (siblings with CC or CONJ labels) and
/// extracts leaf ranges for S nodes within those coordinations.
pub fn parse_tree_indices(subtree: &Tree, offset: usize) -> Vec<Vec<usize>> {
    if subtree.is_leaf() {
        return Vec::new();
    }

    let subtree_labels: Vec<String> = subtree
        .children
        .iter()
        .map(|c| c.label.to_lowercase())
        .collect();

    let has_coordination = subtree_labels
        .iter()
        .any(|lbl| lbl == "cc" || lbl == "conj");

    let mut result = Vec::new();
    let mut child_offset = offset;

    for child in &subtree.children {
        if child.is_leaf() {
            child_offset += 1;
            continue;
        }

        let n_leaves = leaf_count(child);
        let child_start = child_offset;

        if has_coordination && child.label == "S" {
            result.push((child_start..child_start + n_leaves).collect());
        }

        result.extend(parse_tree_indices(child, child_start));

        child_offset = child_start + n_leaves;
    }

    result
}

#[derive(Debug, Clone, PartialEq)]
enum Token {
    Open,
    Close,
    Word(String),
}

fn tokenize(s: &str) -> Vec<Token> {
    let mut tokens = Vec::new();
    let mut chars = s.chars().peekable();

    while let Some(&ch) = chars.peek() {
        match ch {
            '(' => {
                tokens.push(Token::Open);
                chars.next();
            }
            ')' => {
                tokens.push(Token::Close);
                chars.next();
            }
            c if c.is_whitespace() => {
                chars.next();
            }
            _ => {
                let mut word = String::new();
                while let Some(&c) = chars.peek() {
                    if c == '(' || c == ')' || c.is_whitespace() {
                        break;
                    }
                    word.push(c);
                    chars.next();
                }
                tokens.push(Token::Word(word));
            }
        }
    }

    tokens
}

fn parse_tree(tokens: &[Token], pos: usize) -> Result<(Tree, usize), ParseError> {
    if pos >= tokens.len() {
        return Err(ParseError::UnexpectedEnd);
    }

    match &tokens[pos] {
        Token::Open => {
            let label_pos = pos + 1;
            if label_pos >= tokens.len() {
                return Err(ParseError::UnexpectedEnd);
            }

            let label = match &tokens[label_pos] {
                Token::Word(w) => w.clone(),
                Token::Open => {
                    let mut children = Vec::new();
                    let mut cur = label_pos;
                    while cur < tokens.len() && tokens[cur] != Token::Close {
                        let (child, next) = parse_tree(tokens, cur)?;
                        children.push(child);
                        cur = next;
                    }
                    if cur >= tokens.len() {
                        return Err(ParseError::UnexpectedEnd);
                    }
                    return Ok((
                        Tree {
                            label: String::new(),
                            children,
                            leaf_text: None,
                        },
                        cur + 1,
                    ));
                }
                Token::Close => {
                    return Ok((
                        Tree {
                            label: String::new(),
                            children: Vec::new(),
                            leaf_text: None,
                        },
                        label_pos + 1,
                    ));
                }
            };

            let mut cur = label_pos + 1;
            let mut children = Vec::new();

            while cur < tokens.len() && tokens[cur] != Token::Close {
                match &tokens[cur] {
                    Token::Open => {
                        let (child, next) = parse_tree(tokens, cur)?;
                        children.push(child);
                        cur = next;
                    }
                    Token::Word(w) => {
                        children.push(Tree {
                            label: w.clone(),
                            children: Vec::new(),
                            leaf_text: Some(w.clone()),
                        });
                        cur += 1;
                    }
                    Token::Close => break,
                }
            }

            if cur >= tokens.len() {
                return Err(ParseError::UnexpectedEnd);
            }
            assert_eq!(tokens[cur], Token::Close);

            Ok((
                Tree {
                    label,
                    children,
                    leaf_text: None,
                },
                cur + 1,
            ))
        }
        Token::Word(w) => Ok((
            Tree {
                label: w.clone(),
                children: Vec::new(),
                leaf_text: Some(w.clone()),
            },
            pos + 1,
        )),
        Token::Close => Err(ParseError::UnexpectedClose { at: pos }),
    }
}

/// Errors from parsing bracket notation.
#[derive(Debug, thiserror::Error)]
pub enum ParseError {
    /// Input string was empty.
    #[error("empty bracket notation string")]
    Empty,
    /// Unexpected end of input.
    #[error("unexpected end of bracket notation")]
    UnexpectedEnd,
    /// Unexpected closing paren.
    #[error("unexpected ')' at token position {at}")]
    UnexpectedClose {
        /// Token position of the unexpected close paren.
        at: usize,
    },
    /// Trailing tokens after the main tree.
    #[error("trailing tokens at position {at}")]
    TrailingTokens {
        /// Token position where trailing input starts.
        at: usize,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_tree() {
        let tree = parse_bracket_notation("(S (NP (DT the) (NN cat)) (VP (VBD sat)))").unwrap();
        assert_eq!(tree.label, "S");
        assert_eq!(tree.children.len(), 2);
        assert_eq!(tree.children[0].label, "NP");
        assert_eq!(tree.children[1].label, "VP");
    }

    #[test]
    fn test_leaf_count() {
        let tree = parse_bracket_notation("(S (NP (DT the) (NN cat)) (VP (VBD sat)))").unwrap();
        assert_eq!(leaf_count(&tree), 3);
    }

    #[test]
    fn test_parse_single_word() {
        let tree = parse_bracket_notation("(S (INTJ hello))").unwrap();
        assert_eq!(tree.label, "S");
        assert_eq!(leaf_count(&tree), 1);
    }

    #[test]
    fn test_parse_tree_indices_no_coordination() {
        let tree = parse_bracket_notation("(S (NP (DT the) (NN cat)) (VP (VBD sat)))").unwrap();
        let indices = parse_tree_indices(&tree, 0);
        assert!(indices.is_empty());
    }

    #[test]
    fn test_parse_tree_indices_with_coordination() {
        let tree = parse_bracket_notation(
            "(ROOT (S (S (NP (PRP I)) (VP (VBP eat))) (CC and) (S (NP (PRP he)) (VP (VBZ runs)))))",
        )
        .unwrap();
        let indices = parse_tree_indices(&tree, 0);
        assert_eq!(indices.len(), 2);
        assert_eq!(indices[0], vec![0, 1]);
        assert_eq!(indices[1], vec![3, 4]);
    }

    #[test]
    fn test_parse_empty() {
        assert!(parse_bracket_notation("").is_err());
    }

    #[test]
    fn test_leaf_is_leaf() {
        let tree = parse_bracket_notation("(S (NP (DT the)))").unwrap();
        assert!(!tree.is_leaf());
        assert!(!tree.children[0].is_leaf());
        assert!(!tree.children[0].children[0].is_leaf());
        assert!(tree.children[0].children[0].children[0].is_leaf());
    }
}
