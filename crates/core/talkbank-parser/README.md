# talkbank-parser

**Status:** Current
**Last updated:** 2026-04-29 07:53 EDT

Parsing implementations for CHAT format using tree-sitter.

## Overview

This crate is the canonical parser for [CHAT format](https://talkbank.org/0info/manuals/CHAT.html),
converting source text into the `ChatFile` AST defined by
[talkbank-model](https://crates.io/crates/talkbank-model). It uses the
[tree-sitter-talkbank](https://github.com/TalkBank/tree-sitter-talkbank)
grammar to produce a concrete syntax tree (CST), then converts it into
the model's abstract types.

Key features:

- **Error recovery** — The GLR-based tree-sitter parser recovers from syntax
  errors and produces partial results, making it suitable for editor
  integration (LSP) and interactive use.
- **Explicit parser handle** — Create a `TreeSitterParser` once and reuse it
  for all parsing in a scope. No hidden global state.
- **Granular parsing** — `TreeSitterParser` methods parse at any level:
  individual words, tiers, headers, or complete files.

## Usage

```rust
use talkbank_parser::TreeSitterParser;

let parser = TreeSitterParser::new().expect("parser init");

// Parse a complete CHAT file:
let chat_file = parser.parse_chat_file(source).expect("valid CHAT");

// Parse a fragment with offset adjustment and streaming errors:
let errors = talkbank_model::ErrorCollector::new();
let outcome = parser.parse_word_fragment("hello", 0, &errors);
```

## License

BSD-3-Clause. See [LICENSE](../../LICENSE) for details.
