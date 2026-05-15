//! Configuration for the roundtrip test runner.
//!
//! This module handles loading configuration settings, primarily the corpus directory.
//!
//! In recent versions, settings are passed as command-line arguments parsed by `clap`
//! in `tests/roundtrip_corpus.rs`.
//!
//! ## Usage (Command Line)
//!
//! ```bash
//! cargo test --release --test roundtrip_corpus -- --corpus-dir <path> [--no-cache] [--emit-artifacts]
//! ```
