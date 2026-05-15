//! Per-command regression-fixture runner.
//!
//! Discovers fixtures under `batchalign3/resources/fixtures/<command>/regressions/`
//! and runs each one through the same in-process direct host the other ML
//! golden tests use. Each fixture's `source.json` declares the command, the
//! input CHAT, the audio (if any), and a list of structural assertions
//! checked against the output CHAT's typed AST.
//!
//! See `resources/fixtures/README.md` for the directory layout and the JSON
//! schema. See `tests/common/regression_manifest.rs` for the typed manifest
//! parser. Add new fixtures by dropping a directory into the appropriate
//! `regressions/` folder and writing one `#[tokio::test]` per fixture in
//! the command-local module.

pub(crate) mod harness;
