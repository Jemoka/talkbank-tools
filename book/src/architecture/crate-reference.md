# Crate Reference

**Status:** Current
**Last updated:** 2026-03-24 01:32 EDT

Summary of every crate in the `talkbank-tools` workspace.

## Core Crates

### talkbank-model

The typed data model for CHAT files. Defines `ChatFile`, `Utterance`, `DependentTier`, `MorTier`, `GraTier`, and all other AST types. Includes validation logic, the `WriteChat` trait for CHAT serialization, serde support for JSON, and `JsonSchema` derivations. Also owns error types (`ParseError`, `ErrorSink` trait, `Span`, `SourceLocation`), diagnostic infrastructure, and `ParseValidateOptions`. Provides a closure-based content walker (`walk_words` / `walk_words_mut`) that centralizes recursive traversal of `UtteranceContent` and `BracketedItem` with domain-aware group gating.

### talkbank-derive

Procedural macros for the model crate (`SemanticEq`, `SemanticDiff`, `SpanShift`, `ValidationTagged`, and the `error_code_enum` macro).

### talkbank-parser

The sole parser. Wraps the tree-sitter C parser and converts the concrete syntax tree (CST) into `ChatFile` model types. Provides error recovery via tree-sitter's GLR algorithm. Used by the LSP, CLI, and batchalign3.

### talkbank-transform

High-level pipelines: parse+validate, CHAT-to-JSON, JSON-to-CHAT, normalization. Integrates the validation cache, JSON schema validation, and parallel directory validation.

## Application Crates

### clan-core

CLAN analysis commands (FREQ, MLU, etc.), transforms (FLO, etc.), and format converters. Each command implements the `CommandOutput` trait with typed results. The crate also owns higher-level library integration seams such as `UtteranceRange`, `DiscoveredChatFiles`, `service::AnalysisCommandName`, `service::AnalysisService` / `service::AnalysisRequest`, and the `service::AnalysisOptions` / `service::AnalysisRequestBuilder` layer so other crates can execute CLAN analyses without reimplementing directory walking, range parsing, raw command-name parsing, JSON output policy, or command-default selection.

### chatter-cli

The `chatter` CLI binary: validate, normalize, to-json, CLAN command dispatch, and corpus management.

### chatter-lsp

Language Server Protocol server with tree-sitter incremental parsing, real-time diagnostics, and semantic highlighting.

### send2clan-sys

FFI bindings for sending files to the CLAN application (macOS Apple Events, Windows WM_APP).

### chatter-gui

Desktop validation app (Tauri v2, React). Mandates TUI parity with the CLI.

## Test Crates

### talkbank-parser-tests

Parser tests. Runs the parser on each file in the 78-file reference corpus and validates results (each `.cha` file is its own `#[test]` via rstest, nextest-compatible). Also runs spec-generated tests, roundtrip tests, and property tests.
