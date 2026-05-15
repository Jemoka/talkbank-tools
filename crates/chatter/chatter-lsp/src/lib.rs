#![warn(missing_docs)]
// Test code is exempt from this crate's `deny`-level panic lints.
// The non-test surface was audited 2026-04-29 (see
// `book/src/operations/panic-audit/chatter-lsp.md`); test code keeps panic
// discipline relaxed because assertion macros panic by design and
// fixture `unwrap()` is the standard Rust testing idiom. Without
// this `cfg_attr(test, ...)` allow, the crate's `deny` lints would
// fire on test code that has no production analogue.
#![cfg_attr(
    test,
    allow(
        clippy::unwrap_used,
        clippy::expect_used,
        clippy::panic,
        clippy::unreachable,
        clippy::todo,
        clippy::unimplemented
    )
)]
//! TalkBank LSP — Language Server Protocol implementation for CHAT format.
//!
//! This crate provides the core logic behind the standalone `chatter-lsp`
//! binary: incremental tree-sitter parsing, real-time validation diagnostics,
//! hover information (alignment timing, speaker details), completions, code
//! actions, and semantic token highlighting.
//!
//! The library is split into public modules so that integration tests can
//! exercise individual subsystems without going through the full LSP wire
//! protocol. The reusable stdio server entrypoint exposed here is what powers
//! the `chatter-lsp` binary (see `src/bin/chatter-lsp.rs`).
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Format>
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Headers>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Main_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>

pub mod alignment;
pub mod backend;
pub mod graph;
pub mod highlight;
pub mod semantic_tokens;

#[cfg(test)]
mod test_fixtures;

use backend::Backend;
use tower_lsp::{LspService, Server};

/// Serve the TalkBank language server over standard input/output inside the
/// current Tokio runtime.
pub async fn serve_stdio() {
    let (service, socket) = LspService::new(Backend::new);
    Server::new(tokio::io::stdin(), tokio::io::stdout(), socket)
        .serve(service)
        .await;
}

/// Create a Tokio runtime and serve the TalkBank language server over stdio.
///
/// This is the reusable entrypoint for the standalone `chatter-lsp` binary.
pub fn run_stdio_server() -> std::io::Result<()> {
    tokio::runtime::Runtime::new()?.block_on(async {
        serve_stdio().await;
        Ok(())
    })
}
