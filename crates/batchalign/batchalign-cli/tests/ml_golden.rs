// Integration test target: Cargo compiles this as a separate crate,
// so the lib's `cfg_attr(test, ...)` allow does not apply. Test code
// uses `unwrap`/`expect` by convention.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unreachable
)]

//! Single test binary for all ML-dependent integration tests.
//!
//! Consolidating all ML tests into one binary ensures one process = one
//! shared warmed worker backend for both direct and server-specific fixture
//! sessions. This prevents the OOM crashes caused by independent binaries each
//! spawning their own worker pools (multiple Whisper/Stanza model copies).
//!
//! Run: `cargo nextest run -p batchalign-cli --profile ml` (the `ml` profile's
//! `default-filter` selects this binary; bare `cargo nextest run` excludes it).
//! Update golden snapshots: `cargo insta review`

mod common;

// ML test submodules — each was previously a separate binary with its own
// worker pool. Now they share one process-global LazyLock<LiveFixtureBackend>.
mod ml_golden {
    pub mod align;
    pub mod audio_helpers;
    pub mod avqi;
    pub mod benchmark;
    pub mod compare;
    pub mod compare_master_parity;
    pub mod coref;
    pub mod error_paths;
    pub mod golden;
    pub mod live_server_fixture;
    pub mod morphotag;
    pub mod opensmile;
    pub mod options;
    mod parity_helpers;
    pub mod profile_verification;
    pub mod regression_fixtures;
    pub mod transcribe;
    pub mod translate;
    pub mod utseg;
}
