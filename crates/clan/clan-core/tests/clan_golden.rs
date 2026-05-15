//! Golden tests comparing clan-core output against CLAN CLI output.
//!
//! These tests are split by concern:
//! - `harness` contains shared CLAN/Rust execution helpers and generated test runners
//! - `baseline`, `check`, and `variants_*` declare manifest-style parity cases
//! - `rust_only` keeps bespoke temp-file coverage alongside simple manifest-driven snapshots

mod common;
#[path = "clan_golden/harness.rs"]
mod harness;

use crate::harness::{
    FilterSpec, OutputFormat, ParityCase, RustSnapshotCase, corpus_dir, corpus_file,
    parity_case_tests, rust_snapshot_tests,
};

include!("clan_golden/baseline.rs");
include!("clan_golden/rust_only.rs");
include!("clan_golden/check.rs");
include!("clan_golden/variants_a.rs");
include!("clan_golden/variants_b.rs");
