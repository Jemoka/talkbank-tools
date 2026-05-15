# Testing

**Status:** Current
**Last updated:** 2026-04-16 22:14 EDT

This chapter covers how to test both the Rust language server and the TypeScript VS Code extension.

## Rust Language Server Tests

### Running Tests

```bash
cargo test -p chatter-lsp
```

Or with the preferred parallel test runner:

```bash
cargo nextest run -p chatter-lsp
```

### Test Locations

Tests live next to the code they cover, in `#[cfg(test)] mod tests`
blocks. Notable places:

| Location | What It Tests |
|----------|--------------|
| `alignment/tests.rs` | Cross-tier alignment computation and hover info |
| `graph/tests.rs` | DOT graph generation from `%gra` data, including stale-baseline marker |
| `backend/features/*/{mod,tests}.rs` | Per-feature handlers (hover, highlights, completion, rename, references, …) |
| `test_fixtures.rs` | Shared `parse_chat` / `parse_chat_with_alignments` / `parse_tree` / `parse_tree_incremental` helpers |

### Shared fixtures

Tests build their CHAT + tree-sitter inputs through
`crate::test_fixtures`. The module exposes four helpers and nothing
else; new test modules **must not redefine their own `parse_chat` or
`parse_tree`** — duplicating these helpers leads to drift between
how production and test code construct CHAT inputs, which has caused
real bugs to slip through in the past.

```rust,ignore
use crate::test_fixtures::{parse_chat, parse_chat_with_alignments, parse_tree};
```

| Helper | Returns | Use when |
|--------|---------|----------|
| `parse_chat(content)` | `ChatFile` | Test needs a parsed model but no per-utterance alignments |
| `parse_chat_with_alignments(content)` | `ChatFile` with `utterance.alignments = Some(_)` | Test exercises main↔`%mor`/`%gra`/`%pho`/`%sin` alignment or the `%wor` timing sidecar |
| `parse_tree(content)` | `tree_sitter::Tree` | Test walks the CST (most feature tests) |
| `parse_tree_incremental(content)` | `tree_sitter::Tree` via `TreeSitterParser::parse_tree_incremental` | Test exercises the incremental-parse code path (references / rename) |

### Test pattern

Feature handlers are pure functions (`&Backend` or `&Utterance` +
params → result), making them straightforward to test:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_fixtures::{parse_chat_with_alignments, parse_tree};

    #[test]
    fn hover_on_main_tier_word_includes_mor_lemma() {
        let content = "@UTF8\n@Begin\n...\n*CHI:\thello .\n%mor:\tn|hello .\n@End\n";
        let chat_file = parse_chat_with_alignments(content);
        let tree = parse_tree(content);
        let pos = Position { line: 3, character: 7 };

        let hover = hover(&chat_file, &tree, pos, content, ParseState::Clean)
            .expect("hover must resolve on the main-tier word");
        let text = match &hover.contents {
            HoverContents::Markup(m) => &m.value,
            _ => panic!("expected markup"),
        };

        assert!(text.contains("%mor") || text.contains("Lemma"));
    }
}
```

### Mandatory regression gates

Any change touching parser, model, validation, alignment, or
serialization must pass the following before commit:

```bash
cargo nextest run -p talkbank-parser-tests -E 'test(parser_equivalence)'
cargo nextest run -p talkbank-parser-tests --test roundtrip_reference_corpus
```

Both must show `93 passed` and `98 passed` respectively (current
numbers). See the workspace `CLAUDE.md` for the full pre-merge gate
list.

## TypeScript Extension Tests

### Running Tests

```bash
cd vscode
npm test              # Run all tests (vitest)
npm run lint          # ESLint
npm run test:coverage # Tests with coverage report
```

### Test Framework

The extension uses [Vitest](https://vitest.dev/) for unit tests. Test files live in `src/test/`.

### What to Test

- **Utility functions** (`bulletParser.ts`, `alignmentSidecar.ts`, `mediaResolver.ts`) -- these are pure functions with no VS Code API dependencies
- **Panel message handling** -- test that PostMessage payloads are correctly formatted
- **Command argument construction** -- test that LSP command arguments are correctly assembled

VS Code API interactions (commands, webviews, tree views) are harder to unit test. For those, use manual testing.

## Manual Testing

### Launch Development Extension

```bash
cd vscode
code --extensionDevelopmentPath=.
```

This opens a new VS Code window with the extension loaded from source. Changes require recompiling (`npm run compile`) and reloading the window (`Cmd+Shift+P` > "Developer: Reload Window").

### Test Files

Use files from `resources/corpus/reference/` for manual testing. This directory contains 87 CHAT files covering 20 languages and diverse CHAT constructs.

### Debugging the Language Server

Enable detailed logging by setting `RUST_LOG` before launching VS Code:

```bash
RUST_LOG=debug code --extensionDevelopmentPath=.
```

For targeted tracing of a specific module:

```bash
RUST_LOG=chatter_lsp::alignment=trace code --extensionDevelopmentPath=.
```

Server output appears in the Output panel under "TalkBank Language Server".

### LSP Message Tracing

To inspect the raw JSON-RPC messages between VS Code and the server:

1. Check if the extension supports the `chatter-lsp.trace.server` setting
2. Set it to `"verbose"` to log all request/response payloads
3. Alternatively, use VS Code's built-in LSP inspector (if available in your version)

## Coverage

```bash
cd vscode
npm run test:coverage
```

This generates a coverage report using Vitest's built-in coverage via `@vitest/coverage-v8`. Check the report to identify untested utility functions or edge cases.

## What Must Pass Before Committing

For LSP changes:

```bash
cargo nextest run -p chatter-lsp        # LSP tests
cargo nextest run -p talkbank-parser-tests  # Parser equivalence (if touching parsing)
```

For extension changes:

```bash
cd vscode && npm test && npm run lint
```

For any change:

```bash
bazel build //... && bazel test //...    # Full pre-merge verification gates (G0-G13)
```

## Related Chapters

- [Architecture](architecture.md) -- understanding the codebase structure
- [Adding Features](adding-features.md) -- testing new features
- [Troubleshooting: LSP Connection](../troubleshooting/lsp.md) -- debugging server issues
