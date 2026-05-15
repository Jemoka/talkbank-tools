//! Server-path morphotag behavior and recovery tests.
//!
//! These tests stay on the HTTP/job-control path because they verify progress
//! aggregation, result assembly, restart semantics, and cache behavior through
//! the live server boundary rather than direct execution.

mod cache;
mod l2_and_isolation;
mod progress;
mod recovery;
