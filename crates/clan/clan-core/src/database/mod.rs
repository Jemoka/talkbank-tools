//! Normative database infrastructure for clinical profiling commands.
//!
//! CLAN's profiling commands (KidEval, Eval, Eval-D) compare a child's or patient's
//! language scores against normative reference databases. These databases are `.cut`
//! files containing per-speaker score vectors with demographic metadata.
//!
//! This module provides:
//!
//! - [`DatabaseEntry`] / [`DatabaseHeader`] — Typed representation of database entries
//! - [`parse_database()`] — Principled parser for the `.cut` database format
//! - [`DatabaseFilter`] — Demographic filtering (language, age, gender, corpus type)
//! - [`compare_to_norms()`] — Statistical comparison (mean, SD, z-score) against filtered norms
//! - [`discover_databases()`] — Enumerate available databases from a library directory

mod comparison;
mod discovery;
mod entry;
mod filter;
mod parser;

pub use comparison::{ComparisonResult, MeasureComparison, compare_to_norms};
pub use discovery::{AvailableDatabase, discover_databases};
pub use entry::{DatabaseEntry, DatabaseHeader, DbMetadata, Sex};
pub use filter::{DatabaseFilter, Gender};
pub use parser::{ParsedDatabase, parse_database};
