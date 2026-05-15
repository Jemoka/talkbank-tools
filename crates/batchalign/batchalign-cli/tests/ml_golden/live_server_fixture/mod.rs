//! Focused checks for the real-model live-server fixture.
//!
//! These tests are intentionally server-specific. They verify the fixture and
//! HTTP-facing control-plane invariants that direct execution does not have:
//! - prepared workers stay warm across isolated server sessions
//! - each acquired session gets a fresh runtime layout with no prior jobs
//! - basic infer-only command families run through the shared server fixture

mod command_smoke;
mod session_invariants;
