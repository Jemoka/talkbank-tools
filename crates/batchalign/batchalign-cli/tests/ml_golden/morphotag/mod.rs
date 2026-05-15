//! Morphotag-focused live-model tests and shared support code.
//!
//! This subtree exists because morphotag has become the primary architecture
//! migration and reliability-hardening target. Keeping its fixtures and helpers
//! local makes it easier to expand coverage without bloating generic test files.

pub mod direct_behavior;
mod error_paths;
pub mod fixtures;
mod golden;
mod golden_l2;
pub mod helpers;
pub mod options;
pub mod parity;
pub mod server_behavior;
